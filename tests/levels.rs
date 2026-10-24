//! Levels, classes, Health and Force (GitHub issue 70). On copies of every save on this
//! machine (in the system's temporary folder; the real saves are only read):
//!
//! - the engine's rules as the editor computes them reproduce the maxima the game itself
//!   wrote into every save (MaxHitPoints, MaxForcePoints);
//! - raising and lowering levels, adding, changing and removing a class, setting Health and
//!   Force directly, and healing, each written and read back, leave the values the engine's
//!   own rules give: the level records one per level, the stored maxima what the engine
//!   computes, the current values what was asked or what the game's level-up gives;
//! - round trips (up then down, add then remove, change and change back) give back the
//!   original records, bases and maxima;
//! - every copy the save keeps of the character (the player in module.ifo and pc.utc; a
//!   party member in AVAILNPCn and the area) says the same.
//!
//! For the player and party members, Force users and not, single and multiclass, at full
//! and partly spent Health and Force. save-editor-format.md section 3.4 has the rules.

use kse_core::catalog::{Catalog, Class};
use kse_core::creature;
use kse_core::formats::erf::{Erf, RES_TYPE_UTC};
use kse_core::formats::gff::{Gff, Struct, Value};
use kse_core::gamedata::GameData;
use kse_core::games::{find_installs, find_saves, Game};
use kse_core::save::{Save, Who};
use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap().filter_map(|e| e.ok()) {
        if e.path().is_file() {
            fs::copy(e.path(), to.join(e.file_name())).unwrap();
        }
    }
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("kmse-tests").join("levels").join(name);
    let _ = fs::remove_dir_all(&dir);
    dir
}

/// Everything the Health and Force rules read or write, to compare a creature before and
/// after a round trip.
#[derive(Debug, PartialEq)]
struct Shape {
    classes: Vec<(i64, i64)>,
    records: Option<Vec<Struct>>,
    hit_points: Option<i64>,
    force_points: Option<i64>,
    max_hp: i64,
    max_fp: i64,
}

fn shape(c: &Struct, game: Game) -> Shape {
    Shape {
        classes: creature::classes(c),
        records: c.list("LvlStatList").cloned(),
        hit_points: c.int("HitPoints"),
        force_points: c.int("ForcePoints"),
        max_hp: creature::max_health(c),
        max_fp: creature::max_force(c, game),
    }
}

/// What every edit must leave: the stored maxima as the engine computes them (the game
/// writes them so at a save), one level record per level for the player, and the shown
/// values reading back through the stored ones.
fn assert_consistent(c: &Struct, game: Game, what: &str) {
    assert_eq!(c.int("MaxHitPoints"), Some(creature::max_health(c)), "{}: stored MaxHitPoints", what);
    if creature::max_force(c, game) != 0 || c.get("MaxForcePoints").is_some() {
        assert_eq!(c.int("MaxForcePoints"), Some(creature::max_force(c, game)), "{}: stored MaxForcePoints", what);
    }
    if creature::uses_level_records(c) {
        assert_eq!(c.list("LvlStatList").unwrap().len() as i64, creature::level(c), "{}: one record per level", what);
    }
    if let Some(p) = c.int("PregameCurrent") {
        assert_eq!(p, creature::health(c).0, "{}: PregameCurrent is the current Health", what);
    }
}

/// The copies a save keeps of the player besides module.ifo: pc.utc in SAVEGAME.sav.
fn pc_copy(folder: &Path) -> Option<Struct> {
    let data = fs::read(folder.join("SAVEGAME.sav")).ok()?;
    let erf = Erf::read(&data).ok()?;
    let r = erf.resources.iter().find(|r| r.kind == RES_TYPE_UTC && r.name.eq_ignore_ascii_case("pc"))?;
    Gff::read(&r.data).ok().map(|g| g.root)
}

/// A party member's copies in the area the save resumes in.
fn area_copies(save: &Save, tag: &str) -> Vec<Struct> {
    save.area_git()
        .and_then(|g| g.root.list("Creature List").cloned())
        .unwrap_or_default()
        .into_iter()
        .filter(|c| c.string("Tag").map_or(false, |t| t.eq_ignore_ascii_case(tag)))
        .collect()
}

/// A class this creature does not have, of its kind (droid or not), a Jedi class when it
/// has none and an ordinary one when it has.
fn other_class(cat: &Catalog, c: &Struct, jedi: bool) -> Option<Class> {
    let have: Vec<i64> = creature::classes(c).iter().map(|(id, _)| *id).collect();
    let droid = have.iter().any(|id| cat.class(*id).map_or(false, |k| k.droid));
    cat.classes
        .iter()
        .filter(|k| !have.contains(&k.id) && k.droid == droid && !k.name.is_empty() && k.hit_die > 0)
        .filter(|k| (k.force_die > 0) == jedi)
        .find(|k| !k.name.to_ascii_lowercase().contains("cut"))
        .cloned()
}

