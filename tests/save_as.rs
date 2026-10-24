//! Save as a New Save (Save Changes' arrow): on a copy of a real save in the system's
//! temporary folder, a change written as a new save lands in the new folder under the new
//! name, and the save it was read from keeps every file byte for byte.

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
        .filter(|e| e.path().is_file())
        .map(|e| (e.file_name().to_string_lossy().to_string(), fs::read(e.path()).unwrap()))
        .collect();
    out.sort();
    out
}

#[test]
fn a_new_save_leaves_the_original_as_it_was() {
    let installs = find_installs(&[]);
    let mut done = 0;
    for game in [Game::K1, Game::K2] {
        let Some(folder) = find_saves(&installs, game, &[]).into_iter().next() else { continue };
        let root: PathBuf = std::env::temp_dir().join("kmse-tests").join("save-as").join(format!("{:?}", game));
        let _ = fs::remove_dir_all(&root);
        let original = root.join("000500 - Game499");
        copy_dir(&folder.path, &original);
        let before = files(&original);

        let mut sv = Save::open(&original).unwrap();
        let xp = sv.player().map(creature::experience).unwrap_or(0);
        sv.edit(Who::Player, "xp", |c| creature::set_experience(c, xp + 1234));
        let target = root.join("000501 - Game500");
        sv.write_as(&target, "KMRP Save As test").unwrap();

        assert_eq!(files(&original), before, "{:?}: the original save is untouched", game);
        let new = Save::open(&target).unwrap();
        assert_eq!(new.player().map(creature::experience), Some(xp + 1234), "{:?}: the change is in the new save", game);
        assert_eq!(save::summary(&target).map(|s| s.name), Some("KMRP Save As test".to_string()), "{:?}: the new save's name", game);
        assert!(sv.changes.is_empty(), "{:?}: nothing is left unsaved", game);
        assert!(sv.write_as(&target, "again").is_err(), "{:?}: an existing folder is never written over", game);
        done += 1;
    }
    eprintln!("{} games checked", done);
}
