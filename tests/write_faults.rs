//! Save Changes under failure (2026-11-07 audit). Every case works on a copy of a real save
//! of each game in the system's temporary folder; the PC's saves are only read.
//!
//! For each failure the save must be byte for byte as it was, nothing of the write may be
//! left in its folder, the error must not claim a half-written save when none is, and the
//! changes must still be in memory to try again. Then the same write, with the cause gone,
//! must go through and read back.
//!
//! - a save file held open part way through the swap (it can be read, not renamed);
//! - the staged file cannot be created (a folder in its place);
//! - the backup cannot be made (a file where the backups folder goes);
//! - the save changed on disk after it was opened;
//! - an original Xbox save;
//! - a save file marked read-only;
//! - a write with no change, and many writes in a row.

use kse_core::creature;
use kse_core::games::{find_installs, find_saves, Game};
use kse_core::save::{self, Save, Who};
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
    let mut out: Vec<(String, Vec<u8>)> = fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| (e.file_name().to_string_lossy().to_string(), if e.path().is_file() { fs::read(e.path()).unwrap() } else { b"<dir>".to_vec() }))
        .collect();
    out.sort();
    out
}

struct Lab {
    root: PathBuf,
    save: PathBuf,
    backups: PathBuf,
    game: Game,
}

fn labs(test: &str) -> Vec<Lab> {
    let installs = find_installs(&[]);
    let mut v = Vec::new();
    for game in [Game::K1, Game::K2] {
        let Some(folder) = find_saves(&installs, game, &[]).into_iter().find(|f| !save::player_outside_archive(&f.path)) else { continue };
        let root = std::env::temp_dir().join("kmse-tests").join("write-faults").join(test).join(format!("{:?}", game));
        let _ = fs::remove_dir_all(&root);
        let save = root.join("saves").join("000500 - Game499");
        copy_dir(&folder.path, &save);
        v.push(Lab { backups: root.join("Save Editor Backup"), root, save, game });
    }
    if v.is_empty() {
        eprintln!("no save to copy: skipped");
    }
    v
}

/// Changes that make every file of the save be written: the player's XP (SAVEGAME.sav),
/// credits (PARTYTABLE.res, and savenfo.res with it), a story value (GLOBALVARS.res).
fn change_everything(sv: &mut Save, by: i64) -> (i64, i64) {
    let xp = sv.player().map(creature::experience).unwrap_or(0) + by;
    sv.edit(Who::Player, "xp", |c| creature::set_experience(c, xp));
    let credits = sv.party_int("PT_GOLD").unwrap_or(0) + by;
    sv.set_party_int("PT_GOLD", credits);
    sv.rename(&format!("write faults {}", by));
    // a story number flipped by `by`'s last bit: GLOBALVARS.res is written too
    sv.edit_globals("audit", |g| {
        if let Some(n) = kse_core::globals::list(g).into_iter().find(|x| matches!(x.value, kse_core::globals::GlobalValue::Bool(_))).map(|x| x.index) {
            kse_core::globals::set_bool(g, n, by % 2 == 1);
        }
    });
    (xp, credits)
}

fn xp_and_credits(folder: &Path) -> (i64, i64) {
    let sv = Save::open(folder).unwrap();
    (sv.player().map(creature::experience).unwrap_or(0), sv.party_int("PT_GOLD").unwrap_or(0))
}

fn clean(folder: &Path) -> bool {
    fs::read_dir(folder).unwrap().filter_map(|e| e.ok()).all(|e| {
        let n = e.file_name().to_string_lossy().to_ascii_lowercase();
        !n.ends_with(".kmrp-new") && !n.ends_with(".kmrp-old")
    })
}

