//! The galaxy map's route (questdb::story_route): the order a save plays the story in, the free
//! planets the player can reorder, the Leviathan after the third of KotOR 1's, and the chapters
//! a save has played though the game took their quests out of the journal. Built from the quest
//! databases alone, plus every save of both games on the PC, read and never written. On a PC
//! without saves the last test passes without checking anything, and says so.

use kse_core::games::{find_installs, find_saves, Game};
use kse_core::globals::{Global, GlobalValue, Kind};
use kse_core::questdb::{self, ChapterState};
use kse_core::save::Save;

fn index(game: Game, planet: &str) -> usize {
    questdb::milestones_ref(game).iter().position(|(p, _)| p.eq_ignore_ascii_case(planet)).unwrap()
}

fn names(game: Game, order: &[usize]) -> Vec<&'static str> {
    let all = questdb::milestones_ref(game);
    order.iter().map(|i| all[*i].0).collect()
}

/// The journal a chapter's end leaves (its milestone's entries), in the order given.
fn journal_of(game: Game, planets: &[&str]) -> Vec<(String, i64)> {
    let all = questdb::milestones_ref(game);
    let mut out: Vec<(String, i64)> = Vec::new();
    for p in planets {
        let m = all[index(game, p)].1;
        for e in m.get("journal").and_then(|j| j.as_array()).into_iter().flatten() {
            let (Some(tag), stage) = (e.get("tag").and_then(|v| v.as_str()), e.get("stage").and_then(|v| v.as_i64()).unwrap_or(0)) else { continue };
            match out.iter_mut().find(|(t, _)| t.eq_ignore_ascii_case(tag)) {
                Some(x) => x.1 = x.1.max(stage),
                None => out.push((tag.to_string(), stage)),
            }
        }
    }
    out
}

fn flag(name: &str) -> Global {
    Global { kind: Kind::Boolean, index: 0, name: name.to_string(), value: GlobalValue::Bool(true) }
}

#[test]
fn a_new_game_follows_the_timeline_with_the_leviathan_after_the_third_planet() {
    let route = questdb::story_route(Game::K1, &[], &questdb::chapters_played(Game::K1, &[]), &[], &[], &[]);
    assert_eq!(
        names(Game::K1, &route.order),
        ["Endar Spire", "Taris", "Dantooine", "Tatooine", "Kashyyyk", "Manaan", "Leviathan", "Korriban", "Unknown World", "Star Forge"]
    );
    assert_eq!(route.groups.len(), 4);
    assert!(route.kinds.iter().all(|k| *k == 2), "nothing visited yet");
    let k2 = questdb::story_route(Game::K2, &[], &questdb::chapters_played(Game::K2, &[]), &[], &[], &[]);
    let order = names(Game::K2, &k2.order);
    assert_eq!(order.first(), Some(&"Peragus"));
    assert_eq!(order.last(), Some(&"Malachor V"));
    // Dxun leads into Onderon wherever the pair goes
    let dxun = order.iter().position(|p| *p == "Dxun").unwrap();
    assert_eq!(order[dxun + 1], "Onderon");
}

#[test]
fn the_order_picked_is_the_order_played() {
    let plan: Vec<String> = ["Korriban", "Manaan", "Tatooine", "Kashyyyk"].iter().map(|s| s.to_string()).collect();
    let route = questdb::story_route(Game::K1, &[], &questdb::chapters_played(Game::K1, &[]), &[], &[], &plan);
    assert_eq!(
        names(Game::K1, &route.order),
        ["Endar Spire", "Taris", "Dantooine", "Korriban", "Manaan", "Tatooine", "Leviathan", "Kashyyyk", "Unknown World", "Star Forge"]
    );
    // KotOR 2: the pair moves as one
    let plan: Vec<String> = ["Korriban", "Dxun", "Nar Shaddaa", "Dantooine"].iter().map(|s| s.to_string()).collect();
    let k2 = questdb::story_route(Game::K2, &[], &questdb::chapters_played(Game::K2, &[]), &[], &[], &plan);
    let order = names(Game::K2, &k2.order);
    let at = |p: &str| order.iter().position(|x| *x == p).unwrap();
    assert!(at("Korriban") < at("Dxun") && at("Dxun") + 1 == at("Onderon") && at("Onderon") < at("Nar Shaddaa") && at("Nar Shaddaa") < at("Dantooine"));
    assert!(at("Dantooine") < at("The Jedi Masters"));
}

#[test]
fn done_comes_first_and_the_rest_go_as_picked() {
    // Korriban done (its star map flag), Manaan begun (the save has been to its first area);
    // a plan that puts the unvisited Tatooine first moves it before Manaan, not before Korriban
    let mut quests = journal_of(Game::K1, &["Endar Spire", "Taris", "Dantooine"]);
    // a Tatooine quest in the journal is not a visit
    let tatooine_own = questdb::chapter_quests(Game::K1, "Tatooine").into_iter().map(|(t, _)| t).find(|t| t.starts_with("tat")).unwrap();
    quests.push((tatooine_own, 1));
    let globals = vec![flag("K_STAR_MAP_KORRIBAN")];
    let visited = vec!["manm26aa".to_string()];
    let plan: Vec<String> = ["Tatooine", "Manaan", "Korriban", "Kashyyyk"].iter().map(|s| s.to_string()).collect();
    // played as chapters_played_in reads a KotOR 1 save: the found star map counts
    let mut played = questdb::chapters_played(Game::K1, &quests);
    played[index(Game::K1, "Korriban")] = true;
    let route = questdb::story_route(Game::K1, &quests, &played, &globals, &visited, &plan);
    let order = names(Game::K1, &route.order);
    assert_eq!(&order[3..6], ["Korriban", "Tatooine", "Manaan"]);
    assert_eq!(route.kinds, [0, 2, 1, 2]);
    // without a plan, begun before not visited
    let route = questdb::story_route(Game::K1, &quests, &played, &globals, &visited, &[]);
    assert_eq!(&names(Game::K1, &route.order)[3..5], ["Korriban", "Manaan"]);
}

