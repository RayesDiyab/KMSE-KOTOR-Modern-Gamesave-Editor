//! Regressions of the 2026-11-07 audit, on copies of the saves on this PC (in memory, or in
//! the system's temporary folder); each fails against the code before its fix.
//!
//! - a hurt character whose Constitution is lowered keeps at least 1 Health (it went below 0);
//! - a companion brought from another save: the same person always, someone else only from a
//!   save at least as far along the story (an earlier save's stand-in is refused);
//! - a link (a junction) inside a save folder is never followed by a backup or a write.

use kse_core::creature;
use kse_core::formats::gff::Value;
use kse_core::games::{find_installs, find_saves, Game};
use kse_core::save::{self, Save, Who};
use std::fs;
use std::path::Path;

fn saves() -> Vec<(Game, Save)> {
    let installs = find_installs(&[]);
    let mut v = Vec::new();
    for game in [Game::K1, Game::K2] {
        if let Some(s) = find_saves(&installs, game, &[]).into_iter().filter(|f| !save::player_outside_archive(&f.path)).find_map(|f| Save::open(&f.path).ok()) {
            v.push((game, s));
        }
    }
    if v.is_empty() {
        eprintln!("no saves here: nothing checked");
    }
    v
}

#[test]
fn lowering_constitution_never_leaves_a_hurt_character_dead() {
    for (game, original) in saves() {
        let mut c = original.player().unwrap().clone();
        creature::set_attribute(&mut c, "Con", 18);
        creature::keep_in_range(&mut c, game);
        // hurt: 2 Health left of the maximum, stored as the game stores it
        let (_, max) = creature::health(&c);
        let base = c.int("HitPoints").unwrap_or(0);
        let stored = c.int("CurrentHitPoints").unwrap_or(base);
        let (now, _) = creature::health(&c);
        c.set_int("CurrentHitPoints", stored - (now - 2), Value::Short(0));
        assert_eq!(creature::health(&c).0, 2, "{:?}: set up hurt", game);
        let mut without = c.clone();
        creature::set_attribute(&mut without, "Con", 3);
        let fell = creature::health(&without).0;
        creature::set_attribute(&mut c, "Con", 3);
        creature::keep_in_range(&mut c, game);
        let (after, max_after) = creature::health(&c);
        assert!(after >= 1, "{:?}: {} Health left (max {} from {})", game, after, max_after, max);
        assert!(after <= max_after);
        assert!(fell < 1, "{:?}: the case is one the check is for ({} without it)", game, fell);
        eprintln!("{:?}: without the check the Health would have been {}", game, fell);
    }
}

#[test]
fn a_companion_from_another_save_must_be_the_same_person() {
    for (game, original) in saves() {
        let Some(&npc) = original.npcs().first() else { continue };
        // the same person: brought over
        let mut here = original.clone();
        here.import_npc(&original, npc).unwrap_or_else(|e| panic!("{:?}: the same person is refused: {}", game, e));
        // someone else in that slot in a save at the same point of the story: allowed (two
        // saves in one chapter; second review), and the slot then holds them
        let mut other = original.clone();
        other.edit(Who::Npc(npc), "tag", |c| c.set("Tag", Value::String(b"someone_else".to_vec())));
        let mut here = original.clone();
        here.import_npc(&other, npc).unwrap_or_else(|e| panic!("{:?}: someone else at the same point: {}", game, e));
        assert_eq!(here.creature(Who::Npc(npc)).and_then(|c| c.string("Tag")).as_deref(), Some("someone_else"));
    }
}

#[cfg(windows)]
#[test]
fn a_link_inside_a_save_folder_is_never_followed() {
    let installs = find_installs(&[]);
    let Some(folder) = [Game::K1, Game::K2].into_iter().find_map(|g| find_saves(&installs, g, &[]).into_iter().find(|f| !save::player_outside_archive(&f.path))) else {
        return eprintln!("no save here: nothing checked");
    };
    let root = std::env::temp_dir().join("kmse-tests").join("junction");
    let _ = fs::remove_dir_all(&root);
    let save_dir = root.join("saves").join("000500 - Game499");
    fs::create_dir_all(&save_dir).unwrap();
    for e in fs::read_dir(&folder.path).unwrap().filter_map(|e| e.ok()) {
        if e.path().is_file() {
            fs::copy(e.path(), save_dir.join(e.file_name())).unwrap();
        }
    }
    // somewhere else on the disk, and a junction to it inside the save folder
    let outside = root.join("outside");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("private.txt"), b"not the save's").unwrap();
    let made = std::process::Command::new("cmd").args(["/C", "mklink", "/J"]).arg(save_dir.join("link")).arg(&outside).output().unwrap();
    if !made.status.success() || !save_dir.join("link").exists() {
        return eprintln!("a junction could not be made here: nothing checked");
    }
    let backups = root.join("Save Editor Backup");
    let mut sv = Save::open(&save_dir).unwrap();
    assert!(sv.back_up(&backups).is_err(), "no backup made through the link");
    let xp = sv.player().map(creature::experience).unwrap_or(0);
    sv.edit(Who::Player, "xp", |c| creature::set_experience(c, xp + 1));
    let before: Vec<_> = fs::read_dir(&save_dir).unwrap().filter_map(|e| e.ok()).map(|e| e.file_name()).collect();
    assert!(sv.write(&backups).is_err(), "no write without a backup it can make");
    let after: Vec<_> = fs::read_dir(&save_dir).unwrap().filter_map(|e| e.ok()).map(|e| e.file_name()).collect();
    assert_eq!(before, after, "the save folder is as it was");
    assert_eq!(fs::read(outside.join("private.txt")).unwrap(), b"not the save's");
    // nothing of the outside folder copied into the backups
    let copied = walk(&backups).iter().any(|p| p.file_name().map_or(false, |n| n == "private.txt"));
    assert!(!copied, "the linked folder's files are not copied");
    let _ = fs::remove_dir(save_dir.join("link"));
    let _ = fs::remove_dir_all(&root);
}