/// Notes a failed condition, so a failure names it.
fn note(ok: &Cell<Vec<String>>, cond: bool, what: &str) {
    if !cond {
        let mut v = ok.take();
        v.push(what.to_string());
        ok.set(v);
    }
}

fn whos(save: &Save) -> Vec<Who> {
    let mut out = vec![Who::Player];
    out.extend(save.npcs().into_iter().map(Who::Npc));
    out
}

fn label(game: Game, name: &str, who: Who) -> String {
    format!("{:?} {} {:?}", game, name, who)
}

/// The engine's rules, as the editor computes them, give the maxima the game wrote into
/// every save it wrote. (Saves the editor wrote, named KMRP..., are left out: before issue
/// 70 the editor left their stored maxima stale, which the game ignores.)
#[test]
fn computed_maxima_match_what_the_game_wrote() {
    let installs = find_installs(&[]);
    let mut checked = 0;
    for game in [Game::K1, Game::K2] {
        if !installs.iter().any(|i| i.game == game) {
            continue;
        }
        for folder in find_saves(&installs, game, &[]) {
            let name = folder.path.file_name().unwrap().to_string_lossy().to_string();
            if name.contains("KMRP") {
                continue;
            }
            let save = Save::open(&folder.path).unwrap();
            for who in whos(&save) {
                let c = save.creature(who).unwrap();
                let what = label(game, &name, who);
                if let Some(max) = c.int("MaxHitPoints") {
                    assert_eq!(creature::max_health(c), max, "{}: maximum Health", what);
                }
                if let Some(max) = c.int("MaxForcePoints") {
                    assert_eq!(creature::max_force(c, game), max, "{}: maximum Force", what);
                }
                // The shown current value reads back through the stored one unchanged.
                let mut copy = c.clone();
                let (hp, fp) = (creature::health(c), creature::force(c, game));
                if hp.0 >= 1 && hp.0 <= hp.1 {
                    creature::set_current_health(&mut copy, hp.0);
                    assert_eq!(copy.int("CurrentHitPoints"), c.int("CurrentHitPoints"), "{}: Health stored as the game stores it", what);
                }
                if fp.0 >= 0 && fp.0 <= fp.1 {
                    creature::set_current_force(&mut copy, game, fp.0);
                    assert_eq!(copy.int("CurrentForce"), c.int("CurrentForce"), "{}: Force stored as the game stores it", what);
                }
                checked += 1;
            }
        }
    }
    eprintln!("{} creatures checked against the game's own maxima", checked);
}

/// One scenario on a fresh copy of a save: `edit` changes every character, the save is
/// written and read back, and `check` sees each character before and after.
fn on_every_save(tag: &str, edit: &dyn Fn(&mut Save, &Catalog, Who) -> bool, check: &dyn Fn(&str, Game, &Struct, &Struct)) -> usize {
    let installs = find_installs(&[]);
    let mut done = 0;
    for game in [Game::K1, Game::K2] {
        let Some(install) = installs.iter().find(|i| i.game == game) else { continue };
        let data = GameData::open(game, &install.path);
        let cat = Catalog::build(&data);
        for folder in find_saves(&installs, game, &[]) {
            let name = folder.path.file_name().unwrap().to_string_lossy().to_string();
            let work = scratch(&format!("{}-{:?}-{}", tag, game, name));
            copy_dir(&folder.path, &work);
            let backups = scratch(&format!("{}-{:?}-{}-backups", tag, game, name));
            let before = Save::open(&work).unwrap();
            let mut save = Save::open(&work).unwrap();
            let pc_before = pc_copy(&work);
            let mut edited = Vec::new();
            for who in whos(&save) {
                if edit(&mut save, &cat, who) {
                    edited.push(who);
                }
            }
            if edited.is_empty() {
                continue;
            }
            save.write(&backups).unwrap();
            let after = Save::open(&work).unwrap();
            for who in edited {
                let what = label(game, &name, who);
                let (b, a) = (before.creature(who).unwrap(), after.creature(who).unwrap());
                assert_consistent(a, game, &what);
                check(&what, game, b, a);
                // Every copy says the same as the one the editor shows.
                let copies: Vec<(Struct, Struct)> = match who {
                    Who::Player => pc_before.clone().into_iter().zip(pc_copy(&work)).collect(),
                    Who::Npc(_) => {
                        let tag = b.string("Tag").unwrap_or_default();
                        area_copies(&before, &tag).into_iter().zip(area_copies(&after, &tag)).collect()
                    }
                };
                for (cb, ca) in copies {
                    assert_consistent(&ca, game, &format!("{} (other copy)", what));
                    if creature::health(&cb) == creature::health(b) && creature::force(&cb, game) == creature::force(b, game) && shape(&cb, game) == shape(b, game) {
                        assert_eq!(creature::health(&ca), creature::health(a), "{}: copies agree on Health", what);
                        assert_eq!(creature::force(&ca, game), creature::force(a, game), "{}: copies agree on Force", what);
                    }
                }
            }
            done += 1;
        }
    }
    done
}