#[test]
fn played_chapters_come_in_the_order_their_quests_entered_the_journal() {
    let quests = journal_of(Game::K1, &["Endar Spire", "Taris", "Dantooine", "Manaan", "Tatooine"]);
    let globals = vec![flag("K_STAR_MAP_MANAAN"), flag("K_STAR_MAP_TATOOINE")];
    let route = questdb::story_route(Game::K1, &quests, &questdb::chapters_played(Game::K1, &quests), &globals, &[], &[]);
    assert_eq!(&names(Game::K1, &route.order)[3..5], ["Manaan", "Tatooine"]);
}

#[test]
fn a_later_chapter_means_the_line_before_it_was_played() {
    // only Tatooine's journal: the game took Dantooine's and Taris's quests out
    let quests = journal_of(Game::K1, &["Tatooine"]);
    let played = questdb::chapters_played(Game::K1, &quests);
    for p in ["Endar Spire", "Taris", "Dantooine", "Tatooine"] {
        assert!(played[index(Game::K1, p)], "{} should read played", p);
    }
    for p in ["Kashyyyk", "Manaan", "Korriban", "Leviathan", "Unknown World"] {
        assert!(!played[index(Game::K1, p)], "{} should not read played", p);
    }
    // the Unknown World means all four planets and the Leviathan
    let quests = journal_of(Game::K1, &["Unknown World"]);
    let played = questdb::chapters_played(Game::K1, &quests);
    for p in ["Tatooine", "Kashyyyk", "Manaan", "Korriban", "Leviathan"] {
        assert!(played[index(Game::K1, p)], "{} should read played", p);
    }
}

#[test]
fn skip_ahead_follows_the_route() {
    // after Dantooine, Korriban picked first: a skip to it plays Korriban alone
    let quests = journal_of(Game::K1, &["Endar Spire", "Taris", "Dantooine"]);
    let plan: Vec<String> = ["Korriban", "Tatooine", "Kashyyyk", "Manaan"].iter().map(|s| s.to_string()).collect();
    let route = questdb::story_route(Game::K1, &quests, &questdb::chapters_played(Game::K1, &quests), &[], &[], &plan);
    let played = questdb::chapters_played(Game::K1, &quests);
    let states: Vec<ChapterState> = route
        .order
        .iter()
        .enumerate()
        .map(|(k, i)| ChapterState { index: *i as i32, order: k as i64 + 1, side: false, played: played[*i], begun: false })
        .collect();
    let to = |p: &str| names(Game::K1, &questdb::skip_path(&states, index(Game::K1, p) as i32).iter().map(|i| *i as usize).collect::<Vec<_>>());
    assert_eq!(to("Korriban"), ["Korriban"]);
    assert_eq!(to("Leviathan"), ["Korriban", "Tatooine", "Kashyyyk", "Leviathan"]);
    assert_eq!(to("Manaan"), ["Korriban", "Tatooine", "Kashyyyk", "Leviathan", "Manaan"]);
}

#[test]
fn every_save_on_the_pc_has_a_whole_route() {
    let installs = find_installs(&[]);
    let mut seen = 0;
    for game in [Game::K1, Game::K2] {
        let mains = questdb::milestones_ref(game).iter().filter(|(_, m)| m.get("side_chapter").and_then(|v| v.as_bool()) != Some(true)).count();
        for folder in find_saves(&installs, game, &[]) {
            let Ok(save) = Save::open(&folder.path) else { continue };
            let globals = save.globals.as_ref().map(|g| kse_core::globals::list(&g.root)).unwrap_or_default();
            let mut visited = save.visited_modules();
            visited.push(save.module_name().to_ascii_lowercase());
            let route = questdb::story_route(game, &save.quests(), &questdb::chapters_played_in(&save), &globals, &visited, &[]);
            let mut sorted = route.order.clone();
            sorted.sort();
            sorted.dedup();
            assert_eq!(sorted.len(), mains, "{}: every main chapter once", folder.path.display());
            assert_eq!(route.order.len(), mains);
            if game == Game::K1 {
                let order = names(game, &route.order);
                let lev = order.iter().position(|p| *p == "Leviathan").unwrap();
                let free_before = order[..lev].iter().filter(|p| questdb::is_free(game, p)).count();
                assert_eq!(free_before, 3, "{}: the Leviathan after the third planet", folder.path.display());
            }
            // done, then begun, then not visited
            assert!(route.kinds.windows(2).all(|w| w[0] <= w[1]), "{}", folder.path.display());
            // the chapters before the one the save stands in, on the story's line, read played
            let played = questdb::chapters_played_in(&save);
            if let Some(here) = questdb::chapter_of_module(game, save.module_name()) {
                let all = questdb::milestones_ref(game);
                let k = route.order.iter().position(|i| *i == here);
                for i in route.order.iter().take(k.unwrap_or(0)) {
                    if !questdb::is_free(game, all[*i].0) && !all[*i].0.eq_ignore_ascii_case("Leviathan") {
                        assert!(played[*i], "{}: {} before {}", folder.path.display(), all[*i].0, all[here].0);
                    }
                }
            }
            seen += 1;
        }
    }
    if seen == 0 {
        eprintln!("no saves on this PC: nothing checked");
    } else {
        eprintln!("{} saves checked", seen);
    }
}
