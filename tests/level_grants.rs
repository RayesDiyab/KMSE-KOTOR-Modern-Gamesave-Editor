//! What the game's own level-up gives without asking, and KotOR 2's AddMultiClass on the
//! player (Skip Ahead's prestige class): the editor against the engine's rules
//! (swkotor.exe 005B27E0; swkotor2.exe 006BEDF0, 0066C7A0, 005FE0C0, 0058A9F0) and the
//! level records the game wrote. On copies of the saves on this machine; the real saves
//! are only read.

use kse_core::catalog::Catalog;
use kse_core::creature;
use kse_core::formats::gff::{Struct, Value};
use kse_core::gamedata::GameData;
use kse_core::games::{find_installs, find_saves, Game};
use kse_core::globals::{self, GlobalValue};
use kse_core::questdb::{add_multiclass_to_player, experience_share, mentor_bonus};
use kse_core::save::{Save, Who};
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
    let dir = std::env::temp_dir().join("kmse-tests").join("level-grants").join(name);
    let _ = fs::remove_dir_all(&dir);
    dir
}

/// DistributeExperience's arithmetic, by hand: the Mentor bonus and the shares, single
/// precision and rounded up.
#[test]
fn experience_shares_as_the_engine_computes_them() {
    assert_eq!(mentor_bonus(0, 1000), 0, "no Mentor");
    assert_eq!(mentor_bonus(6, 1000), 30);
    assert_eq!(mentor_bonus(7, 1000), 50);
    assert_eq!(mentor_bonus(13, 1000), 70);
    assert_eq!(mentor_bonus(7, 1001), 51, "50.05 rounded up");
    assert_eq!(experience_share(None, 100, 1052), 1052, "the player: GAME_XP_General 100");
    assert_eq!(experience_share(Some(80), 100, 1052), 842, "a companion: 841.6 rounded up");
    assert_eq!(experience_share(Some(80), 100, 1050), 840, "0.8f x 1050 is 840.0 in single precision");
    assert_eq!(experience_share(Some(0), 100, 1050), 0);
}

/// The player's prestige class through AddMultiClass (a_pc_jedi), on copies of every KotOR 2
/// save whose player has one Jedi class: the class at level 0, the XP for the next level
/// handed out through the party with the Mentor bonus, PT_XP_POOL and G_PC_LEVEL.
#[test]
fn prestige_class_as_the_engine_adds_it() {
    let installs = find_installs(&[]);
    let Some(install) = installs.iter().find(|i| i.game == Game::K2) else { return };
    let data = GameData::open(Game::K2, &install.path);
    let cat = Catalog::build(&data);
    let class = cat.class(12).cloned().unwrap();
    let npc = data.table("npc");
    let mut checked = 0;
    for folder in find_saves(&installs, Game::K2, &[]) {
        let name = folder.path.file_name().unwrap().to_string_lossy().to_string();
        let work = scratch(&format!("prestige-{}", name));
        copy_dir(&folder.path, &work);
        let backups = scratch(&format!("prestige-{}-backups", name));
        let mut save = Save::open(&work).unwrap();
        let Some(p) = save.player().cloned() else { continue };
        if creature::classes(&p).len() != 1 || !cat.is_force_class(creature::classes(&p)[0].0) {
            continue;
        }
        let (lvl, xp) = (creature::level(&p), creature::experience(&p));
        let delta = cat.xp_for(lvl + 1).unwrap() - xp;
        let members: Vec<usize> = save.party_members().into_iter().filter(|m| save.creature(Who::Npc(*m)).is_some()).collect();
        let mentor = members.iter().filter_map(|m| save.creature(Who::Npc(*m))).find(|c| creature::feats(c).contains(&202)).map(creature::level).unwrap_or(0);
        let total = delta + mentor_bonus(mentor, delta);
        let npc_xp: Vec<(usize, i64)> = members.iter().map(|m| (*m, creature::experience(save.creature(Who::Npc(*m)).unwrap()))).collect();
        let pool = save.party_int("PT_XP_POOL").unwrap_or(0);
        let handed = add_multiclass_to_player(&mut save, &data, &cat, &class);
        assert_eq!(handed, delta.max(0), "{}", name);
        save.write(&backups).unwrap();
        let after = Save::open(&work).unwrap();
        let a = after.player().unwrap();
        assert_eq!(creature::classes(a)[1], (12, 0), "{}: the class at level 0", name);
        assert_eq!(a.int("MClassLevUpIn"), Some(1));
        assert_eq!(a.list("LvlStatList").map(|l| l.len()), p.list("LvlStatList").map(|l| l.len()), "{}: no level record", name);
        assert_eq!(creature::health(a), creature::health(&p), "{}: Health untouched", name);
        if delta > 0 {
            assert_eq!(creature::experience(a), xp + total, "{}: the player's share", name);
            assert_eq!(after.party_int("PT_XP_POOL"), Some(pool + delta), "{}", name);
            for (m, before) in npc_xp {
                let pct = npc.int(m, "percentxp").unwrap();
                let want = before + experience_share(Some(pct), 100, total);
                assert_eq!(creature::experience(after.creature(Who::Npc(m)).unwrap()), want, "{}: member {}", name, m);
            }
            let g = after.globals.as_ref().map(|g| globals::list(&g.root)).unwrap_or_default();
            let level = g.iter().find(|g| g.name.eq_ignore_ascii_case("G_PC_LEVEL")).map(|g| g.value.clone());
            assert_eq!(level, Some(GlobalValue::Number(lvl + 1)), "{}: G_PC_LEVEL", name);
        }
        checked += 1;
    }
    eprintln!("{} saves", checked);
}

