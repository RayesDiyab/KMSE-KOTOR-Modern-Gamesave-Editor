//! History's Revert to This Backup (2026-11-07, the maintainer: "a safe feature ... impossible
//! to break. Test it extensively"). Everything here works on copies of the saves on this PC in
//! the system's temporary folder; no real save or real backup is touched.
//!
//! - a restore puts the save back byte for byte, keeps the save as it was in a checked backup,
//!   and can itself be undone from that backup;
//! - files the backup lacks are removed, files the save lacks are put back;
//! - every refusal leaves the save byte for byte as it was and makes no backup;
//! - a file held open part way through the swap (Windows) puts every file back as it was;
//! - after a restore the save opens, backs up and saves as any other.

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

/// Every file of a folder with its bytes, in name order: what "the same" means here.
fn files(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out: Vec<(String, Vec<u8>)> = fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| (e.file_name().to_string_lossy().to_string(), if e.path().is_file() { fs::read(e.path()).unwrap() } else { b"<dir>".to_vec() }))
        .collect();
    out.sort();
    out
}

/// A copy of a real save of `game` in its own scratch folder, laid out as on a PC: the save
/// in `saves/`, its backups in `Save Editor Backup/`. None where this PC has no such save.
struct Lab {
    root: PathBuf,
    save: PathBuf,
    backups: PathBuf,
    game: Game,
}

fn lab(game: Game, test: &str) -> Option<Lab> {
    let installs = find_installs(&[]);
    let folder = find_saves(&installs, game, &[]).into_iter().find(|f| !save::player_outside_archive(&f.path) && kse_core::save::Save::open(&f.path).is_ok())?;
    let root = std::env::temp_dir().join("kmse-tests").join("restore").join(test).join(format!("{:?}", game));
    let _ = fs::remove_dir_all(&root);
    let save = root.join("saves").join("000500 - Game499");
    copy_dir(&folder.path, &save);
    Some(Lab { backups: root.join("Save Editor Backup"), root, save, game })
}