fn walk(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.filter_map(|e| e.ok()) {
            let p = e.path();
            if e.file_type().map_or(false, |t| t.is_dir()) {
                out.extend(walk(&p));
            } else {
                out.push(p);
            }
        }
    }
    out
}

/// KotOR's slot 0: the Endar Spire's Trask (a stand-in) and Bastila after Taris. Bastila comes
/// over into a save at the Endar Spire, as the story moves the slot on; Trask never goes into
/// a save where Bastila has the slot.
#[test]
fn the_endar_spire_stand_in_never_replaces_bastila() {
    let installs = find_installs(&[]);
    let opened: Vec<Save> = find_saves(&installs, Game::K1, &[]).into_iter().filter(|f| !save::player_outside_archive(&f.path)).filter_map(|f| Save::open(&f.path).ok()).collect();
    let tag = |s: &Save| s.creature(Who::Npc(0)).and_then(|c| c.string("Tag")).unwrap_or_default().to_ascii_lowercase();
    let (Some(trask), Some(bastila)) = (opened.iter().find(|s| tag(s) == "end_trask"), opened.iter().find(|s| tag(s) == "bastila" && s.recruited(0))) else {
        return eprintln!("no KotOR saves at the Endar Spire and after Taris here: nothing checked");
    };
    let mut later = bastila.clone();
    assert!(later.import_npc(trask, 0).is_err(), "Trask is not put in Bastila's place");
    assert_eq!(tag(&later), "bastila");
    let mut earlier = trask.clone();
    earlier.import_npc(bastila, 0).unwrap_or_else(|e| panic!("Bastila comes over to the Endar Spire: {}", e));
    assert_eq!(tag(&earlier), "bastila");
}

fn leader_flags(s: &Save) -> (usize, i64) {
    let leads = s.party.root.list("PT_MEMBERS").map_or(0, |l| l.iter().filter(|m| m.int("PT_IS_LEADER").unwrap_or(0) != 0).count());
    (leads, s.party.root.int("PT_CONTROLLED_NP").unwrap_or(-1))
}

/// A save moved to another point of the story leaves no party member marked as the one the
/// player controls. The Endar Spire's "test" save marks slot 0 (Trask) leader; carried to the
/// end of Taris, slot 0 is Bastila, the game queued a switch to her on every area load, and
/// after the turret game it crashed in CrownFutureLeader (KMRP's agent, 2026-11-07, confirmed
/// in the game with the flag cleared). Skip Ahead and Go Back both clear it.
#[test]
fn a_story_move_leaves_the_player_leading() {
    let installs = find_installs(&[]);
    let Some(install) = installs.iter().find(|i| i.game == Game::K1) else { return eprintln!("KotOR is not installed here: nothing checked") };
    let data = kse_core::gamedata::GameData::open(Game::K1, &install.path);
    let order = |m: &serde_json::Value| m.get("order").and_then(|v| v.as_i64()).unwrap_or(99);
    let upto = |planet: &str| {
        let all = kse_core::questdb::milestones(Game::K1);
        let end = all.iter().find(|(p, _)| p.eq_ignore_ascii_case(planet)).map(|(_, m)| order(m)).unwrap();
        let mut path: Vec<_> = all.into_iter().map(|(_, m)| m).filter(|m| m.get("side_chapter").is_none() && order(m) <= end).collect();
        path.sort_by_key(order);
        path
    };
    let mut checked = 0;
    for folder in find_saves(&installs, Game::K1, &[]) {
        let Ok(original) = Save::open(&folder.path) else { continue };
        if leader_flags(&original).0 == 0 || original.module_name().to_ascii_lowercase() != "end_m01aa" {
            continue;
        }
        // Skip Ahead from the Endar Spire to the end of Taris
        let plan = kse_core::story_preview::Plan::Skip { chapters: upto("Taris"), stage: None, class: None, dark: false, prestige: None, train: false };
        let skipped = plan.prepare(&original, &data).unwrap().save;
        assert_eq!(leader_flags(&skipped), (0, -1), "{}: after Skip Ahead the player leads", folder.path.display());
        // Go Back over Taris, from a save whose party table marks a member leader again
        let mut marked = skipped.clone();
        marked.edit_party_table("leader-test", |p| {
            if let Some(m) = p.list_mut("PT_MEMBERS").and_then(|l| l.first_mut()) {
                m.set_int("PT_IS_LEADER", 1, Value::Byte(0));
            }
        });
        if leader_flags(&marked).0 == 0 {
            continue; // no member to mark
        }
        let path = upto("Taris");
        let kept = path[..path.len() - 1].to_vec();
        kse_core::questdb::go_back(&mut marked, &data, &kept, &[path.last().unwrap().clone()], false, None).unwrap();
        assert_eq!(leader_flags(&marked), (0, -1), "{}: after Go Back the player leads", folder.path.display());
        checked += 1;
    }
    eprintln!("{} Endar Spire saves with a member leading checked", checked);
}
