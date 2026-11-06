//! KotOR 1's star maps through Skip Ahead: the count, the planets found and the Leviathan's
//! place, for any number of planets and any order. In memory only, on a save read from the
//! PC that has found no map yet: nothing is written to disk. On a PC without such a save the
//! tests pass without checking anything, and say so.

use kse_core::gamedata::GameData;
use kse_core::games::{find_installs, find_saves, Game};
use kse_core::globals::{self, GlobalValue};
use kse_core::questdb;
use kse_core::save::Save;

const FLAGS: [&str; 4] = ["K_STAR_MAP_TATOOINE", "K_STAR_MAP_KASHYYYK", "K_STAR_MAP_MANAAN", "K_STAR_MAP_KORRIBAN"];

fn number(save: &Save, name: &str) -> i64 {
    let all = globals::list(&save.globals.as_ref().unwrap().root);
    all.iter().find(|g| g.name.eq_ignore_ascii_case(name)).map_or(0, |g| match g.value {
        GlobalValue::Number(n) => n,
        _ => 0,
    })
}

fn found(save: &Save) -> Vec<&'static str> {
    let all = globals::list(&save.globals.as_ref().unwrap().root);
    FLAGS.iter().copied().filter(|f| all.iter().any(|g| g.name.eq_ignore_ascii_case(f) && g.value == GlobalValue::Bool(true))).collect()
}

fn set_flag(save: &mut Save, data: &GameData, flag: &str) {
    questdb::apply(save, data, &step(serde_json::json!({ "set_global_bool": [flag, true] })), "test.flag");
}

fn set_number(save: &mut Save, data: &GameData, name: &str, value: i64) {
    questdb::apply(save, data, &step(serde_json::json!({ "set_global_number": [name, value] })), "test.number");
}

fn step(effect: serde_json::Value) -> questdb::Step {
    questdb::Step { module: String::new(), via: None, needs: Vec::new(), does: vec![effect], branch: None, found: Default::default() }
}

/// A save that has found no map, with the game it belongs to.
fn start() -> Option<(Save, GameData)> {
    let installs = find_installs(&[]);
    let install = installs.iter().find(|i| i.game == Game::K1)?;
    let folder = find_saves(&installs, Game::K1, &[]).into_iter().find(|f| {
        !kse_core::save::player_outside_archive(&f.path)
            && Save::open(&f.path).map_or(false, |s| found(&s).is_empty() && number(&s, "K_STAR_MAP") == 0 && number(&s, "K_KOTOR_MASTER") <= 10)
    })?;
    Some((Save::open(&folder.path).unwrap(), GameData::open(Game::K1, &install.path)))
}

/// The main chapters up to `planet`, in the timeline's order, as Skip to Here passes them.
fn upto(planet: &str) -> Vec<serde_json::Value> {
    let order = |m: &serde_json::Value| m.get("order").and_then(|v| v.as_i64()).unwrap_or(99);
    let all = questdb::milestones(Game::K1);
    let end = all.iter().find(|(p, _)| p.eq_ignore_ascii_case(planet)).map(|(_, m)| order(m)).unwrap();
    let mut path: Vec<_> = all.into_iter().map(|(_, m)| m).filter(|m| order(m) <= end).collect();
    path.sort_by_key(order);
    path
}

fn chapter(planet: &str) -> serde_json::Value {
    questdb::milestones(Game::K1).into_iter().find(|(p, _)| p.eq_ignore_ascii_case(planet)).unwrap().1
}

#[test]
fn each_planet_counts_one_map() {
    let Some((save, data)) = start() else {
        eprintln!("no save without a star map here: nothing checked");
        return;
    };
    // skip to, count, maps found, K_CAPTURED_LEV, K_KOTOR_MASTER
    let table: [(&str, i64, usize, i64, i64); 7] = [
        ("Dantooine", 10, 0, 0, 15),
        ("Tatooine", 20, 1, 0, 15),
        ("Kashyyyk", 30, 2, 0, 15),
        ("Manaan", 40, 3, 5, 15),
        // the Leviathan comes before the fourth planet, so the capture is behind it
        ("Korriban", 50, 4, 10, 30),
        // and a skip to the Leviathan leaves the fourth planet to play
        ("Leviathan", 40, 3, 10, 20),
        ("Unknown World", 50, 4, 10, 50),
    ];
    for (planet, count, maps, captured, master) in table {
        let mut s = save.clone();
        questdb::skip_through(&mut s, &data, &upto(planet), Some(0), false).unwrap();
        assert_eq!(number(&s, "K_STAR_MAP"), count, "{}: K_STAR_MAP", planet);
        assert_eq!(found(&s).len(), maps, "{}: maps found {:?}", planet, found(&s));
        assert_eq!(number(&s, "K_CAPTURED_LEV"), captured, "{}: K_CAPTURED_LEV", planet);
        assert_eq!(number(&s, "K_KOTOR_MASTER"), master, "{}: K_KOTOR_MASTER", planet);
    }
}