impl Lab {
    /// Opens the save (making its backup), changes the player's XP by `by` and saves.
    fn edit_and_save(&self, by: i64) {
        let mut sv = Save::open(&self.save).unwrap();
        sv.back_up(&self.backups).unwrap();
        let xp = sv.player().map(creature::experience).unwrap_or(0);
        sv.edit(Who::Player, "xp", |c| creature::set_experience(c, xp + by));
        sv.write(&self.backups).unwrap();
    }
    fn xp(&self) -> i64 {
        Save::open(&self.save).unwrap().player().map(creature::experience).unwrap_or(0)
    }
    fn list(&self) -> Vec<PathBuf> {
        save::backup_list(&self.backups, &self.save)
    }
    /// A restore that must be refused: the save stays byte for byte, no backup is made, and
    /// nothing of a write is left in the save folder.
    fn refused(&self, backup: &Path, why: &str) {
        let before = files(&self.save);
        let backups_before = self.list();
        let r = save::restore(&self.save, backup, &self.backups, self.game);
        assert!(r.is_err(), "{:?}: {}: refused", self.game, why);
        let e = r.err().unwrap();
        assert!(!e.starts_with(save::HALF_WRITTEN), "{:?}: {}: refused cleanly, not half written ({})", self.game, why, e);
        assert_eq!(files(&self.save), before, "{:?}: {}: the save is untouched", self.game, why);
        assert_eq!(self.list(), backups_before, "{:?}: {}: no backup made", self.game, why);
    }
    fn done(self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn games(test: &str) -> Vec<Lab> {
    let v: Vec<Lab> = [Game::K1, Game::K2].into_iter().filter_map(|g| lab(g, test)).collect();
    if v.is_empty() {
        eprintln!("no save to copy: skipped");
    }
    v
}

#[test]
fn a_restore_puts_the_save_back_and_can_be_undone() {
    for lab in games("round-trip") {
        let g = lab.game;
        let original = files(&lab.save);
        let xp0 = lab.xp();
        lab.edit_and_save(500);
        lab.edit_and_save(700);
        assert_eq!(lab.xp(), xp0 + 500 + 700);
        let edited = files(&lab.save);
        let first = lab.list()[0].clone();
        assert_eq!(files(&first), original, "{:?}: the first backup is the save as it came", g);
        let count = lab.list().len();

        let r = save::restore(&lab.save, &first, &lab.backups, g).unwrap();
        assert_eq!(files(&lab.save), original, "{:?}: the save is the backup again, byte for byte", g);
        assert_eq!(lab.xp(), xp0);
        assert_eq!(files(&r.before), edited, "{:?}: the save as it was before is kept", g);
        assert_eq!(lab.list().len(), count + 1, "{:?}: one backup more", g);
        assert_eq!(lab.list().last(), Some(&r.before), "{:?}: and it is the newest", g);
        assert_eq!(files(&first), original, "{:?}: the backup restored from is unchanged", g);

        // undone: the backup made before the restore puts the edits back
        let undo = save::restore(&lab.save, &r.before, &lab.backups, g).unwrap();
        assert_eq!(files(&lab.save), edited, "{:?}: the restore undone", g);
        assert_eq!(files(&undo.before), original);

        // the same again is refused, as there is nothing to do
        lab.refused(&r.before, "already the same");
        lab.done();
    }
}

#[test]
fn files_the_backup_lacks_go_and_files_the_save_lacks_come_back() {
    for lab in games("files") {
        let g = lab.game;
        let mut sv = Save::open(&lab.save).unwrap();
        let backup = sv.back_up(&lab.backups).unwrap();
        let original = files(&lab.save);
        // a file added to the save since, and one of the save's own taken away
        fs::write(lab.save.join("notes.txt"), b"written after the backup").unwrap();
        let screen = fs::read_dir(&lab.save).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).find(|p| p.extension().map_or(false, |x| x.eq_ignore_ascii_case("tga")));
        if let Some(s) = &screen {
            fs::remove_file(s).unwrap();
        }
        let changed = files(&lab.save);
        let r = save::restore(&lab.save, &backup, &lab.backups, g).unwrap();
        assert_eq!(files(&lab.save), original, "{:?}: exactly the backup's files", g);
        assert!(!lab.save.join("notes.txt").exists());
        assert_eq!(files(&r.before), changed, "{:?}: the added file is kept in the backup made first", g);
        lab.done();
    }
}

#[test]
fn every_refusal_leaves_the_save_as_it_was() {
    for lab in games("refusals") {
        let g = lab.game;
        let mut sv = Save::open(&lab.save).unwrap();
        let good = sv.back_up(&lab.backups).unwrap();
        lab.edit_and_save(100);
        let own = good.parent().unwrap().to_path_buf();

        // the save itself
        lab.refused(&lab.save, "the save as its own backup");
        // a folder that is not one of this save's backups
        let elsewhere = lab.root.join("elsewhere").join("2026-01-01 00-00-00");
        copy_dir(&good, &elsewhere);
        lab.refused(&elsewhere, "a copy outside the backups");
        let other_save = lab.backups.join("000777 - Game776").join("2026-01-01 00-00-00");
        copy_dir(&good, &other_save);
        lab.refused(&other_save, "another save's backup");
        // the backups folder of the save, not one backup in it
        lab.refused(&own, "the folder of backups itself");
        // a folder that does not exist
        lab.refused(&own.join("1999-01-01 00-00-00"), "a backup that is not there");

        // damaged: the archive cut short
        let cut = own.join("2026-01-02 00-00-00");
        copy_dir(&good, &cut);
        let sav = fs::read_dir(&cut).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).find(|p| p.extension().map_or(false, |x| x.eq_ignore_ascii_case("sav"))).unwrap();
        let bytes = fs::read(&sav).unwrap();
        fs::write(&sav, &bytes[..bytes.len() / 3]).unwrap();
        lab.refused(&cut, "a damaged archive");
        // damaged: savenfo.res gone
        let no_nfo = own.join("2026-01-03 00-00-00");
        copy_dir(&good, &no_nfo);
        for e in fs::read_dir(&no_nfo).unwrap().filter_map(|e| e.ok()) {
            if e.file_name().to_string_lossy().eq_ignore_ascii_case("savenfo.res") {
                fs::remove_file(e.path()).unwrap();
            }
        }
        lab.refused(&no_nfo, "no savenfo.res");
        // empty
        let empty = own.join("2026-01-04 00-00-00");
        fs::create_dir_all(&empty).unwrap();
        lab.refused(&empty, "an empty folder");
        // a folder inside it
        let nested = own.join("2026-01-05 00-00-00");
        copy_dir(&good, &nested);
        fs::create_dir_all(nested.join("inner")).unwrap();
        lab.refused(&nested, "a folder inside the backup");
        // a write's left-over file in the backup
        let leftover = own.join("2026-01-06 00-00-00");
        copy_dir(&good, &leftover);
        fs::write(leftover.join("savenfo.kmrp-new"), b"x").unwrap();
        lab.refused(&leftover, "a write's file in the backup");
        // two files with one stem: the staged names would collide
        let stems = own.join("2026-01-07 00-00-00");
        copy_dir(&good, &stems);
        fs::write(stems.join("SAVEGAME.txt"), b"x").unwrap();
        lab.refused(&stems, "two files sharing a stem");

        // a write cut off in the save: nothing restored over it
        fs::write(lab.save.join("PARTYTABLE.kmrp-new"), b"half").unwrap();
        lab.refused(&good, "a cut-off write in the save");
        fs::remove_file(lab.save.join("PARTYTABLE.kmrp-new")).unwrap();
        // a folder inside the save
        fs::create_dir_all(lab.save.join("inner")).unwrap();
        lab.refused(&good, "a folder inside the save");
        fs::remove_dir_all(lab.save.join("inner")).unwrap();

        // the good one still works after all that
        save::restore(&lab.save, &good, &lab.backups, g).unwrap();
        assert!(save::same_files(&lab.save, &good));
        lab.done();
    }
}