fn made_up(game: Game, pc: bool, tag: &str, class: i64, level: i64, int: u8) -> Struct {
    let mut c = Struct::new(0);
    c.set("IsPC", Value::Byte(pc as u8));
    c.set("Tag", Value::String(tag.into()));
    for (a, v) in [("Str", 14), ("Dex", 14), ("Con", 12), ("Int", int), ("Wis", 12), ("Cha", 10)] {
        c.set(a, Value::Byte(v));
    }
    let mut e = Struct::new(2);
    e.set("Class", Value::Int(class as i32));
    e.set("ClassLevel", Value::Short(level as i16));
    c.set("ClassList", Value::List(vec![e]));
    c.set("FeatList", Value::List(Vec::new()));
    c.set("SkillPoints", Value::Word(1));
    for (l, v) in [("HitPoints", 50), ("CurrentHitPoints", 50), ("ForcePoints", 20), ("CurrentForce", 20)] {
        c.set(l, Value::Short(v));
    }
    if pc {
        let recs = (0..level)
            .map(|_| {
                let mut r = Struct::new(0);
                r.set("LvlStatClass", Value::Byte(class as u8));
                r.set("LvlStatHitDie", Value::Byte(10));
                r.set("LvlStatForce", Value::Byte(if game == Game::K1 { 4 } else { 4 }));
                r.set("SkillPoints", Value::Word(1));
                r.set("FeatList", Value::List(Vec::new()));
                r
            })
            .collect();
        c.set("LvlStatList", Value::List(recs));
    }
    c
}

/// A direct raise gives what the game's level-up gives before any choice: the feats the
/// level grants on its own (in FeatList and the player's level record), the skill points
/// banked; lowering takes exactly that back.
#[test]
fn raised_levels_bring_their_granted_feats_and_points() {
    let installs = find_installs(&[]);
    let mut checked = 0;
    for game in [Game::K1, Game::K2] {
        let Some(install) = installs.iter().find(|i| i.game == game) else { continue };
        let data = GameData::open(game, &install.path);
        let cat = Catalog::build(&data);
        let xp_for = |l| cat.xp_for(l);
        // Jedi Guardian 5 to 6. KotOR 1 grants Force Jump Advanced (102) and Knight Sense
        // (108) at 6; KotOR 2 Force Jump Advanced (102) at 6 and Unarmed Specialist II (213)
        // at the sixth Jedi level. 102 is held already: not granted twice.
        let guardian = cat.class(3).cloned().unwrap();
        assert_eq!(guardian.grants.skill_point_base, if game == Game::K1 { 2 } else { 1 });
        for pc in [true, false] {
            let mut c = made_up(game, pc, if pc { "" } else { "kreia" }, 3, 5, 14);
            // Held already: not granted twice. (Only the player's records say what a level
            // gave; for anyone else lowering takes the level's feats away whoever gave them.)
            if pc {
                creature::add_feat(&mut c, 102);
            }
            let start = c.clone();
            creature::set_class_level(&mut c, 0, 6, &guardian, &xp_for);
            let gained: Vec<i64> = creature::feats(&c).into_iter().filter(|f| !creature::feats(&start).contains(f)).collect();
            // Int 14 (+2). KotOR 1: (2 + 2) / 2 = 2; KotOR 2: 1 + 2 = 3.
            let points = if game == Game::K1 { 2 } else { 3 };
            assert_eq!(creature::skill_points(&c), 1 + points, "{:?} pc {}: points banked", game, pc);
            let want = match (game, pc) {
                (Game::K1, true) => vec![108],
                (Game::K1, false) => vec![102, 108],
                (Game::K2, true) => vec![213],
                (Game::K2, false) => vec![102, 213],
            };
            assert_eq!(gained, want, "{:?} pc {}", game, pc);
            if pc {
                let rec = c.list("LvlStatList").unwrap().last().unwrap();
                let rf: Vec<i64> = rec.list("FeatList").unwrap().iter().filter_map(|f| f.int("Feat")).collect();
                assert_eq!(rf, gained, "the record holds the granted feats");
                assert_eq!(rec.int("SkillPoints"), Some(1 + points), "the record holds what is unspent");
            }
            creature::set_class_level(&mut c, 0, 5, &guardian, &xp_for);
            assert_eq!(creature::feats(&c), creature::feats(&start), "{:?} pc {}: feats back", game, pc);
            assert_eq!(creature::skill_points(&c), 1, "{:?} pc {}: points back", game, pc);
            assert_eq!(c.list("LvlStatList").map(|l| l.len()), start.list("LvlStatList").map(|l| l.len()));
            checked += 1;
        }
        if game == Game::K2 {
            // Atton's own column: Fighting Spirit (236) at total level 8, as the game gave it
            // in the saves on this machine; Scoundrel's base 4 with Int 10.
            let scoundrel = cat.class(2).cloned().unwrap();
            let mut a = made_up(game, false, "atton", 2, 7, 10);
            creature::set_class_level(&mut a, 0, 8, &scoundrel, &xp_for);
            assert!(creature::feats(&a).contains(&236), "Atton's Fighting Spirit at 8");
            assert_eq!(creature::skill_points(&a), 1 + 4);
            // Kreia: her column bars Light Armour Proficiency (5, "255") wherever a class
            // grants it; Bao-Dur's bars Heavy Armour (4).
            let soldier = cat.class(0).cloned().unwrap();
            if let Some((_, l)) = soldier.grants.granted.iter().find(|(f, _)| *f == 5) {
                let mut k = made_up(game, false, "kreia", 0, l - 1, 10);
                creature::set_class_level(&mut k, 0, *l, &soldier, &xp_for);
                assert!(!creature::feats(&k).contains(&5), "Kreia never gets 5");
                let mut s = made_up(game, false, "someone", 0, l - 1, 10);
                creature::set_class_level(&mut s, 0, *l, &soldier, &xp_for);
                assert!(creature::feats(&s).contains(&5), "anyone else does");
            }
            // The first character level: four times the points.
            let mut one = made_up(game, false, "someone", 0, 0, 12);
            creature::set_class_level(&mut one, 0, 1, &soldier, &xp_for);
            assert_eq!(creature::skill_points(&one), 1 + (soldier.grants.skill_point_base + 1).max(1) * 4);
        }
    }
    eprintln!("{} made-up creatures", checked);
}