#[test]
fn the_fourth_planet_comes_after_the_capture_and_bastila_is_taken() {
    let Some((save, data)) = start() else { return };
    let mut s = save.clone();
    questdb::skip_through(&mut s, &data, &upto("Korriban"), Some(0), false).unwrap();
    assert!(s.quests().iter().any(|(t, v)| t.eq_ignore_ascii_case("lev_captured") && *v >= 99), "the Leviathan is played");
    assert!(!s.recruited(0), "the Leviathan takes slot 0");
}

#[test]
fn planets_played_in_another_order_are_kept() {
    let Some((save, data)) = start() else { return };
    let mut s = save.clone();
    questdb::skip_through(&mut s, &data, &upto("Dantooine"), Some(0), false).unwrap();
    // Korriban and Manaan played first, by hand; then a skip to Tatooine
    set_flag(&mut s, &data, "K_STAR_MAP_KORRIBAN");
    set_flag(&mut s, &data, "K_STAR_MAP_MANAAN");
    set_number(&mut s, &data, "K_STAR_MAP", 30);
    questdb::skip_through(&mut s, &data, &[chapter("Tatooine")], None, false).unwrap();
    assert_eq!(number(&s, "K_STAR_MAP"), 40, "the third map, whichever it is");
    assert_eq!(found(&s), ["K_STAR_MAP_TATOOINE", "K_STAR_MAP_MANAAN", "K_STAR_MAP_KORRIBAN"]);
    assert_eq!(number(&s, "K_CAPTURED_LEV"), 5, "the capture waits for the next take-off");
    assert_eq!(number(&s, "K_KOTOR_MASTER"), 15, "the Unknown World stays closed");
}

#[test]
fn a_skip_to_the_stage_of_a_map_finds_that_map_alone() {
    let Some((save, data)) = start() else { return };
    for (planet, tag, stage, flag) in [("Tatooine", "tat17_starmap", 90, "K_STAR_MAP_TATOOINE"), ("Korriban", "kor33_findstarmap", 40, "K_STAR_MAP_KORRIBAN")] {
        let mut s = save.clone();
        questdb::skip_through(&mut s, &data, &upto("Dantooine"), Some(0), false).unwrap();
        questdb::skip_to_stage(&mut s, &data, &[chapter(planet)], tag, stage, None, false, None, false).unwrap();
        assert_eq!(found(&s), [flag], "{}", planet);
        assert_eq!(number(&s, "K_STAR_MAP"), 20, "{}", planet);
        assert_eq!(number(&s, "K_CAPTURED_LEV"), 0, "{}", planet);
        assert_eq!(number(&s, "K_KOTOR_MASTER"), 15, "{}", planet);
    }
}

/// A stage of the fourth planet, skipped to after the Leviathan and before that planet's own
/// map: the count is 40 again, and that must not set the capture up a second time.
#[test]
fn a_stage_after_the_leviathan_does_not_play_the_capture_again() {
    let Some((save, data)) = start() else { return };
    let mut s = save.clone();
    questdb::skip_through(&mut s, &data, &upto("Leviathan"), Some(0), false).unwrap();
    assert_eq!(number(&s, "K_CAPTURED_LEV"), 10);
    questdb::skip_to_stage(&mut s, &data, &[chapter("Korriban")], "kor33_enteracademy", 50, None, false, None, false).unwrap();
    assert_eq!(number(&s, "K_STAR_MAP"), 40);
    assert_eq!(number(&s, "K_CAPTURED_LEV"), 10);
}

#[test]
fn a_chapter_replayed_does_not_change_the_count() {
    let Some((save, data)) = start() else { return };
    let mut s = save.clone();
    questdb::skip_through(&mut s, &data, &upto("Tatooine"), Some(0), false).unwrap();
    questdb::skip_through(&mut s, &data, &[chapter("Tatooine")], None, false).unwrap();
    assert_eq!(number(&s, "K_STAR_MAP"), 20);
    assert_eq!(found(&s).len(), 1);
}
