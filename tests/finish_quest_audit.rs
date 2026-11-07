//! Finish Quest, checked against what the game does rather than against itself (2026-11-07
//! audit). The quest data lists each path through a conversation or script as its own step;
//! the game runs one path. Saves here are worked on in memory only.
//!
//! - every quest of both games: of each group of alternatives in a line, at most one path is
//!   taken; the XP of stages with steps at several points of a line is as worked out by hand (data only, so it also runs where no game is installed);
//! - KotOR II's DroidRetrieval stage 20: Kodin is paid once, not 63 times (-46,400 credits);
//! - KotOR's Tatooine star map: finishing it finds that one map, not all four;
//! - KotOR II's escape_merchant: finishing it recruits Mandalore, as its script does;
//! - a quest at an ending already, or at the stage asked, is not finished again.

use kse_core::gamedata::GameData;
use kse_core::games::{find_installs, find_saves, Game};
use kse_core::globals::{self, GlobalValue};
use kse_core::questdb;
use kse_core::save::Save;

fn condition(branch: &Option<String>) -> String {
    let b = branch.as_deref().unwrap_or("").trim();
    let c = if b.starts_with("way ") { b.split_once(" / ").map(|(_, c)| c).unwrap_or("") } else { b };
    if c.contains("player alignment") {
        String::new()
    } else {
        c.trim().to_string()
    }
}

/// The group of alternatives within a way, as the editor reads it: what comes before the
/// condition's last clause, and the kind of that clause.
fn group(cond: &str) -> String {
    let (before, last) = cond.rsplit_once(" / ").unwrap_or(("", cond));
    let last = last.trim();
    let kind = if last.starts_with("before this line:") {
        "script"
    } else if last.starts_with("line ") {
        "line"
    } else if last.starts_with("choice:") {
        "choice"
    } else {
        "test"
    };
    format!("{}|{}", before.trim(), kind)
}

/// XP the steps of one stage give, on the way one play takes.
fn stage_xp(game: Game, tag: &str, stage: i64) -> i64 {
    let (q, _) = questdb::quest(game, tag).unwrap_or_else(|| panic!("{} is in the data", tag));
    questdb::one_way(&q, stage, false, 50)
        .iter()
        .filter(|t| q.stages.iter().any(|s| s.stage == stage && s.steps.iter().any(|x| x.via == t.via && x.branch == t.branch && x.does == t.does)))
        .flat_map(|t| &t.does)
        .filter_map(|e| e.get("xp").and_then(|v| v.as_i64()))
        .sum()
}

/// The XP of stages whose steps sit at several points of one line, worked out by hand from the
/// data (2026-11-07 second review): the node's own script and the line's own test are both
/// run, and of two lines after one reply only one.
#[test]
fn steps_at_different_points_of_a_line_are_all_run() {
    // "otherwise: 110 XP" or "if has the part: 110 XP and the part taken": 110, not 220 or 0
    assert_eq!(stage_xp(Game::K2, "tutorial_3CFD", 90), 110);
    // "before this line: if 101PER_Know_Sedatives == 0: 50 XP" (its != 0 sibling gives none)
    assert_eq!(stage_xp(Game::K2, "MedBayMur", 10), 50);
    // the entry's 160, and of line E66 (110 XP) or E67 (nothing) one: E66
    assert_eq!(stage_xp(Game::K1, "dan_murder", 70), 270);
    // the entry's 500, and line E74's 110
    assert_eq!(stage_xp(Game::K1, "kor35_doublecross2", 10), 610);
}