/// The rules reproduce what the game wrote: every player level record in every save on this
/// machine holds the feats the level grants on its own (those the creature did not have
/// yet), with the creature's levels as they were after that level.
#[test]
fn granted_feats_match_the_games_level_records() {
    let installs = find_installs(&[]);
    let mut checked = 0;
    for game in [Game::K1, Game::K2] {
        let Some(install) = installs.iter().find(|i| i.game == game) else { continue };
        let data = GameData::open(game, &install.path);
        let cat = Catalog::build(&data);
        for folder in find_saves(&installs, game, &[]) {
            let name = folder.path.file_name().unwrap().to_string_lossy().to_string();
            if name.contains("KMRP") {
                continue; // written by the editor
            }
            let Ok(save) = Save::open(&folder.path) else { continue };
            let Some(p) = save.player() else { continue };
            let Some(records) = p.list("LvlStatList") else { continue };
            if !creature::level_records_match(p) {
                continue;
            }
            // Replay the records from the second one (the first is character creation).
            let ids: Vec<i64> = creature::classes(p).iter().map(|(k, _)| *k).collect();
            let mut levels = vec![0i64; ids.len()];
            let mut have: Vec<i64> = Vec::new();
            for (i, r) in records.iter().enumerate() {
                let k = r.int("LvlStatClass").unwrap_or(-1);
                let Some(idx) = ids.iter().position(|x| *x == k) else { break };
                levels[idx] += 1;
                let feats: Vec<i64> = r.list("FeatList").map(|l| l.iter().filter_map(|f| f.int("Feat")).collect()).unwrap_or_default();
                if i > 0 {
                    let mut c = p.clone();
                    let list: Vec<Struct> = ids
                        .iter()
                        .zip(&levels)
                        .filter(|(_, l)| **l > 0)
                        .map(|(id, l)| {
                            let mut e = Struct::new(2);
                            e.set("Class", Value::Int(*id as i32));
                            e.set("ClassLevel", Value::Short(*l as i16));
                            e
                        })
                        .collect();
                    let pos = list.iter().position(|e| e.int("Class") == Some(k)).unwrap();
                    c.set("ClassList", Value::List(list));
                    let fl: Vec<Struct> = have
                        .iter()
                        .map(|f| {
                            let mut e = Struct::new(1);
                            e.set("Feat", Value::Word(*f as u16));
                            e
                        })
                        .collect();
                    c.set("FeatList", Value::List(fl));
                    let class = cat.class(k).cloned().unwrap();
                    let granted = creature::level_feats(&c, pos, &class, true);
                    // The game puts them in the record (KotOR 1; KotOR 2 at a level with no
                    // feat to choose) or straight on the creature (KotOR 2 when the feat
                    // screen opens): either way the creature has them.
                    let all = creature::feats(p);
                    for g in &granted {
                        assert!(all.contains(g), "{:?} {} record {}: granted feat {} missing", game, name, i, g);
                        if game == Game::K1 {
                            assert!(feats.contains(g), "{:?} {} record {}: granted feat {} missing from {:?}", game, name, i, g, feats);
                        }
                    }
                    have.extend(granted);
                    checked += 1;
                }
                have.extend(feats);
            }
        }
    }
    eprintln!("{} level records checked", checked);
}
