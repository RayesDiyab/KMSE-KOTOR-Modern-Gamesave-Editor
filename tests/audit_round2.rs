//! Findings of the second audit round of 2026-11-06 that a test can reach. The folders here are
//! made in the system's temporary folder; saves are read from the PC and changed in memory only.
//! A test that needs a save passes without checking anything on a PC without one, and says so.

use kse_core::games::{find_installs, find_saves, Game};
use kse_core::questdb;
use kse_core::save::{self, Save};
use std::fs;

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("kmse-round2-{}-{}", name, std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// A write that finished but could not remove an old file (a virus scanner held it) is not a
/// write cut off part way: the save still opens. One cut off part way is still refused.
#[test]
fn a_finished_write_is_not_taken_for_one_cut_off() {
    let dir = scratch("leftover");
    fs::write(dir.join("savenfo.res"), b"new").unwrap();
    fs::write(dir.join("savenfo.kmrp-old"), b"old").unwrap();
    assert_eq!(save::interrupted_write(&dir), None, "an old file beside its new one is only clean-up");
    fs::remove_file(dir.join("savenfo.res")).unwrap();
    assert!(save::interrupted_write(&dir).is_some(), "an old file whose file is gone: cut off");
    fs::write(dir.join("savenfo.res"), b"new").unwrap();
    fs::write(dir.join("PARTYTABLE.kmrp-new"), b"new").unwrap();
    assert!(save::interrupted_write(&dir).is_some(), "a new file not yet in place: cut off");
    let _ = fs::remove_dir_all(&dir);
}

/// After Skip Ahead or Go Back the save stands where it was until the game makes the queued
/// trip, so where it will be is the trip's area. Before, a return from the Unknown World read
/// every planet before it played again, from the area the save still stood in.
#[test]
fn a_queued_trip_decides_where_the_save_is() {
    let installs = find_installs(&[]);
    let Some(folder) = find_saves(&installs, Game::K1, &[])
        .into_iter()
        // a save whose journal has not reached the Leviathan, with no trip waiting
        .find(|f| {
            !save::player_outside_archive(&f.path)
                && Save::open(&f.path).map_or(false, |s| {
                    let lev = questdb::milestones_ref(Game::K1).iter().position(|(p, _)| *p == "Leviathan").unwrap();
                    s.queued_trip().is_none() && !questdb::chapters_played(Game::K1, &s.quests())[lev]
                })
        })
    else {
        return eprintln!("no KotOR save here: nothing checked");
    };
    let mut s = Save::open(&folder.path).unwrap();
    // a trip to Kashyyyk's first area, as a return to Kashyyyk queues one
    s.queue_transition("kas_m22aa", "kas_m22aa_from_ebo", &[]).unwrap();
    assert_eq!(s.queued_trip().as_deref(), Some("kas_m22aa"));
    assert_eq!(s.place(), "kas_m22aa");
    let played = questdb::chapters_played_in(&s);
    let all = questdb::milestones_ref(Game::K1);
    let at = |p: &str| all.iter().position(|(x, _)| x.eq_ignore_ascii_case(p)).unwrap();
    for p in ["Endar Spire", "Taris", "Dantooine"] {
        assert!(played[at(p)], "{} lies before Kashyyyk on the line", p);
    }
    for p in ["Leviathan", "Unknown World", "Star Forge"] {
        assert!(!played[at(p)], "{} does not come before Kashyyyk", p);
    }
    // a trip to a later area (Travel Here can queue one) says nothing about what was played:
    // the earlier of the two places counts
    s.queue_transition("sta_m45aa", "sta_m45aa_start", &[]).unwrap();
    let played = questdb::chapters_played_in(&s);
    for p in ["Leviathan", "Unknown World", "Tatooine", "Korriban"] {
        assert!(!played[at(p)], "{} does not read played from a trip to the Star Forge", p);
    }
}
