//! The backup made on opening (2026-11-07): on a copy of a real save in the system's temporary
//! folder, opening backs the save up byte for byte, opening it again unchanged keeps that one
//! backup, saving uses it after checking it, a damaged backup is replaced by a new one before
//! anything is written, and a second save in the same session backs up the first save's files.

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

#[test]
fn the_backup_is_made_on_opening_and_checked_on_saving() {
    let installs = find_installs(&[]);
    let mut done = 0;
    for game in [Game::K1, Game::K2] {
        let Some(folder) = find_saves(&installs, game, &[]).into_iter().filter(|f| !save::player_outside_archive(&f.path) && kse_core::save::Save::open(&f.path).is_ok()).next() else { continue };
        let root: PathBuf = std::env::temp_dir().join("kmse-tests").join("backup-on-open").join(format!("{:?}", game));
        let _ = fs::remove_dir_all(&root);
        let original = root.join("saves").join("000500 - Game499");
        copy_dir(&folder.path, &original);
        let backups = root.join("Save Editor Backup");

        // opening backs it up, the same bytes
        let mut sv = Save::open(&original).unwrap();
        let first = sv.back_up(&backups).unwrap();
        assert!(save::same_files(&original, &first), "{:?}: the backup holds the save as it is", game);
        assert_eq!(save::backup_list(&backups, &original), vec![first.clone()]);

        // opened again unchanged: the same backup, no second copy
        let mut again = Save::open(&original).unwrap();
        assert_eq!(again.back_up(&backups).unwrap(), first, "{:?}: an unchanged save keeps its backup", game);
        assert_eq!(save::backup_list(&backups, &original).len(), 1);

        // saving checks the backup and uses it
        let xp = sv.player().map(creature::experience).unwrap_or(0);
        sv.edit(Who::Player, "xp", |c| creature::set_experience(c, xp + 10));
        assert_eq!(sv.write(&backups).unwrap(), first, "{:?}: a good backup is used, not copied again", game);
        assert_eq!(save::backup_list(&backups, &original).len(), 1);
        assert!(!save::same_files(&original, &first), "{:?}: the save was written", game);

        // a second save in the session backs up the first save's files before writing
        let mut sv = Save::open(&original).unwrap();
        let written_once = root.join("written-once");
        copy_dir(&original, &written_once);
        sv.edit(Who::Player, "xp", |c| creature::set_experience(c, xp + 20));
        let second = sv.write(&backups).unwrap();
        assert_ne!(second, first);
        assert!(save::same_files(&written_once, &second), "{:?}: the new backup is the save as the first write left it", game);

        // a damaged backup fails the check: a new one is made before anything is written
        let mut sv = Save::open(&original).unwrap();
        let checked = sv.back_up(&backups).unwrap();
        let before_damage = root.join("before-damage");
        copy_dir(&original, &before_damage);
        let victim = fs::read_dir(&checked).unwrap().filter_map(|e| e.ok()).find(|e| e.path().is_file()).unwrap().path();
        fs::write(&victim, b"damaged").unwrap();
        sv.edit(Who::Player, "xp", |c| creature::set_experience(c, xp + 30));
        let fresh = sv.write(&backups).unwrap();
        assert_ne!(fresh, checked, "{:?}: the damaged backup is not trusted", game);
        assert!(save::same_files(&before_damage, &fresh), "{:?}: the new backup is the save before this write", game);

        // oldest first
        let list = save::backup_list(&backups, &original);
        let mut sorted = list.clone();
        sorted.sort();
        assert_eq!(list, sorted);
        assert!(list.len() >= 3);

        let _ = fs::remove_dir_all(&root);
        done += 1;
    }
    if done == 0 {
        eprintln!("no save to copy: skipped");
    }
}