#[test]
fn a_backup_of_the_other_game_is_refused() {
    let (Some(k1), Some(k2)) = (lab(Game::K1, "other-game"), lab(Game::K2, "other-game")) else {
        eprintln!("needs a save of each game: skipped");
        return;
    };
    let mut sv = Save::open(&k1.save).unwrap();
    let own = sv.back_up(&k1.backups).unwrap().parent().unwrap().to_path_buf();
    k1.edit_and_save(5);
    let foreign = own.join("2026-01-08 00-00-00");
    copy_dir(&k2.save, &foreign);
    k1.refused(&foreign, "a KotOR II save as a KotOR backup");
    k1.done();
    k2.done();
}

/// A file held open with no sharing, so it cannot be renamed: the swap fails part way and
/// every file must be back as it was, with nothing of the restore left in the folder.
#[cfg(windows)]
#[test]
fn a_file_held_open_part_way_puts_everything_back() {
    use std::os::windows::fs::OpenOptionsExt;
    for lab in games("held-open") {
        let g = lab.game;
        let mut sv = Save::open(&lab.save).unwrap();
        let good = sv.back_up(&lab.backups).unwrap();
        lab.edit_and_save(300);
        let edited = files(&lab.save);
        // the files in the order the swap takes them (by name): hold the first, then the last,
        // so it fails before anything moved and after nearly everything moved
        let mut names: Vec<String> = fs::read_dir(&lab.save).unwrap().filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().to_string()).collect();
        names.sort();
        for held in [names.first().unwrap().clone(), names.last().unwrap().clone()] {
            let count = lab.list().len();
            // readable (the backup made first reads it), but it cannot be renamed or removed
            let lock = fs::OpenOptions::new().read(true).share_mode(1).open(lab.save.join(&held)).unwrap();
            let r = save::restore(&lab.save, &good, &lab.backups, g);
            drop(lock);
            let e = r.err().unwrap_or_else(|| panic!("{:?}: held {}: the restore cannot finish", g, held));
            assert!(!e.starts_with(save::HALF_WRITTEN), "{:?}: held {}: every file put back ({})", g, held, e);
            assert_eq!(files(&lab.save), edited, "{:?}: held {}: the save is as it was", g, held);
            assert!(save::interrupted_write(&lab.save).is_none(), "{:?}: held {}: nothing of the restore left", g, held);
            assert_eq!(lab.list().len(), count + 1, "{:?}: held {}: the save was backed up before the swap", g, held);
            assert_eq!(files(lab.list().last().unwrap()), edited);
        }
        // let go, it goes through
        save::restore(&lab.save, &good, &lab.backups, g).unwrap();
        assert!(save::same_files(&lab.save, &good));
        lab.done();
    }
}

/// A backup file held open: it cannot be read, so the restore stops before the save is touched.
#[cfg(windows)]
#[test]
fn a_backup_that_cannot_be_read_stops_before_the_save_is_touched() {
    use std::os::windows::fs::OpenOptionsExt;
    for lab in games("backup-held") {
        let g = lab.game;
        let mut sv = Save::open(&lab.save).unwrap();
        let good = sv.back_up(&lab.backups).unwrap();
        lab.edit_and_save(40);
        let edited = files(&lab.save);
        let some = fs::read_dir(&good).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).find(|p| p.extension().map_or(false, |x| x.eq_ignore_ascii_case("tga") || x.eq_ignore_ascii_case("res"))).unwrap();
        let lock = fs::OpenOptions::new().read(true).share_mode(0).open(&some).unwrap();
        let r = save::restore(&lab.save, &good, &lab.backups, g);
        drop(lock);
        assert!(r.is_err());
        assert!(!r.err().unwrap().starts_with(save::HALF_WRITTEN));
        assert_eq!(files(&lab.save), edited, "{:?}: the save is as it was", g);
        assert!(save::interrupted_write(&lab.save).is_none());
        lab.done();
    }
}