#[test]
fn one_path_per_way_in_every_quest_of_both_games() {
    let mut ways = 0;
    for game in [Game::K1, Game::K2] {
        let mut tags: Vec<String> = questdb::milestones_ref(game).iter().flat_map(|(p, _)| questdb::chapter_quests(game, p)).map(|(t, _)| t).collect();
        tags.sort();
        tags.dedup();
        for tag in tags {
            let Some((q, _)) = questdb::quest(game, &tag) else { continue };
            for s in &q.stages {
                for dark in [false, true] {
                    let steps = questdb::one_way(&q, s.stage, dark, 50);
                    // the steps of this stage among them with a condition, by their group of
                    // alternatives: the way, what comes before the last clause, and its kind
                    let mut by_group: std::collections::BTreeMap<String, Vec<String>> = Default::default();
                    for t in steps.iter().filter(|t| t.via.is_some() && s.steps.iter().any(|x| x.via == t.via && x.branch == t.branch && x.does == t.does)) {
                        let c = condition(&t.branch);
                        if !c.is_empty() {
                            let key = format!("{}|{}", t.via.as_ref().map(|v| v.to_string()).unwrap_or_default(), group(&c));
                            by_group.entry(key).or_default().push(c);
                        }
                    }
                    for (key, paths) in by_group {
                        ways += 1;
                        assert!(paths.len() <= 1, "{:?} {} stage {} ({}): {} alternatives taken at once: {:?}", game, tag, s.stage, key.chars().take(60).collect::<String>(), paths.len(), paths.iter().take(3).collect::<Vec<_>>());
                    }
                }
            }
        }
    }
    assert!(ways > 100, "the data has conditional ways to check ({})", ways);
}

#[test]
fn kodin_is_paid_once() {
    let (q, _) = questdb::quest(Game::K2, "DroidRetrieval").expect("KotOR II's DroidRetrieval is in the data");
    let credits: i64 = questdb::one_way(&q, 20, false, 50)
        .iter()
        .filter(|t| q.stages.iter().any(|s| s.stage == 20 && s.steps.iter().any(|x| x.via == t.via && x.branch == t.branch)))
        .flat_map(|t| &t.does)
        .filter_map(|e| e.get("credits").and_then(|v| v.as_i64()))
        .sum();
    // one path pays Kodin at most a few hundred credits, each payment listed in it once
    assert!(credits > -1500, "stage 20 takes {} credits", credits);
}

fn flags(save: &Save, names: &[&str]) -> usize {
    let list = save.globals.as_ref().map(|g| globals::list(&g.root)).unwrap_or_default();
    list.iter().filter(|g| names.iter().any(|n| g.name.eq_ignore_ascii_case(n)) && g.value == GlobalValue::Bool(true)).count()
}

fn number(save: &Save, name: &str) -> Option<i64> {
    let list = save.globals.as_ref().map(|g| globals::list(&g.root)).unwrap_or_default();
    list.iter().find(|g| g.name.eq_ignore_ascii_case(name)).and_then(|g| match g.value {
        GlobalValue::Number(n) => Some(n),
        _ => None,
    })
}

fn setup(game: Game) -> Option<(GameData, Vec<Save>)> {
    let installs = find_installs(&[]);
    let install = installs.iter().find(|i| i.game == game)?;
    let data = GameData::open(game, &install.path);
    let saves: Vec<Save> = find_saves(&installs, game, &[]).into_iter().filter_map(|f| Save::open(&f.path).ok()).collect();
    (!saves.is_empty()).then_some((data, saves))
}

