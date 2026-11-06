//! The findings of the audit of 2026-11-02 that a test can reach, each by its number in
//! docs/audit-2026-11-02.md. Writes go to copies in the system's temporary folder, the rest to
//! saves in memory. Tests that need a save pass without checking anything on a PC without one,
//! and say so.

use kse_core::creature;
use kse_core::formats::erf::Erf;
use kse_core::formats::gff::Struct;
use kse_core::gamedata::GameData;
use kse_core::games::{find_installs, find_saves, Game};
use kse_core::questdb::{self, ChapterState};
use kse_core::save::{self, Save, Who};
use kse_core::story_preview::Plan;
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

fn files(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out: Vec<_> = fs::read_dir(dir).unwrap().filter_map(|e| e.ok()).filter(|e| e.path().is_file()).map(|e| (e.file_name().to_string_lossy().to_string(), fs::read(e.path()).unwrap())).collect();
    out.sort();
    out
}

/// A copy of a normal save of the PC, under a folder of its own.
fn copy(label: &str) -> Option<(PathBuf, PathBuf)> {
    let installs = find_installs(&[]);
    let folder = find_saves(&installs, Game::K1, &[]).into_iter().find(|f| !save::player_outside_archive(&f.path))?;
    let root = std::env::temp_dir().join("kmse-tests").join("audit").join(label);
    let _ = fs::remove_dir_all(&root);
    let work = root.join("000500 - Game499");
    copy_dir(&folder.path, &work);
    Some((root, work))
}

fn number(save: &Save, name: &str) -> i64 {
    let all = kse_core::globals::list(&save.globals.as_ref().unwrap().root);
    all.iter().find(|g| g.name.eq_ignore_ascii_case(name)).map_or(0, |g| match g.value {
        kse_core::globals::GlobalValue::Number(n) => n,
        _ => 0,
    })
}

fn flag(save: &Save, name: &str) -> bool {
    let all = kse_core::globals::list(&save.globals.as_ref().unwrap().root);
    all.iter().any(|g| g.name.eq_ignore_ascii_case(name) && g.value == kse_core::globals::GlobalValue::Bool(true))
}

/// A KotOR 1 save that has found no map, with its game.
fn fresh() -> Option<(Save, GameData)> {
    let installs = find_installs(&[]);
    let install = installs.iter().find(|i| i.game == Game::K1)?;
    let folder = find_saves(&installs, Game::K1, &[]).into_iter().find(|f| {
        !save::player_outside_archive(&f.path) && Save::open(&f.path).map_or(false, |s| number(&s, "K_STAR_MAP") == 0 && number(&s, "K_KOTOR_MASTER") <= 10 && !flag(&s, "K_STAR_MAP_TATOOINE"))
    })?;
    Some((Save::open(&folder.path).unwrap(), GameData::open(Game::K1, &install.path)))
}

fn chapter(planet: &str) -> serde_json::Value {
    questdb::milestones(Game::K1).into_iter().find(|(p, _)| p.eq_ignore_ascii_case(planet)).unwrap().1
}

fn upto(planet: &str) -> Vec<serde_json::Value> {
    let order = |m: &serde_json::Value| m.get("order").and_then(|v| v.as_i64()).unwrap_or(99);
    let end = order(&chapter(planet));
    let mut path: Vec<_> = questdb::milestones(Game::K1).into_iter().map(|(_, m)| m).filter(|m| m.get("side_chapter").is_none() && order(m) <= end).collect();
    path.sort_by_key(order);
    path
}

fn quest(tag: &str) -> questdb::Quest {
    questdb::quest(Game::K1, tag).unwrap().0
}

fn sum(steps: &[questdb::Step], key: &str) -> i64 {
    steps.iter().flat_map(|s| &s.does).filter_map(|e| e.get(key)).filter_map(|v| v.as_i64()).sum()
}

// ---- lost work or a damaged save ----

