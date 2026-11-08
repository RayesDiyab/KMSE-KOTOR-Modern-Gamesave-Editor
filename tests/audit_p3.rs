//! Regressions of the 2026-11-07 audit's smaller (P3) findings that a test can reach, on copies
//! of the saves on this PC in memory; each fails against the code before its fix. Tests that
//! need a save or a game pass without checking anything on a PC without one, and say so.
//!
//! - E-6: a party member who stays keeps their own party entry (the leader flag with it);
//! - E-7: a member the story keeps out of the party (Can be picked off) is not put in it;
//! - E-12: a level history naming more than two classes is not rewound into three classes;
//! - S-11: a journal step to a stage the game may lower (Carth's 3 over his talks) lowers it.

use kse_core::catalog::Catalog;
use kse_core::creature;
use kse_core::formats::gff::Value;
use kse_core::gamedata::GameData;
use kse_core::games::{find_installs, find_saves, Game};
use kse_core::questdb;
use kse_core::save::{self, Save, Who};

fn first_save(game: Game, ok: &dyn Fn(&Save) -> bool) -> Option<Save> {
    let installs = find_installs(&[]);
    find_saves(&installs, game, &[]).into_iter().filter(|f| !save::player_outside_archive(&f.path) && kse_core::save::Save::open(&f.path).is_ok()).filter_map(|f| Save::open(&f.path).ok()).find(|s| ok(s))
}

#[test]
fn a_member_who_stays_keeps_the_leader_flag() {
    let mut checked = 0;
    for game in [Game::K1, Game::K2] {
        let Some(mut s) = first_save(game, &|s| s.npcs().into_iter().filter(|n| s.recruited(*n) && (s.in_party(*n) || s.selectable(*n))).count() >= 2) else { continue };
        let two: Vec<usize> = s.npcs().into_iter().filter(|n| s.recruited(*n) && (s.in_party(*n) || s.selectable(*n))).take(2).collect();
        s.set_party_members(&two[..1]).unwrap();
        // the save made while the first member was led
        s.edit_party_table("lead", |p| {
            if let Some(Value::List(l)) = p.get_mut("PT_MEMBERS") {
                l[0].set_int("PT_IS_LEADER", 1, Value::Byte(0));
            }
            p.set_int("PT_CONTROLLED_NP", two[0] as i64, Value::Int(0));
        });
        s.set_party_members(&two).unwrap();
        let entries = s.party.root.list("PT_MEMBERS").unwrap();
        let first = entries.iter().find(|e| e.int("PT_MEMBER_ID") == Some(two[0] as i64)).unwrap();
        assert_eq!(first.int("PT_IS_LEADER"), Some(1), "{:?}: the member who stayed lost the leader flag", game);
        assert_eq!(s.party.root.int("PT_CONTROLLED_NP"), Some(two[0] as i64));
        checked += 1;
    }
    if checked == 0 {
        eprintln!("no save with two members to pick here: nothing checked");
    }
}

#[test]
fn a_member_the_story_keeps_out_is_not_put_in_the_party() {
    let mut checked = 0;
    for game in [Game::K1, Game::K2] {
        let Some(mut s) = first_save(game, &|s| s.npcs().into_iter().any(|n| s.recruited(n) && !s.in_party(n))) else { continue };
        let n = s.npcs().into_iter().find(|n| s.recruited(*n) && !s.in_party(*n)).unwrap();
        s.set_selectable(n, false);
        let mut members = s.party_members();
        members.truncate(1);
        members.push(n);
        assert!(s.set_party_members(&members).is_err(), "{:?}: a member the story keeps out was put in the party", game);
        assert!(!s.in_party(n));
        checked += 1;
    }
    if checked == 0 {
        eprintln!("no save with a recruited member outside the party here: nothing checked");
    }
}

#[test]
fn a_history_of_three_classes_is_not_rewound() {
    let installs = find_installs(&[]);
    let Some(install) = installs.iter().find(|i| i.game == Game::K1) else { return eprintln!("KotOR is not installed here: nothing checked") };
    let data = GameData::open(Game::K1, &install.path);
    let cat = Catalog::build(&data);
    let Some(s) = first_save(Game::K1, &|s| s.player().and_then(|p| p.list("LvlStatList")).map_or(false, |l| l.len() >= 3)) else {
        return eprintln!("no KotOR save with three levels here: nothing checked");
    };
    let mut p = s.player().unwrap().clone();
    assert!(creature::validate_rewind_prefix(&p, 3, &cat).is_ok(), "the save's own history should rewind");
    if let Some(Value::List(records)) = p.get_mut("LvlStatList") {
        for (i, r) in records.iter_mut().take(3).enumerate() {
            r.set_int("LvlStatClass", i as i64, Value::Byte(0));
        }
    }
    assert!(creature::validate_rewind_prefix(&p, 3, &cat).is_err(), "three classes in the history were accepted");
}

#[test]
fn a_journal_step_to_a_stage_the_game_may_lower_lowers_it() {
    let installs = find_installs(&[]);
    let Some(install) = installs.iter().find(|i| i.game == Game::K1) else { return eprintln!("KotOR is not installed here: nothing checked") };
    let data = GameData::open(Game::K1, &install.path);
    // Carth's quest at 3 (his revenge on the Leviathan) is set over his talks' later stages
    let tag = "k_swg_carthtalk";
    let lowered: Vec<i64> = questdb::lowered_stages(Game::K1, tag).into_iter().map(|(s, _)| s).collect();
    assert!(lowered.contains(&3), "Carth's 3 is a stage the game may lower: {:?}", lowered);
    let Some(mut s) = first_save(Game::K1, &|_| true) else { return eprintln!("no KotOR save here: nothing checked") };
    s.set_quest(tag, 70);
    let step: questdb::Step = serde_json::from_value(serde_json::json!({ "does": [{ "journal": [tag, 3] }] })).unwrap();
    questdb::apply(&mut s, &data, &step, "test");
    let stage = |s: &Save| s.quests().into_iter().find(|(t, _)| t.eq_ignore_ascii_case(tag)).map(|(_, v)| v);
    assert_eq!(stage(&s), Some(3), "the stage was not lowered");
    // a stage the game may not lower stays where it is
    s.set_quest(tag, 70);
    let down: questdb::Step = serde_json::from_value(serde_json::json!({ "does": [{ "journal": [tag, 20] }] })).unwrap();
    assert!(!lowered.contains(&20));
    questdb::apply(&mut s, &data, &down, "test");
    assert_eq!(stage(&s), Some(70), "a stage the game does not lower was lowered");
    let _ = Who::Player;
}
