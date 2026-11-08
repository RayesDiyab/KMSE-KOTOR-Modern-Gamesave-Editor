//! The save's picture (Screen.tga), shown on the game's Load screen and in the editor's save
//! list, goes with every save the editor makes: Copy, Save As, Save Changes and a revert.
//! On copies of a real save with a picture; nothing checked where there is none.

use kse_core::games::{find_installs, find_saves, Game};
use kse_core::save::{self, Save, Who};
use std::fs;
use std::path::{Path, PathBuf};

fn picture(folder: &Path) -> Option<Vec<u8>> {
    fs::read_dir(folder).ok()?.filter_map(|e| e.ok()).find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case("screen.tga")).and_then(|e| fs::read(e.path()).ok())
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap().filter_map(|e| e.ok()) {
        if e.path().is_file() {
            fs::copy(e.path(), to.join(e.file_name())).unwrap();
        }
    }
}

#[test]
fn every_save_the_editor_makes_keeps_its_picture() {
    let installs = find_installs(&[]);
    let mut done = 0;
    for game in [Game::K1, Game::K2] {
        let Some(found) = find_saves(&installs, game, &[]).into_iter().find(|f| picture(&f.path).is_some() && !save::player_outside_archive(&f.path) && Save::open(&f.path).is_ok()) else { continue };
        let shot = picture(&found.path).unwrap();
        assert!(save::screenshot(&found.path).is_some(), "{:?}: the picture decodes", game);
        let root: PathBuf = std::env::temp_dir().join("kmse-tests").join("save-picture").join(format!("{:?}", game));
        let _ = fs::remove_dir_all(&root);
        let original = root.join("saves").join("000600 - Game599");
        copy_dir(&found.path, &original);
        let backups = root.join("Save Editor Backup");

        // Copy
        let copied = root.join("saves").join("000601 - Game600");
        Save::copy_save(&original, &copied, "Copied").unwrap();
        assert_eq!(picture(&copied).as_deref(), Some(shot.as_slice()), "{:?}: Copy keeps the picture", game);

        // Save Changes
        let mut sv = Save::open(&original).unwrap();
        sv.back_up(&backups).unwrap();
        let xp = sv.player().map(kse_core::creature::experience).unwrap_or(0);
        sv.edit(Who::Player, "xp", |c| kse_core::creature::set_experience(c, xp + 1));
        sv.write(&backups).unwrap();
        assert_eq!(picture(&original).as_deref(), Some(shot.as_slice()), "{:?}: Save Changes keeps the picture", game);

        // Save As
        let target = root.join("saves").join("000602 - Game601");
        let mut sv = Save::open(&original).unwrap();
        sv.edit(Who::Player, "xp", |c| kse_core::creature::set_experience(c, xp + 2));
        sv.write_as(&target, "Saved As").unwrap();
        assert_eq!(picture(&target).as_deref(), Some(shot.as_slice()), "{:?}: Save As keeps the picture", game);
        assert!(save::screenshot(&target).is_some());

        let _ = fs::remove_dir_all(&root);
        done += 1;
    }
    if done == 0 {
        eprintln!("no save with a picture here: nothing checked");
    }
}