/// 4: the slot changed on disk after the save was read.
#[test]
fn a_save_changed_on_disk_is_not_written_over() {
    let Some((root, work)) = copy("stale") else { return eprintln!("no save here: nothing checked") };
    let mut sv = Save::open(&work).unwrap();
    sv.edit(Who::Player, "xp", |c| creature::set_experience(c, 1));
    let file = fs::read_dir(&work).unwrap().filter_map(|e| e.ok()).find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case("savenfo.res")).unwrap().path();
    let mut bytes = fs::read(&file).unwrap();
    bytes.push(0);
    fs::write(&file, &bytes).unwrap();
    let before = files(&work);
    assert_eq!(sv.write(&root.join("backups")).unwrap_err(), save::CHANGED_ON_DISK);
    assert!(sv.write_as(&root.join("000501 - Game500"), "x").is_err());
    assert_eq!(files(&work), before);
    assert!(!root.join("backups").exists(), "refused before the backup");
}

/// 4: a second Save Changes on the same save is no conflict with its own first.
#[test]
fn a_save_can_be_written_twice() {
    let Some((root, work)) = copy("twice") else { return };
    let mut sv = Save::open(&work).unwrap();
    for xp in [1, 2] {
        sv.edit(Who::Player, "xp", |c| creature::set_experience(c, xp));
        sv.write(&root.join("backups")).unwrap();
    }
}

/// 5: a file that cannot be replaced puts back the ones already replaced.
#[cfg(windows)]
#[test]
fn a_locked_file_leaves_the_save_as_it_was() {
    use std::os::windows::fs::OpenOptionsExt;
    let Some((root, work)) = copy("locked") else { return };
    let mut sv = Save::open(&work).unwrap();
    // the archive goes first and the party table after it, so the failure is part way
    sv.edit(Who::Player, "xp", |c| creature::set_experience(c, 777));
    let credits = sv.party_int("PT_GOLD").unwrap_or(0);
    sv.set_party_int("PT_GOLD", credits + 1);
    let before = files(&work);
    let party = fs::read_dir(&work).unwrap().filter_map(|e| e.ok()).find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case("partytable.res")).unwrap().path();
    let lock = fs::OpenOptions::new().read(true).share_mode(0).open(&party).unwrap();
    let result = sv.write(&root.join("backups"));
    drop(lock);
    let error = result.unwrap_err();
    assert!(!error.starts_with(save::HALF_WRITTEN), "{}", error);
    assert_eq!(files(&work), before, "byte for byte, and no stray files");
    assert!(!sv.changes.is_empty(), "the changes are still there to save again");
}

/// 10: a failed Save as stays on the save it was and leaves no folder.
#[cfg(windows)]
#[test]
fn a_failed_save_as_stays_where_it_was() {
    let Some((root, work)) = copy("save-as-fail") else { return };
    let mut sv = Save::open(&work).unwrap();
    sv.edit(Who::Player, "xp", |c| creature::set_experience(c, 777));
    // a name no folder can have
    let target = root.join("bad\0name");
    assert!(sv.write_as(&target, "x").is_err());
    assert_eq!(sv.folder, work);
    assert!(!sv.changes.is_empty());
    sv.write(&root.join("backups")).unwrap();
}

/// 6: a slot holding only a template entry stays as it is until it is made a whole item.
#[test]
fn a_template_entry_is_never_moved_to_the_inventory() {
    let Some((mut save, data)) = fresh() else { return };
    questdb::skip_through(&mut save, &data, &upto("Taris"), Some(0), false).unwrap();
    let Some((who, slot, res)) = save.npcs().into_iter().filter(|n| save.recruited(*n)).find_map(|n| {
        let c = save.creature(Who::Npc(n))?;
        let slot = creature::equipped(c).iter().map(|i| i.id).find(|s| save.slot_is_template(Who::Npc(n), *s).is_some())?;
        Some((Who::Npc(n), slot, save.slot_is_template(Who::Npc(n), slot)?))
    }) else {
        return eprintln!("no template entry here: nothing checked");
    };
    let items = save.inventory().len();
    assert!(!save.unequip(who, slot));
    assert_eq!(save.inventory().len(), items, "no blank entry");
    let uti = data.gff(&res, "uti", kse_core::formats::erf::RES_TYPE_UTI).unwrap();
    let like = save.inventory().first().cloned();
    save.fill_template_slot(who, slot, kse_core::catalog::item_from_template(&uti.root, like.as_ref(), 1));
    assert!(save.unequip(who, slot));
    assert_eq!(save.inventory().len(), items + 1);
    assert!(save.inventory().last().unwrap().get("BaseItem").is_some(), "the real item");
}

