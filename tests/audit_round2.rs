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

/// While a trip waits in the queue (Skip Ahead, Go Back and Travel Here set one), neither the
/// area the save stands in nor the trip's says what was played: Go Back leaves the save in the
/// area gone back from and sends it to the Ebon Hawk or a cutscene area, Travel Here can send it
/// to a later planet. The journal alone decides then.
#[test]
fn a_queued_trip_leaves_the_journal_to_decide() {
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
    let all = questdb::milestones_ref(Game::K1);
    let at = |p: &str| all.iter().position(|(x, _)| x.eq_ignore_ascii_case(p)).unwrap();
    let mut s = Save::open(&folder.path).unwrap();
    // the Ebon Hawk (where most returns send the save), a later planet, a cutscene area
    for trip in ["ebo_m12aa", "sta_m45aa", "stunt_06", "unk_m41aa"] {
        s.queue_transition(trip, "wp", &[]).unwrap();
        assert_eq!(s.queued_trip().as_deref(), Some(trip));
        assert_eq!(s.place(), trip);
        let played = questdb::chapters_played_in(&s);
        for p in ["Leviathan", "Unknown World", "Star Forge"] {
            assert!(!played[at(p)], "{} read played with a trip to {} waiting", p, trip);
        }
    }
}