fn class_of(cat: &Catalog, c: &Struct, index: usize) -> Option<Class> {
    creature::classes(c).get(index).and_then(|(id, _)| cat.class(*id)).cloned()
}

fn xp(cat: &Catalog) -> impl Fn(i64) -> Option<i64> + '_ {
    move |l| cat.xp_for(l)
}

/// Spends half the Health and Force (shown values), as a fight would.
fn spend_half(c: &mut Struct, game: Game) {
    let (_, hp) = creature::health(c);
    creature::set_current_health(c, (hp / 2).max(1));
    let (_, fp) = creature::force(c, game);
    creature::set_current_force(c, game, fp / 2);
}

/// Direct edits: current values alone, then maximum and current together, read back.
#[test]
fn health_and_force_set_directly() {
    let n = on_every_save(
        "direct",
        &|save, cat, who| {
            let game = cat.game;
            save.edit(who, "hp", |c| {
                let (_, max) = creature::health(c);
                creature::set_max_health(c, max + 7);
                creature::set_current_health(c, max - 3);
            });
            save.edit(who, "fp", |c| {
                let (_, max) = creature::force(c, game);
                if max > 0 {
                    creature::set_max_force(c, game, max + 5);
                    creature::set_current_force(c, game, max / 2);
                }
            });
            true
        },
        &|what, game, b, a| {
            let (_, max) = creature::health(b);
            assert_eq!(creature::health(a), ((max - 3).max(1).min(max + 7), max + 7), "{}: Health as set", what);
            let (_, fmax) = creature::force(b, game);
            if fmax > 0 {
                let (fcur, fnew) = creature::force(a, game);
                assert_eq!(fnew, fmax + 5, "{}: maximum Force as set", what);
                assert_eq!(fcur, fmax / 2, "{}: current Force as set", what);
            } else {
                assert_eq!(creature::force(a, game).1, 0, "{}: no Force stays none", what);
            }
            // Levels and classes untouched.
            assert_eq!(creature::classes(a), creature::classes(b), "{}", what);
        },
    );
    eprintln!("{} saves", n);
}

/// Raising a level: Health full, Force up by what its maximum rose, one record per level
/// for the player, the base up by the die; lowering it again gives everything back.
#[test]
fn levels_up_then_down() {
    let n = on_every_save(
        "updown",
        &|save, cat, who| {
            let game = cat.game;
            let Some(class) = save.creature(who).and_then(|c| class_of(cat, c, 0)) else { return false };
            let xp_for = xp(cat);
            let ok = Cell::new(Vec::<String>::new());
            save.edit(who, "updown", |c| {
                spend_half(c, game);
                let start = shape(c, game);
                let (hp0, fp0) = (creature::health(c), creature::force(c, game));
                let lvl = creature::classes(c)[0].1;
                creature::set_class_level(c, 0, lvl + 2, &class, &xp_for);
                let up = shape(c, game);
                let (hp1, fp1) = (creature::health(c), creature::force(c, game));
                // The game's level-up: Health full, Force up by the maximum's rise.
                note(&ok, hp1.0 == hp1.1, "hp1.0 == hp1.1");
                note(&ok, fp1.0 == (fp0.0 + fp1.1 - fp0.1).max(0), "fp1.0 == (fp0.0 + fp1.1 - fp0.1).max(0)");
                note(&ok, up.hit_points == start.hit_points.map(|v| v + 2 * class.hit_die), "up.hit_points == start.hit_points.map(|v| v + 2 * class.hit_die)");
                if let (Some(r0), Some(r1)) = (&start.records, &up.records) {
                    note(&ok, r1.len() == r0.len() + 2, "r1.len() == r0.len() + 2");
                    note(&ok, r1[r0.len()..].iter().all(|r| r.int("LvlStatHitDie") == Some(class.hit_die) && r.int("LvlStatForce") == Some(class.force_die) && r.int("LvlStatClass") == Some(class.id)), "r1[r0.len()..].iter().all(|r| r.int('LvlStatHitDie') == Some(class.hit_die) && r.int('LvlStatForce') == Some(class.force_die) && r.int('LvlStatClass') == Some(class.id))");
                }
                creature::set_class_level(c, 0, lvl, &class, &xp_for);
                let down = shape(c, game);
                note(&ok, down == start, "down == start");
                // Down again: Health kept full (nothing missing), Force back where it was.
                let (hp2, fp2) = (creature::health(c), creature::force(c, game));
                note(&ok, hp2 == (hp0.1, hp0.1), "hp2 == (hp0.1, hp0.1)");
                note(&ok, fp2 == fp0 || fp1.0 == 0, "fp2 == fp0 || fp1.0 == 0");
            });
            let failed = ok.take();
            assert!(failed.is_empty(), "{:?}: level up then down: {:?}", who, failed);
            // Leave it two levels up, half spent, for the written check.
            save.edit(who, "up", |c| {
                let lvl = creature::classes(c)[0].1;
                creature::set_class_level(c, 0, lvl + 2, &class, &xp_for);
                spend_half(c, game);
            });
            true
        },
        &|what, game, b, a| {
            assert_eq!(creature::level(a), creature::level(b) + 2, "{}", what);
            let (hp, max) = creature::health(a);
            assert_eq!(hp, (max / 2).max(1), "{}: half Health after the write", what);
            let (fp, fmax) = creature::force(a, game);
            assert_eq!(fp, fmax / 2, "{}: half Force after the write", what);
        },
    );
    eprintln!("{} saves", n);
}