/// A failed write: the save as it was, nothing left behind, not called half written, the
/// changes kept to try again.
fn failed_cleanly(lab: &Lab, sv: &Save, before: &[(String, Vec<u8>)], r: Result<PathBuf, String>, why: &str) {
    let e = r.err().unwrap_or_else(|| panic!("{:?}: {}: the write must fail", lab.game, why));
    assert!(!e.starts_with(save::HALF_WRITTEN), "{:?}: {}: not half written ({})", lab.game, why, e);
    assert_eq!(files(&lab.save), before, "{:?}: {}: the save is as it was", lab.game, why);
    assert!(clean(&lab.save), "{:?}: {}: nothing of the write is left", lab.game, why);
    assert!(!sv.changes.is_empty(), "{:?}: {}: the changes are kept to try again", lab.game, why);
}

#[cfg(windows)]
#[test]
fn a_file_held_open_part_way_through_the_swap() {
    use std::os::windows::fs::OpenOptionsExt;
    for lab in labs("held") {
        let mut sv = Save::open(&lab.save).unwrap();
        sv.back_up(&lab.backups).unwrap();
        let want = change_everything(&mut sv, 777);
        let before = files(&lab.save);
        // savenfo.res is swapped last of the four (archive, party table, savenfo, globals
        // before it only when there are globals): earlier swaps have happened and go back
        for held in ["savenfo.res", "SAVEGAME.sav"] {
            let path = fs::read_dir(&lab.save).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).find(|p| p.file_name().unwrap().to_string_lossy().eq_ignore_ascii_case(held)).unwrap();
            // readable (the backup check reads it), but it cannot be renamed
            let lock = fs::OpenOptions::new().read(true).share_mode(1).open(&path).unwrap();
            let r = sv.write(&lab.backups);
            drop(lock);
            failed_cleanly(&lab, &sv, &before, r, &format!("{} held open", held));
        }
        sv.write(&lab.backups).unwrap();
        assert_eq!(xp_and_credits(&lab.save), want, "{:?}: written once let go", lab.game);
        let _ = fs::remove_dir_all(&lab.root);
    }
}

#[test]
fn the_staged_file_cannot_be_created() {
    for lab in labs("staging") {
        let mut sv = Save::open(&lab.save).unwrap();
        sv.back_up(&lab.backups).unwrap();
        let want = change_everything(&mut sv, 55);
        // a folder where the staged savenfo goes, made after the save was opened
        let blocker = lab.save.join("savenfo.kmrp-new");
        let before = files(&lab.save);
        fs::create_dir_all(&blocker).unwrap();
        let r = sv.write(&lab.backups);
        fs::remove_dir_all(&blocker).unwrap();
        failed_cleanly(&lab, &sv, &before, r, "staging blocked");
        sv.write(&lab.backups).unwrap();
        assert_eq!(xp_and_credits(&lab.save), want);
        let _ = fs::remove_dir_all(&lab.root);
    }
}

#[test]
fn the_backup_cannot_be_made() {
    for lab in labs("no-backup") {
        let mut sv = Save::open(&lab.save).unwrap();
        // no backup on opening, and a file where the backups folder goes
        fs::create_dir_all(lab.backups.parent().unwrap()).unwrap();
        fs::write(&lab.backups, b"not a folder").unwrap();
        let want = change_everything(&mut sv, 9);
        let before = files(&lab.save);
        let r = sv.write(&lab.backups);
        failed_cleanly(&lab, &sv, &before, r, "no backup possible");
        fs::remove_file(&lab.backups).unwrap();
        let b = sv.write(&lab.backups).unwrap();
        assert_eq!(files(&b), before, "{:?}: the backup made is the save before the write", lab.game);
        assert_eq!(xp_and_credits(&lab.save), want);
        let _ = fs::remove_dir_all(&lab.root);
    }
}

#[test]
fn a_save_changed_on_disk_after_opening_is_not_written() {
    for lab in labs("changed-on-disk") {
        let mut sv = Save::open(&lab.save).unwrap();
        sv.back_up(&lab.backups).unwrap();
        change_everything(&mut sv, 3);
        // the game saving into the slot meanwhile: a file of it grows
        let nfo = fs::read_dir(&lab.save).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).find(|p| p.file_name().unwrap().to_string_lossy().eq_ignore_ascii_case("savenfo.res")).unwrap();
        let mut bytes = fs::read(&nfo).unwrap();
        bytes.push(0);
        fs::write(&nfo, &bytes).unwrap();
        let before = files(&lab.save);
        let r = sv.write(&lab.backups);
        assert_eq!(r.as_ref().err().map(String::as_str), Some(save::CHANGED_ON_DISK), "{:?}", lab.game);
        failed_cleanly(&lab, &sv, &before, r, "changed on disk");
        let _ = fs::remove_dir_all(&lab.root);
    }
}