/// 7: nothing lands on the floor at NaN, and the save takes no NaN.
#[test]
fn a_position_that_is_not_a_number_is_refused() {
    assert_eq!(kse_core::map::Floor::default().height(f32::NAN, 1.0, 0.0), None);
    let Some((mut save, _)) = fresh() else { return };
    save.move_player([f32::NAN, 0.0, 0.0], None);
    assert!(save.changes.is_empty());
    assert!(save.move_to_module("nowhere", [0.0, f32::NAN, 0.0], "").is_err());
}

/// 9: Travel Here twice leaves one trip, the second.
#[test]
fn only_the_latest_trip_stays_in_the_queue() {
    let Some((mut save, _)) = fresh() else { return };
    save.queue_transition("danm13", "one", &[]).unwrap();
    save.queue_transition("tar_m02aa", "two", &[]).unwrap();
    let (ifo, _) = save.current_module_parts().unwrap();
    let trips: Vec<&Struct> = ifo.list("EventQueue").unwrap().iter().filter(|e| e.int("EventId") == Some(26)).collect();
    assert_eq!(trips.len(), 1);
}

/// 34: a damaged count is an error, not a request for more memory than exists.
#[test]
fn a_damaged_archive_count_is_an_error() {
    let mut data = vec![0u8; 200];
    data[0..4].copy_from_slice(b"MOD ");
    data[4..8].copy_from_slice(b"V1.0");
    data[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(Erf::read(&data).is_err());
}

// ---- the story ----

fn state(index: i32, order: i64, side: bool, played: bool) -> ChapterState {
    ChapterState { index, order, side, played }
}

/// 14: from Taris, a skip to a side chapter plays that chapter alone.
#[test]
fn a_side_chapter_stands_alone() {
    let chapters = [state(0, 1, false, true), state(1, 2, false, true), state(2, 3, false, false), state(3, 4, false, false), state(9, 11, true, false)];
    assert_eq!(questdb::skip_path(&chapters, 9), [9]);
    assert_eq!(questdb::skip_path(&chapters, 3), [2, 3]);
}

/// 15, 16: side chapters stay; the chapter the player is in is named as not finished.
#[test]
fn a_return_keeps_side_chapters_and_names_the_chapter_in_play() {
    let chapters = [state(0, 1, false, true), state(1, 2, false, true), state(2, 3, false, false), state(9, 11, true, true)];
    let r = questdb::return_to(&chapters, 1);
    assert_eq!(r.kept, [0, 9]);
    assert_eq!(r.undo, [1, 2]);
    assert_eq!(r.unfinished, Some(2));
    let r = questdb::return_to(&chapters, 9);
    assert_eq!(r.undo, [9]);
    assert_eq!(r.kept, [0, 1]);
    assert_eq!(r.unfinished, None);
}

/// 13: the measured stages give one way's reward, and no stage of either game gives two
/// bands of one alignment-scaled line.
#[test]
fn a_stage_is_reached_one_way() {
    let steps = questdb::one_way(&quest("kor35_renegadesith"), 40, false, 50);
    assert_eq!(sum(&steps, "xp"), 1000, "was 5,000");
    let light = sum(&questdb::one_way(&quest("kor35_waysith"), 50, false, 50), "dark_side");
    let dark = sum(&questdb::one_way(&quest("kor35_waysith"), 50, true, 50), "dark_side");
    assert!(light <= dark, "the side played picks the way: {} and {}", light, dark);
    for game in [Game::K1, Game::K2] {
        for (_, planet) in questdb::milestones(game).iter().map(|(p, m)| (m, p.clone())) {
            for (tag, _) in questdb::chapter_quests(game, &planet) {
                let Some((q, _)) = questdb::quest(game, &tag) else { continue };
                for stage in q.stages.iter().map(|s| s.stage) {
                    let steps = questdb::one_way(&q, stage, false, 50);
                    let mut seen: Vec<String> = Vec::new();
                    for s in steps.iter().filter(|s| s.branch.as_deref().map_or(false, |b| b.contains("if player alignment"))) {
                        let line = format!("{:?}{}", s.via, s.branch.as_deref().unwrap_or_default());
                        assert!(!seen.contains(&line), "{} {}: twice {}", tag, stage, line);
                        seen.push(line);
                    }
                }
            }
        }
    }
}

/// 17: the dark run's own endings read as played.
#[test]
fn a_dark_run_reads_as_played() {
    let all = questdb::milestones_ref(Game::K1);
    for (i, (planet, m)) in all.iter().enumerate() {
        let Some(dark) = m.get("dark").and_then(|d| d.get("journal")).and_then(|j| j.as_array()) else { continue };
        let quests: Vec<(String, i64)> = dark.iter().filter_map(|e| Some((e.get("tag")?.as_str()?.to_string(), e.get("stage")?.as_i64()?))).collect();
        if quests.is_empty() {
            continue;
        }
        assert!(questdb::chapters_played(Game::K1, &quests)[i], "{}", planet);
    }
}

/// 20: a planet whose map was not the last still reads played.
#[test]
fn k_starforge_does_not_decide_a_planet() {
    let all = questdb::milestones_ref(Game::K1);
    let (i, (_, m)) = all.iter().enumerate().find(|(_, (p, _))| p.eq_ignore_ascii_case("Tatooine")).unwrap();
    let quests: Vec<(String, i64)> = m["journal"].as_array().unwrap().iter().filter(|e| e["tag"] != "k_starforge").map(|e| (e["tag"].as_str().unwrap().to_string(), e["stage"].as_i64().unwrap())).chain([("k_starforge".to_string(), 30)]).collect();
    assert!(questdb::chapters_played(Game::K1, &quests)[i]);
}

/// 18: what the game took out of the journal stays out.
#[test]
fn quests_the_game_removes_stay_removed() {
    let Some((mut save, data)) = fresh() else { return };
    questdb::skip_through(&mut save, &data, &upto("Taris"), Some(0), false).unwrap();
    let removed = chapter("Taris")["journal_remove"].as_array().unwrap().clone();
    let quests = save.quests();
    let back: Vec<_> = removed.iter().filter_map(|t| t.as_str()).filter(|t| quests.iter().any(|(q, _)| q.eq_ignore_ascii_case(t))).collect();
    assert!(back.is_empty(), "back in the journal: {:?}", back);
}

/// 22: each planet's XP counts once, whatever the order of the skip.
#[test]
fn xp_follows_the_chapters_played() {
    let Some((save, data)) = fresh() else { return };
    let xp = |chapters: &[serde_json::Value]| {
        let mut s = save.clone();
        questdb::skip_through(&mut s, &data, chapters, Some(0), false).unwrap();
        s.player().map(creature::experience).unwrap_or(0)
    };
    let korriban = xp(&upto("Korriban"));
    assert_eq!(korriban, 161_890, "with the Leviathan, which comes before the fourth planet");
    let leviathan = xp(&upto("Leviathan"));
    assert_eq!(leviathan, 161_890 - (156_320 - 134_420), "without the fourth planet");
}

/// 11: a return past maps takes them off; the rest stay counted.
#[test]
fn a_return_takes_the_maps_of_what_it_undoes() {
    let Some((mut s, data)) = fresh() else { return };
    questdb::skip_through(&mut s, &data, &upto("Korriban"), Some(0), false).unwrap();
    assert_eq!(number(&s, "K_STAR_MAP"), 50);
    let (kept, undo): (Vec<_>, Vec<_>) = upto("Korriban").into_iter().partition(|m| m["order"].as_i64() < Some(5));
    questdb::go_back(&mut s, &data, &kept, &undo, false, None).unwrap();
    assert_eq!(number(&s, "K_STAR_MAP"), 20, "Tatooine's alone");
    assert!(flag(&s, "K_STAR_MAP_TATOOINE") && !flag(&s, "K_STAR_MAP_KORRIBAN") && !flag(&s, "K_STAR_MAP_MANAAN") && !flag(&s, "K_STAR_MAP_KASHYYYK"));
    assert_eq!(number(&s, "K_CAPTURED_LEV"), 0);
    assert_eq!(number(&s, "K_KOTOR_MASTER"), 15, "the Unknown World closed again");
    assert!(s.player().map_or(0, creature::experience) <= 72_616, "XP is not raised: 21");
}

/// 12: a return to the Leviathan with all four maps takes the fourth back, so the capture can
/// come, with its count of 40, as the game needs it.
#[test]
fn a_return_to_the_leviathan_leaves_the_capture_to_play() {
    let Some((mut s, data)) = fresh() else { return };
    questdb::skip_through(&mut s, &data, &upto("Korriban"), Some(0), false).unwrap();
    let notes = questdb::go_back(&mut s, &data, &upto("Korriban"), &[chapter("Leviathan")], false, None).unwrap();
    assert_eq!(number(&s, "K_STAR_MAP"), 40);
    assert_eq!(number(&s, "K_CAPTURED_LEV"), 5, "waiting for the next take-off");
    assert_eq!(number(&s, "K_KOTOR_MASTER"), 15);
    assert!(notes.iter().any(|n| n.contains("is undone too")), "{:?}", notes);
}

/// 27: the same notes reach the program as the skip made.
#[test]
fn a_plan_keeps_its_notes() {
    let Some((save, data)) = fresh() else { return };
    let plan = Plan::Skip { chapters: upto("Dantooine"), stage: None, class: Some(0), dark: false, prestige: None, train: false };
    assert!(!plan.prepare(&save, &data).unwrap().notes.is_empty());
}

// ---- character, inventory and world ----

/// 36: an imported companion's items are renumbered for this save.
#[test]
fn an_imported_companion_takes_this_saves_ids() {
    let Some((mut save, data)) = fresh() else { return };
    let mut other = save.clone();
    questdb::skip_through(&mut other, &data, &upto("Taris"), Some(0), false).unwrap();
    let Some(n) = other.npcs().into_iter().find(|n| other.recruited(*n)) else { return };
    let items = other.creature(Who::Npc(n)).map_or(0, |c| creature::equipped(c).len());
    save.import_npc(&other, n).unwrap();
    assert_eq!(save.creature(Who::Npc(n)).map_or(0, |c| creature::equipped(c).len()), items);
    let ids: Vec<i64> = save.creature(Who::Npc(n)).map(|c| creature::equipped(c).iter().filter_map(|i| i.int("ObjectId")).collect()).unwrap_or_default();
    let mut unique = ids.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(ids.len(), unique.len());
}

/// 38: Stealth Field Generators, base item 88, are on the list.
#[test]
fn a_belt_with_a_creature_slot_is_an_item() {
    let Some((_, data)) = fresh() else { return };
    assert!(kse_core::catalog::item_category(&data.table("baseitems"), 88).is_some());
    assert!(kse_core::catalog::item_category(&data.table("baseitems"), 81).is_none(), "a claw is not");
}

/// 37 and 42 sit in the program and in `creature`; a raised level keeps XP past it.
#[test]
fn raising_a_level_keeps_xp_past_it() {
    let Some((mut save, data)) = fresh() else { return };
    let cat = kse_core::catalog::Catalog::build(&data);
    let Some(class) = save.player().and_then(|p| creature::classes(p).first().map(|c| c.0)).and_then(|id| cat.class(id).cloned()) else { return };
    let xp_for = |l: i64| cat.xp_for(l);
    save.edit(Who::Player, "xp", |c| creature::set_experience(c, 500_000));
    let level = save.player().map(creature::level).unwrap();
    save.edit(Who::Player, "level", |c| creature::set_class_level(c, 0, level + 1, &class, &xp_for));
    assert_eq!(save.player().map(creature::experience), Some(500_000));
}

/// 26: the Jedi path stays the one picked.
#[test]
fn the_jedi_path_stays_the_one_picked() {
    let Some((save, data)) = fresh() else { return };
    let m = chapter("Dantooine");
    let variable = m["jedi_class"]["variable"].as_str().unwrap();
    for pick in 0..3 {
        let mut s = save.clone();
        questdb::skip_through(&mut s, &data, &upto("Dantooine"), Some(pick), false).unwrap();
        assert_eq!(number(&s, variable), m["jedi_class"]["choices"][pick]["path_state"].as_i64().unwrap());
    }
}

#[test]
fn backups_stay_out_of_what_steam_syncs() {
    let game = std::path::Path::new("D:/Games/swkotor");
    let backups = game.join("Save Editor Backup");
    assert_eq!(kse_core::save::backup_root(&game.join("Saves/000001 - Game0"), Some(game)), backups);
    assert_eq!(kse_core::save::backup_root(&game.join("cloudsaves/123/000001 - Game0"), Some(game)), backups);
    // A save outside the game keeps its backups beside it.
    let other = std::path::Path::new("D:/Elsewhere/saves");
    assert_eq!(kse_core::save::backup_root(&other.join("000001 - Game0"), Some(game)), other.join("Save Editor Backup"));
    assert_eq!(kse_core::save::backup_root(&other.join("000001 - Game0"), None), other.join("Save Editor Backup"));
}

/// A copy of KotOR 2's save on Peragus, with its game, and the ten chapters of the main path.
fn k2_start(label: &str) -> Option<(Save, GameData, Vec<serde_json::Value>)> {
    let installs = find_installs(&[]);
    let install = installs.iter().find(|i| i.game == Game::K2)?;
    let folder = find_saves(&installs, Game::K2, &[]).into_iter().filter(|f| !save::player_outside_archive(&f.path)).find(|f| {
        Save::open(&f.path).map_or(false, |s| s.module_name().to_ascii_lowercase().starts_with("101per"))
    })?;
    let work = std::env::temp_dir().join("kmse-tests").join("audit").join(label);
    let _ = fs::remove_dir_all(&work);
    copy_dir(&folder.path, &work);
    let side = |m: &serde_json::Value| m.get("side_chapter").and_then(|v| v.as_bool()).unwrap_or(false);
    let mut path: Vec<serde_json::Value> = questdb::milestones(Game::K2).into_iter().map(|(_, m)| m).filter(|m| !side(m)).collect();
    path.sort_by_key(|m| m.get("order").and_then(|v| v.as_i64()).unwrap_or(99));
    Some((Save::open(&work).unwrap(), GameData::open(Game::K2, &install.path), path))
}

/// 16: the chapter the player is in has not found its master yet, so a return takes back one
/// master fewer than a return over the finished chapter does.
#[test]
fn a_k2_chapter_in_progress_keeps_its_count() {
    let Some((mut save, data, path)) = k2_start("k2-unfinished") else { return eprintln!("no KotOR 2 save here: nothing checked") };
    let at = |p: &str| path.iter().position(|m| m["planet"] == p).unwrap();
    let (nar, onderon) = (at("Nar Shaddaa"), at("Onderon"));
    let undo: Vec<serde_json::Value> = path[nar..=onderon].to_vec();
    let kept: Vec<serde_json::Value> = path[..nar].to_vec();
    questdb::skip_through(&mut save, &data, &path[..=onderon], None, false).unwrap();
    assert_eq!(number(&save, "000_Jedi_Found"), 2);
    questdb::go_back_part_played(&mut save, &data, &kept, &undo, true, None, 50, Some("Onderon")).unwrap();
    assert_eq!(number(&save, "000_Jedi_Found"), 1);
}

/// 24: KotOR 2's prestige class and its story state leave together, on the real tables.
#[test]
fn a_k2_return_takes_the_prestige_class_back() {
    let Some((mut save, data, path)) = k2_start("k2-prestige") else { return eprintln!("no KotOR 2 save here: nothing checked") };
    let pick = match creature::classes(save.player().unwrap())[0].0 { 3 => 0, 5 => 1, _ => 2 };
    let prestige = questdb::PRESTIGE_LIGHT[pick];
    questdb::skip_through_with(&mut save, &data, &path, None, false, Some(prestige), true).unwrap();
    assert_eq!(number(&save, "000_Prestige"), 2);
    questdb::go_back(&mut save, &data, &path[..2], &path[2..], true, None).unwrap();
    assert!(creature::classes(save.player().unwrap()).iter().all(|c| c.0 != prestige));
    assert_eq!(number(&save, "000_Prestige"), 0);
    assert_eq!(number(&save, "000_Jedi_Found"), 0);
}

/// 25: on a run reached by skipping, the only KotOR 2 run there is here, every chapter of the
/// main path reads played, and the early save reads none of the later ones.
#[test]
fn a_k2_run_reads_as_played() {
    let Some((mut save, data, path)) = k2_start("k2-played") else { return eprintln!("no KotOR 2 save here: nothing checked") };
    let all = questdb::milestones_ref(Game::K2);
    let index = |p: &serde_json::Value| all.iter().position(|(n, _)| p == n).unwrap();
    let early = questdb::chapters_played(Game::K2, &save.quests());
    assert!(!early[index(&path[9]["planet"])]);
    questdb::skip_through(&mut save, &data, &path, None, false).unwrap();
    let played = questdb::chapters_played(Game::K2, &save.quests());
    for m in &path {
        assert!(played[index(&m["planet"])], "{}", m["planet"]);
    }
}