/// A class added (a Jedi class to a creature without one, an ordinary one to a Jedi) and
/// removed again; and a class changed to another and back.
#[test]
fn classes_added_removed_and_changed() {
    let n = on_every_save(
        "classes",
        &|save, cat, who| {
            let game = cat.game;
            let Some(c0) = save.creature(who).cloned() else { return false };
            let xp_for = xp(cat);
            let jedi = creature::classes(&c0).iter().any(|(id, _)| cat.is_force_class(*id));
            let mut touched = false;
            // Add and remove (single-class creatures).
            if creature::classes(&c0).len() == 1 {
                if let Some(new) = other_class(cat, &c0, !jedi) {
                    let ok = Cell::new(Vec::<String>::new());
                    save.edit(who, "addremove", |c| {
                        spend_half(c, game);
                        let start = shape(c, game);
                        let fp0 = creature::force(c, game);
                        let need = cat.xp_for(creature::level(c) + 1);
                        let classes_before = creature::classes(c);
                        let added = creature::add_class(c, &new, need);
                        note(&ok, added, &format!("add_class to a creature with classes {:?}, tag {:?}, IsPC {:?}", classes_before, c.string("Tag"), c.int("IsPC")));
                        let (hp1, fp1) = (creature::health(c), creature::force(c, game));
                        note(&ok, hp1.0 == hp1.1, "hp1.0 == hp1.1");
                        // A party member whose base plus level times modifier is no more than
                        // the level loads its current Force as the level at most (the engine's
                        // load, 006AFED0 / 005B1254), so a maximum the new class's granted feats
                        // raise above that (Force Sensitive: +40) cannot be stored full.
                        let floor_rule = !creature::uses_level_records(c) && fp1.0 == creature::level(c) && fp1.1 > fp1.0;
                        note(&ok, fp1.0 == (fp0.0 + fp1.1 - fp0.1).max(0) || floor_rule, &format!("fp1.0 == (fp0.0 + fp1.1 - fp0.1).max(0): {:?} -> {:?}", fp0, fp1));
                        note(&ok, creature::level(c) == start.classes[0].1 + 1, "creature::level(c) == start.classes[0].1 + 1");
                        if let Some(r) = &start.records {
                            note(&ok, c.list("LvlStatList").unwrap().len() == r.len() + 1, "c.list('LvlStatList').unwrap().len() == r.len() + 1");
                        }
                        // A Force class added after an ordinary one gives the player Force;
                        // an ordinary class added after a Jedi one takes it away (the
                        // engine counts the newest class).
                        if creature::uses_level_records(c) {
                            note(&ok, (fp1.1 > 0) == (new.force_die > 0), "(fp1.1 > 0) == (new.force_die > 0)");
                        }
                        creature::remove_class(c, 1, &new, &xp_for);
                        note(&ok, shape(c, game) == start, "shape(c, game) == start");
                        note(&ok, creature::force(c, game) == fp0 || fp1.0 == 0, "creature::force(c, game) == fp0 || fp1.0 == 0");
                    });
                    let failed = ok.take();
            assert!(failed.is_empty(), "{:?}: class {} added and removed: {:?}", who, new.id, failed);
                    // Leave it added for the written check.
                    save.edit(who, "add", |c| {
                        let need = cat.xp_for(creature::level(c) + 1);
                        creature::add_class(c, &new, need);
                    });
                    touched = true;
                }
            }
            // Change and change back (every creature, its first class).
            let Some(c1) = save.creature(who).cloned() else { return touched };
            let Some(old) = class_of(cat, &c1, 0) else { return touched };
            // A save whose records do not match its classes (another editor's) cannot be
            // changed back exactly: the records of the two classes mix.
            if !creature::level_records_match(&c1) {
                return touched;
            }
            if let Some(new) = other_class(cat, &c1, old.force_die > 0) {
                let ok = Cell::new(Vec::<String>::new());
                save.edit(who, "change", |c| {
                    let start = shape(c, game);
                    let (hp0, fp0) = (creature::health(c), creature::force(c, game));
                    creature::change_class(c, 0, &old, &new);
                    let (hp1, fp1) = (creature::health(c), creature::force(c, game));
                    // What was missing stays missing.
                    note(&ok, hp1.1 - hp1.0 == hp0.1 - hp0.0 || hp1.0 == 1, "hp1.1 - hp1.0 == hp0.1 - hp0.0 || hp1.0 == 1");
                    note(&ok, fp1.1 - fp1.0 == fp0.1 - fp0.0 || fp1.0 == 0, "fp1.1 - fp1.0 == fp0.1 - fp0.0 || fp1.0 == 0");
                    creature::change_class(c, 0, &new, &old);
                    note(&ok, shape(c, game) == start, "shape(c, game) == start");
                    note(&ok, creature::health(c) == hp0 || hp1.0 == 1, "creature::health(c) == hp0 || hp1.0 == 1");
                    note(&ok, creature::force(c, game) == fp0 || fp1.0 == 0, "creature::force(c, game) == fp0 || fp1.0 == 0");
                });
                let failed = ok.take();
            assert!(failed.is_empty(), "{:?}: class {} changed to {} and back: {:?}", who, old.id, new.id, failed);
                touched = true;
            }
            touched
        },
        &|what, game, b, a| {
            assert!(creature::level(a) >= creature::level(b), "{}", what);
            let _ = game;
        },
    );
    eprintln!("{} saves", n);
}