#[test]
fn finishing_a_star_map_quest_finds_one_map() {
    let Some((data, saves)) = setup(Game::K1) else { return eprintln!("KotOR is not installed or has no saves here: nothing checked") };
    let maps = ["K_STAR_MAP_TATOOINE", "K_STAR_MAP_KASHYYYK", "K_STAR_MAP_MANAAN", "K_STAR_MAP_KORRIBAN"];
    let journal = kse_core::quests::all(&data);
    let tag = "tat17_starmap";
    let Some(q) = journal.iter().find(|q| q.tag.eq_ignore_ascii_case(tag)) else { return eprintln!("{} not in this game's journal", tag) };
    let mut checked = 0;
    for original in &saves {
        // a save where Tatooine's map is not found yet
        if flags(original, &["K_STAR_MAP_TATOOINE"]) == 1 {
            continue;
        }
        for ending in q.endings() {
            let mut save = original.clone();
            save.remove_quest(tag);
            let before = flags(&save, &maps);
            let master = number(&save, "K_KOTOR_MASTER");
            questdb::finish_quest(&mut save, &data, tag, ending.id, false).unwrap();
            let after = flags(&save, &maps);
            assert!(after <= before + 1, "ending {}: {} maps found, {} before", ending.id, after, before);
            if after < 4 {
                assert_eq!(number(&save, "K_KOTOR_MASTER"), master, "ending {}: the Unknown World opened with {} maps", ending.id, after);
            }
            checked += 1;
        }
    }
    eprintln!("{} star map finishes checked", checked);
    assert!(checked > 0 || saves.iter().all(|o| flags(o, &["K_STAR_MAP_TATOOINE"]) == 1), "a save without Tatooine's map was there to check");
}

#[test]
fn finishing_escape_merchant_recruits_mandalore() {
    let Some((data, saves)) = setup(Game::K2) else { return eprintln!("KotOR II is not installed or has no saves here: nothing checked") };
    let journal = kse_core::quests::all(&data);
    let tag = "escape_merchant";
    let Some(q) = journal.iter().find(|q| q.tag.eq_ignore_ascii_case(tag)) else { return eprintln!("{} not in this game's journal", tag) };
    let Some(ending) = q.endings().into_iter().map(|e| e.id).max() else { return };
    let mut save = saves[0].clone();
    save.remove_quest(tag);
    if save.recruited(2) {
        save.set_recruited(2, false);
    }
    let notes = questdb::finish_quest(&mut save, &data, tag, ending, false).unwrap();
    // its stage 90 adds slot 2 by template (AddAvailableNPCByTemplate(2, "p_mand"))
    let script_recruits = questdb::quest(Game::K2, tag).map_or(false, |(qq, _)| qq.stages.iter().any(|s| s.steps.iter().any(|t| t.does.iter().any(|e| e.get("other").and_then(|v| v.as_str()).map_or(false, |o| o.starts_with("AddAvailableNPCByTemplate(2"))))));
    if script_recruits {
        assert!(save.recruited(2), "Mandalore is recruited: {:?}", notes);
        assert!(!notes.iter().any(|n| n.contains("AddAvailableNPC")), "not also left to the game: {:?}", notes);
    }
}

#[test]
fn a_finished_quest_is_not_finished_again() {
    for game in [Game::K1, Game::K2] {
        let Some((data, saves)) = setup(game) else { continue };
        let journal = kse_core::quests::all(&data);
        let mut checked = 0;
        for original in saves.iter().take(4) {
            for (tag, stage) in original.quests() {
                let Some(q) = journal.iter().find(|q| q.tag.eq_ignore_ascii_case(&tag)) else { continue };
                let mut save = original.clone();
                // at the stage asked: refused, nothing changed
                assert!(questdb::finish_quest(&mut save, &data, &tag, stage, false).is_err(), "{:?} {} at {}", game, tag, stage);
                // ended: refused for every ending
                if q.stage(stage).map_or(false, |s| s.end) {
                    for e in q.endings() {
                        assert!(questdb::finish_quest(&mut save, &data, &tag, e.id, false).is_err(), "{:?} {} ended at {}, again at {}", game, tag, stage, e.id);
                    }
                }
                assert_eq!(save.quests(), original.quests(), "{:?} {}: a refusal changes nothing", game, tag);
                assert!(save.changes.is_empty(), "{:?} {}: a refusal changes nothing", game, tag);
                checked += 1;
            }
        }
        eprintln!("{:?}: {} quests checked", game, checked);
        assert!(checked > 0, "{:?}: saves with quests were there to check", game);
    }
}