#[test]
fn an_xbox_save_is_never_written() {
    for lab in labs("xbox") {
        // there when the save is opened, so the write is refused for being an Xbox save, not
        // for a file that appeared after opening
        fs::write(lab.save.join("SaveMeta.xbx"), b"signature").unwrap();
        let mut sv = Save::open(&lab.save).unwrap();
        change_everything(&mut sv, 1);
        let before = files(&lab.save);
        let r = sv.write(&lab.backups);
        assert_eq!(r.as_ref().err().map(String::as_str), Some(save::XBOX_SAVE), "{:?}", lab.game);
        failed_cleanly(&lab, &sv, &before, r, "xbox save");
        let target = lab.root.join("saves").join("000501 - Game500");
        assert_eq!(sv.write_as(&target, "xbox copy").err().as_deref(), Some(save::XBOX_SAVE), "{:?}: no Save As of it either", lab.game);
        assert!(!target.exists());
        let _ = fs::remove_dir_all(&lab.root);
    }
}

#[test]
fn a_read_only_save_file_fails_cleanly_or_saves_whole() {
    for lab in labs("read-only") {
        let mut sv = Save::open(&lab.save).unwrap();
        sv.back_up(&lab.backups).unwrap();
        let want = change_everything(&mut sv, 12);
        let before = files(&lab.save);
        let party = fs::read_dir(&lab.save).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).find(|p| p.file_name().unwrap().to_string_lossy().eq_ignore_ascii_case("PARTYTABLE.res")).unwrap();
        let mut perm = fs::metadata(&party).unwrap().permissions();
        perm.set_readonly(true);
        fs::set_permissions(&party, perm.clone()).unwrap();
        let r = sv.write(&lab.backups);
        // whatever Windows allows, never half: all four new, or all four as they were
        for e in fs::read_dir(&lab.save).unwrap().filter_map(|e| e.ok()) {
            let mut p = fs::metadata(e.path()).unwrap().permissions();
            if p.readonly() {
                p.set_readonly(false);
                fs::set_permissions(e.path(), p).unwrap();
            }
        }
        match r {
            Ok(_) => assert_eq!(xp_and_credits(&lab.save), want, "{:?}: written whole", lab.game),
            Err(e) => {
                assert!(!e.starts_with(save::HALF_WRITTEN), "{:?}: {}", lab.game, e);
                assert_eq!(files(&lab.save), before, "{:?}: as it was", lab.game);
            }
        }
        assert!(save::interrupted_write(&lab.save).is_none());
        let _ = fs::remove_dir_all(&lab.root);
    }
}

#[test]
fn a_write_with_no_change_leaves_the_save_as_it_was_and_many_writes_in_a_row_hold() {
    for lab in labs("no-change") {
        let before = files(&lab.save);
        let mut sv = Save::open(&lab.save).unwrap();
        sv.back_up(&lab.backups).unwrap();
        sv.write(&lab.backups).unwrap();
        assert_eq!(files(&lab.save), before, "{:?}: nothing changed, nothing written", lab.game);
        for round in 1..=8 {
            let mut sv = Save::open(&lab.save).unwrap();
            sv.back_up(&lab.backups).unwrap();
            let want = change_everything(&mut sv, round * 100);
            let pre = files(&lab.save);
            let b = sv.write(&lab.backups).unwrap();
            assert_eq!(files(&b), pre, "{:?}: round {}: the backup is the save before the write", lab.game, round);
            assert_eq!(xp_and_credits(&lab.save), want, "{:?}: round {}", lab.game, round);
            assert!(clean(&lab.save));
        }
        let _ = fs::remove_dir_all(&lab.root);
    }
}