/// Taking levels away newest first, as Skip Ahead's Go Back does, from two levels up: the
/// creature ends where it started.
#[test]
fn levels_taken_away_newest_first() {
    let n = on_every_save(
        "rewind",
        &|save, cat, who| {
            if who != Who::Player {
                return false;
            }
            let game = cat.game;
            let xp_for = xp(cat);
            let Some(c0) = save.creature(who).cloned() else { return false };
            let Some(first) = class_of(cat, &c0, 0) else { return false };
            let ok = Cell::new(Vec::<String>::new());
            save.edit(who, "rewind", |c| {
                let start = shape(c, game);
                let target = creature::level(c);
                let lvl = creature::classes(c)[0].1;
                creature::set_class_level(c, 0, lvl + 2, &first, &xp_for);
                while creature::level(c) > target {
                    let last = c.list("LvlStatList").and_then(|l| l.last()).and_then(|r| r.int("LvlStatClass")).unwrap_or(first.id);
                    let classes = creature::classes(c);
                    let Some(index) = classes.iter().position(|(id, _)| *id == last) else { break };
                    let class = cat.class(last).cloned().unwrap();
                    creature::set_class_level(c, index, classes[index].1 - 1, &class, &xp_for);
                }
                note(&ok, shape(c, game) == start, "shape(c, game) == start");
            });
            let failed = ok.take();
            assert!(failed.is_empty(), "rewind gives the creature back: {:?}", failed);
            true
        },
        &|what, game, b, a| {
            assert_eq!(shape(a, game).records, shape(b, game).records, "{}", what);
            assert_eq!(creature::max_health(a), creature::max_health(b), "{}", what);
        },
    );
    eprintln!("{} saves", n);
}

/// Healing: full Health and Force as the game shows them, for everyone, including party
/// members with Toughness or Force Sensitive, whose stored value the engine reads without
/// those bonuses.
#[test]
fn healing_fills_both() {
    let n = on_every_save(
        "heal",
        &|save, cat, who| {
            let game = cat.game;
            save.edit(who, "spend", |c| spend_half(c, game));
            save.edit(who, "heal", |c| creature::heal(c, game));
            true
        },
        &|what, game, _b, a| {
            let (hp, max) = creature::health(a);
            assert_eq!(hp, max, "{}: full Health", what);
            let (fp, fmax) = creature::force(a, game);
            assert_eq!(fp, fmax, "{}: full Force", what);
        },
    );
    eprintln!("{} saves", n);
}