#[test]
fn after_a_restore_the_save_opens_backs_up_and_saves() {
    for lab in games("after") {
        let g = lab.game;
        let mut sv = Save::open(&lab.save).unwrap();
        let good = sv.back_up(&lab.backups).unwrap();
        let xp0 = lab.xp();
        lab.edit_and_save(900);
        save::restore(&lab.save, &good, &lab.backups, g).unwrap();
        // opening finds its backup: the one restored from is the same, but not the newest
        let mut sv = Save::open(&lab.save).unwrap();
        let b = sv.back_up(&lab.backups).unwrap();
        assert!(save::same_files(&lab.save, &b), "{:?}: a good backup on opening", g);
        sv.edit(Who::Player, "xp", |c| creature::set_experience(c, xp0 + 1));
        sv.write(&lab.backups).unwrap();
        assert_eq!(lab.xp(), xp0 + 1, "{:?}: saved as any other", g);
        lab.done();
    }
}

/// Restoring back and forth many times between three versions: each time exactly that version,
/// and the list of backups only ever grows.
#[test]
fn back_and_forth_many_times() {
    for lab in games("many") {
        let g = lab.game;
        let mut sv = Save::open(&lab.save).unwrap();
        let v0 = sv.back_up(&lab.backups).unwrap();
        lab.edit_and_save(11);
        lab.edit_and_save(22);
        let list = lab.list();
        assert!(list.len() >= 2);
        let v1 = list[1].clone();
        let v2_files = files(&lab.save);
        let mut versions = vec![(v0.clone(), files(&v0)), (v1.clone(), files(&v1))];
        let mut last = lab.list().len();
        for round in 0..6 {
            let (target, want) = &versions[round % 2];
            save::restore(&lab.save, target, &lab.backups, g).unwrap();
            assert_eq!(&files(&lab.save), want, "{:?}: round {}", g, round);
            let now = lab.list().len();
            assert_eq!(now, last + 1, "{:?}: round {}: one backup more each time", g, round);
            last = now;
        }
        // and the newest version is still there to go back to
        let newest_of_v2 = lab.list().into_iter().find(|p| files(p) == v2_files).expect("the edited version is kept");
        save::restore(&lab.save, &newest_of_v2, &lab.backups, g).unwrap();
        assert_eq!(files(&lab.save), v2_files);
        versions.clear();
        lab.done();
    }
}

/// Two saves with one folder name share a backups folder (two Steam accounts' cloud saves, a
/// copy kept inside the game folder): each lists and reverts to its own backups only.
#[test]
fn a_save_of_the_same_folder_name_never_gets_the_others_backups() {
    for lab in games("same-name") {
        let g = lab.game;
        let other = lab.root.join("elsewhere").join(lab.save.file_name().unwrap());
        copy_dir(&lab.save, &other);
        // the other one changed, then both backed up into the one backups folder
        let mut o = Save::open(&other).unwrap();
        o.back_up(&lab.backups).unwrap();
        let xp = o.player().map(creature::experience).unwrap_or(0);
        o.edit(Who::Player, "xp", |c| creature::set_experience(c, xp + 4321));
        o.write(&lab.backups).unwrap();
        let mut o = Save::open(&other).unwrap();
        let theirs = o.back_up(&lab.backups).unwrap();
        let mut mine = Save::open(&lab.save).unwrap();
        let own = mine.back_up(&lab.backups).unwrap();
        assert_eq!(theirs.parent(), own.parent(), "{:?}: one backups folder for both", g);
        let list = save::backup_list(&lab.backups, &lab.save);
        assert!(list.contains(&own), "{:?}: its own backup is listed", g);
        assert!(!list.contains(&theirs), "{:?}: the other save's backup is not", g);
        lab.edit_and_save(7);
        lab.refused(&theirs, "the other save's backup of the same folder name");
        save::restore(&lab.save, &own, &lab.backups, g).unwrap();
        lab.done();
    }
}

/// The game folder moved (another drive, another Steam library): the backups made before the
/// move are still the save's own, listed and taken back (second review of 2026-11-07).
#[test]
fn backups_stay_the_saves_own_after_the_game_folder_moves() {
    for lab in games("moved") {
        let g = lab.game;
        let mut sv = Save::open(&lab.save).unwrap();
        let first = sv.back_up(&lab.backups).unwrap();
        lab.edit_and_save(61);
        let moved_root = lab.root.with_file_name(format!("{}-moved", lab.root.file_name().unwrap().to_string_lossy()));
        let _ = fs::remove_dir_all(&moved_root);
        fs::rename(&lab.root, &moved_root).unwrap();
        let save_dir = moved_root.join(lab.save.strip_prefix(&lab.root).unwrap());
        let backups = moved_root.join(lab.backups.strip_prefix(&lab.root).unwrap());
        let moved_first = backups.join(first.strip_prefix(&lab.backups).unwrap());
        let list = save::backup_list(&backups, &save_dir);
        assert!(list.contains(&moved_first), "{:?}: the first backup is still listed after the move: {:?}", g, list);
        save::restore(&save_dir, &moved_first, &backups, g).unwrap();
        assert!(save::same_files(&save_dir, &moved_first));
        let _ = fs::remove_dir_all(&moved_root);
    }
}