/// The two rules on made-up creatures, including the corners no save on this machine
/// reaches: a party member whose base plus level times Constitution modifier is no more
/// than the level, the Toughness feats, ability effects, the player's newest class.
#[test]
fn rules_on_made_up_creatures() {
    fn short(v: i64) -> Value {
        Value::Short(v as i16)
    }
    fn make(pc: bool, classes: &[(i64, i64)], con: i64, base_hp: i64, base_fp: i64) -> Struct {
        let mut c = Struct::new(0);
        c.set("IsPC", Value::Byte(pc as u8));
        for (a, v) in [("Str", 10), ("Dex", 10), ("Con", con), ("Int", 10), ("Wis", 14), ("Cha", 12)] {
            c.set(a, Value::Byte(v as u8));
        }
        let list = classes
            .iter()
            .map(|(id, l)| {
                let mut e = Struct::new(2);
                e.set("Class", Value::Int(*id as i32));
                e.set("ClassLevel", Value::Short(*l as i16));
                e
            })
            .collect();
        c.set("ClassList", Value::List(list));
        c.set("HitPoints", short(base_hp));
        c.set("CurrentHitPoints", short(base_hp));
        c.set("MaxHitPoints", short(0));
        c.set("ForcePoints", short(base_fp));
        c.set("CurrentForce", short(base_fp));
        c.set("MaxForcePoints", short(0));
        if pc {
            let mut recs = Vec::new();
            for (id, l) in classes {
                for _ in 0..*l {
                    let mut r = Struct::new(0);
                    let (hd, fd) = match id {
                        0 => (10, 0),
                        3 => (10, 4),
                        _ => (8, 0),
                    };
                    r.set("LvlStatHitDie", Value::Byte(hd));
                    r.set("LvlStatForce", Value::Byte(fd));
                    r.set("LvlStatClass", Value::Byte(*id as u8));
                    recs.push(r);
                }
            }
            c.set("LvlStatList", Value::List(recs));
        }
        c
    }
    fn feat(c: &mut Struct, f: i64) {
        creature::add_feat(c, f);
    }

    // The player, Soldier 2 / Jedi Guardian 3, Con 14 (+2), Wis 14 (+2), Cha 12 (+1).
    let mut p = make(true, &[(0, 2), (3, 3)], 14, 0, 0);
    assert_eq!(creature::max_health(&p), 5 * 12);
    // KotOR 1: Force from the three Jedi records, max(1, 4 + 2 + 1) each.
    assert_eq!(creature::max_force(&p, Game::K1), 21);
    // KotOR 2: Wisdom alone.
    assert_eq!(creature::max_force(&p, Game::K2), 18);
    feat(&mut p, 84);
    assert_eq!(creature::max_health(&p), 5 * 13, "Toughness, 1 a level");
    feat(&mut p, 124);
    assert_eq!(creature::max_health(&p), 5 * 14, "feat 124 instead, 2 a level");
    feat(&mut p, 116);
    assert_eq!(creature::max_force(&p, Game::K1), 61, "feat 116 adds 40");
    // An ability effect (type 36: Constitution +2) raises the modifier.
    let mut e = Struct::new(0);
    e.set("Type", Value::Dword(36));
    let ints = [2i64, 2].iter().map(|v| {
        let mut s = Struct::new(0);
        s.set("Value", Value::Int(*v as i32));
        s
    });
    e.set("IntList", Value::List(ints.collect()));
    p.set("EffectList", Value::List(vec![e]));
    assert_eq!(creature::max_health(&p), 5 * 15);
    // Current values: stored relative to the base.
    creature::set_current_health(&mut p, 40);
    assert_eq!(p.int("CurrentHitPoints"), Some(40 - 75 + 0));
    assert_eq!(creature::health(&p), (40, 75));
    // The player's newest class decides: an ordinary class last, no Force at all.
    let q = make(true, &[(3, 3), (0, 2)], 14, 0, 0);
    assert_eq!(creature::max_force(&q, Game::K1), 0);

    // A party member, Jedi Guardian 4, Con 14, base 40 Health and 16 Force.
    let mut m = make(false, &[(3, 4)], 14, 40, 16);
    assert_eq!(creature::max_health(&m), 40 + 4 * 2);
    assert_eq!(creature::max_force(&m, Game::K1), 16 + 4 * 3);
    feat(&mut m, 84);
    feat(&mut m, 95);
    assert_eq!(creature::max_health(&m), 40 + 4 * (2 + 1 + 2), "Toughness and feat 95 per level");
    // Its current value is read without those feats: full means the stored value carries
    // them.
    creature::heal(&mut m, Game::K1);
    assert_eq!(creature::health(&m), (60, 60));
    assert_eq!(m.int("CurrentHitPoints"), Some(60 - 4 * 2));
    // Base plus level times modifier no more than the level: Con 3 (-4).
    let mut low = make(false, &[(0, 4)], 3, 10, 0);
    assert_eq!(creature::max_health(&low), 4, "the level, 1 a level");
    creature::set_current_health(&mut low, 2);
    assert_eq!(creature::health(&low), (2, 4));
    creature::set_current_health(&mut low, 4);
    assert_eq!(creature::health(&low), (4, 4));
    // A creature without a Jedi class has no Force, whatever its base says.
    let n = make(false, &[(0, 4)], 14, 40, 30);
    assert_eq!(creature::max_force(&n, Game::K1), 0);
    // Droids (race 5) neither.
    let mut d = make(false, &[(3, 4)], 14, 40, 30);
    d.set("Race", Value::Word(5));
    assert_eq!(creature::max_force(&d, Game::K2), 0);
}

/// A companion recruited from the game's template (Skip Ahead): the stored maxima as the
/// game would write them, the current values the template's, so the game loads the member
/// exactly as its own AddAvailableNPCByTemplate would.
#[test]
fn recruited_companions_keep_the_templates_values() {
    let installs = find_installs(&[]);
    let mut checked = 0;
    for (game, npc, resref) in [(Game::K1, 2usize, "p_carth"), (Game::K2, 0usize, "p_atton")] {
        let Some(install) = installs.iter().find(|i| i.game == game) else { continue };
        let data = GameData::open(game, &install.path);
        let Some(template) = data.gff(resref, "utc", RES_TYPE_UTC) else { continue };
        for folder in find_saves(&installs, game, &[]) {
            let name = folder.path.file_name().unwrap().to_string_lossy().to_string();
            let work = scratch(&format!("recruit-{:?}-{}", game, name));
            copy_dir(&folder.path, &work);
            let backups = scratch(&format!("recruit-{:?}-{}-backups", game, name));
            let mut save = Save::open(&work).unwrap();
            if save.npcs().contains(&npc) && save.recruited(npc) {
                continue;
            }
            save.recruit_from_template(npc, &template.root).unwrap();
            save.write(&backups).unwrap();
            let after = Save::open(&work).unwrap();
            let c = after.creature(Who::Npc(npc)).unwrap();
            let what = format!("{:?} {} {}", game, name, resref);
            assert_consistent(c, game, &what);
            assert_eq!(creature::health(c), creature::health(&template.root), "{}: Health as the template loads", what);
            assert_eq!(creature::force(c, game), creature::force(&template.root, game), "{}: Force as the template loads", what);
            assert_eq!(c.int("CurrentHitPoints"), template.root.int("CurrentHitPoints"), "{}", what);
            checked += 1;
        }
    }
    eprintln!("{} recruits checked", checked);
}

/// KotOR 2's AddMultiClass (a_makejedi, the companions' Jedi training) as the engine runs it
/// on a party member (swkotor2.exe 0066C7A0): the class at level 0, no hit or Force die, the
/// XP for the next level and the same amount on JoiningXP, MClassLevUpIn on the new class;
/// Health untouched, Force's current value kept while its maximum now counts the Jedi class.
#[test]
fn companions_trained_as_the_engine_does() {
    let installs = find_installs(&[]);
    let k2 = installs.iter().find(|i| i.game == Game::K2).map(|i| Catalog::build(&GameData::open(Game::K2, &i.path)));

    // A made-up party member: Scoundrel 8, Con 14, Wis 12 (+1), 2000 XP short of level 9.
    let sentinel = Class { id: 5, name: "Jedi Sentinel".into(), hit_die: 8, force_die: 4, droid: false, game: Game::K2, grants: Default::default() };
    let mut m = Struct::new(0);
    m.set("IsPC", Value::Byte(0));
    for (a, v) in [("Str", 10), ("Dex", 14), ("Con", 14), ("Int", 12), ("Wis", 12), ("Cha", 12)] {
        m.set(a, Value::Byte(v));
    }
    let mut e = Struct::new(2);
    e.set("Class", Value::Int(2));
    e.set("ClassLevel", Value::Short(8));
    m.set("ClassList", Value::List(vec![e]));
    for (l, v) in [("HitPoints", 60), ("CurrentHitPoints", 40), ("MaxHitPoints", 76), ("ForcePoints", 0), ("CurrentForce", 0)] {
        m.set(l, Value::Short(v));
    }
    m.set("Experience", Value::Dword(34000));
    m.set("JoiningXP", Value::Int(500));
    let hp0 = creature::health(&m);
    let fp0 = creature::force(&m, Game::K2);
    assert_eq!(fp0.1, 0, "no Force before the training");
    assert!(creature::add_multiclass(&mut m, &sentinel, Some(36000)));
    assert_eq!(creature::classes(&m), vec![(2, 8), (5, 0)], "the class at level 0");
    assert_eq!(creature::level(&m), 8, "the total level unchanged");
    assert_eq!(m.int("HitPoints"), Some(60));
    assert_eq!(m.int("ForcePoints"), Some(0));
    assert_eq!(creature::health(&m), hp0, "Health untouched");
    let fp1 = creature::force(&m, Game::K2);
    assert_eq!(fp1.1, 8, "the Force maximum counts the Jedi class: 0 + 8 x 1");
    assert_eq!(fp1.0, fp0.0, "the current Force kept");
    assert_eq!(m.int("Experience"), Some(36000), "the XP for the next level");
    assert_eq!(m.int("JoiningXP"), Some(2500), "JoiningXP takes the same 2000");
    assert_eq!(m.int("MClassLevUpIn"), Some(1));
    assert_eq!(m.int("MaxForcePoints"), Some(8));
    let entry = &m.list("ClassList").unwrap()[1];
    assert!(entry.list("KnownList0").is_some() && entry.list("SpellsPerDayList").is_some());
    assert!(matches!(entry.get("ClassLevel"), Some(Value::Short(0))));
    // A second time, or a third class: nothing.
    assert!(!creature::add_multiclass(&mut m, &sentinel, Some(36000)));
    // XP already past the next level: the engine keeps it, JoiningXP still takes the
    // (negative) difference.
    let mut over = m.clone();
    over.list_mut("ClassList").unwrap().pop();
    over.set("Experience", Value::Dword(37000));
    assert!(creature::add_multiclass(&mut over, &sentinel, Some(36000)));
    assert_eq!(over.int("Experience"), Some(37000));
    assert_eq!(over.int("JoiningXP"), Some(2500 - 1000));

    // Every KotOR 2 save on this machine: every party member without a Jedi class, written
    // and read back.
    let Some(cat) = k2 else { return };
    let Some(class) = cat.class(5).cloned() else { return };
    let n = on_every_save(
        "multiclass",
        &|save, c2, who| {
            if c2.game != Game::K2 || who == Who::Player {
                return false;
            }
            let Some(c0) = save.creature(who).cloned() else { return false };
            if creature::classes(&c0).len() != 1 || creature::classes(&c0).iter().any(|(id, _)| c2.is_force_class(*id)) || c0.int("Race") == Some(5) {
                return false;
            }
            save.edit(who, "multiclass", |c| {
                let next = c2.xp_for(creature::level(c) + 1);
                assert!(creature::add_multiclass(c, &class, next));
            });
            true
        },
        &|what, game, b, a| {
            assert_eq!(game, Game::K2);
            let mut want = creature::classes(b);
            want.push((5, 0));
            assert_eq!(creature::classes(a), want, "{}", what);
            assert_eq!(a.int("HitPoints"), b.int("HitPoints"), "{}", what);
            assert_eq!(a.int("ForcePoints"), b.int("ForcePoints"), "{}", what);
            assert_eq!(creature::health(a), creature::health(b), "{}: Health untouched", what);
            assert_eq!(creature::force(a, game).0, creature::force(b, game).0.max(0), "{}: current Force kept", what);
            assert!(creature::force(a, game).1 > 0, "{}: a Jedi now", what);
            let need = cat.xp_for(creature::level(b) + 1).unwrap();
            let xb = creature::experience(b);
            assert_eq!(creature::experience(a), xb.max(need), "{}", what);
            assert_eq!(a.int("JoiningXP").unwrap_or(0), b.int("JoiningXP").unwrap_or(0) + need - xb, "{}", what);
            assert_eq!(a.int("MClassLevUpIn"), Some(1), "{}", what);
        },
    );
    eprintln!("{} saves", n);
}

/// The ends of what a level allows (the editor's "Set to" in its out-of-reach pop-up): the
/// highest and lowest most Health and Force, found by asking for far more and far less,
/// set exactly when asked for themselves, and a value past either end stops there.
#[test]
fn the_ends_of_a_level_set_exactly() {
    let installs = find_installs(&[]);
    let mut checked = 0;
    for game in [Game::K1, Game::K2] {
        if !installs.iter().any(|i| i.game == game) {
            continue;
        }
        for folder in find_saves(&installs, game, &[]) {
            let name = folder.path.file_name().unwrap().to_string_lossy().to_string();
            let save = Save::open(&folder.path).unwrap();
            for who in whos(&save) {
                let c = save.creature(who).unwrap();
                let what = label(game, &name, who);
                let health_at = |v: i64| {
                    let mut t = c.clone();
                    creature::set_max_health(&mut t, v);
                    creature::max_health(&t)
                };
                let force_at = |v: i64| {
                    let mut t = c.clone();
                    creature::set_max_force(&mut t, game, v);
                    creature::max_force(&t, game)
                };
                for (pool, at) in [("Health", &health_at as &dyn Fn(i64) -> i64), ("Force", &force_at)] {
                    let (lo, hi) = (at(i16::MIN as i64), at(i16::MAX as i64));
                    assert!(lo <= hi, "{}: {} range {}..{}", what, pool, lo, hi);
                    assert_eq!(at(hi), hi, "{}: the highest most {} sets exactly", what, pool);
                    assert_eq!(at(lo), lo, "{}: the lowest most {} sets exactly", what, pool);
                    assert_eq!(at(hi + 100), hi, "{}: past the highest most {} stops there", what, pool);
                }
                checked += 1;
            }
        }
    }
    eprintln!("{} creatures checked at both ends", checked);
}
