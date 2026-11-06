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
        let Some(folder) = find_saves(&installs, game, &[]).into_iter().filter(|f| !kse_core::save::player_outside_archive(&f.path)).next() else { continue };
        let root: PathBuf = std::env::temp_dir().join("kmse-tests").join("save-as").join(format!("{:?}", game));
        let _ = fs::remove_dir_all(&root);
        let original = root.join("000500 - Game499");
        copy_dir(&folder.path, &original);
        let before = files(&original);

        let mut sv = Save::open(&original).unwrap();
        let xp = sv.player().map(creature::experience).unwrap_or(0);
        sv.edit(Who::Player, "xp", |c| creature::set_experience(c, xp + 1234));
        let target = save::next_save_folder(&original).unwrap();
        assert_eq!(target, root.join("000501 - Game500"));
        sv.write_as(&target, "KMRP Save As test").unwrap();

        assert_eq!(files(&original), before, "{:?}: the original save is untouched", game);
        let new = Save::open(&target).unwrap();
        assert_eq!(new.player().map(creature::experience), Some(xp + 1234), "{:?}: the change is in the new save", game);
        assert_eq!(save::summary(&target).map(|s| s.name), Some("KMRP Save As test".to_string()), "{:?}: the new save's name", game);
        assert!(sv.changes.is_empty(), "{:?}: nothing is left unsaved", game);
        assert!(sv.write_as(&target, "again").is_err(), "{:?}: an existing folder is never written over", game);
        let copy_target = save::next_save_folder(&target).unwrap();
        assert_eq!(copy_target, root.join("000502 - Game501"));
        let source_before_copy = files(&target);
        Save::copy_save(&target, &copy_target, "KMRP Copy test").unwrap();
        assert_eq!(save::summary(&copy_target).unwrap().name, "KMRP Copy test");
        assert_eq!(Save::open(&copy_target).unwrap().player().map(creature::experience), Some(xp + 1234));
        assert_eq!(files(&target), source_before_copy);
        done += 1;
    }
    eprintln!("{} games checked", done);
}

/// A new save's folder follows the game's own pattern, because the game overwrites a save by
/// writing to "<number> - Game<number - 1>"; any other folder name made the overwrite a second
/// save (2026-10-29: "000920 - KMRPPazaak" beside "000920 - Game919").
#[test]
fn a_new_save_folder_is_named_as_the_game_names_it() {
    let root = std::env::temp_dir().join(format!("kmse-next-folder-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    for name in ["000000 - QUICKSAVE", "000001 - AUTOSAVE", "000023 - Game22", "000920 - KMRPPazaak"] {
        fs::create_dir_all(root.join(name)).unwrap();
    }
    fs::write(root.join("999999 - not a save folder"), b"ignored").unwrap();
    let next = save::next_save_folder(&root.join("000023 - Game22"));
    assert_eq!(next, Some(root.join("000921 - Game920")));
    fs::create_dir_all(root.join("4294967295 - limit")).unwrap();
    assert_eq!(save::next_save_folder(&root.join("000023 - Game22")), None);
    fs::remove_dir_all(&root).unwrap();
}

/// KotOR 1's autosave on first arriving in an area keeps the player in pifo.ifo: on a copy of
/// such a save, it opens with its player, and Save Changes and Save as a New Save both refuse
/// it and leave every file as it was (2026-10-29).
#[test]
fn a_save_with_its_player_in_pifo_opens_read_only() {
    let installs = find_installs(&[]);
    let mut done = 0;
    for game in [Game::K1, Game::K2] {
        let Some(folder) = find_saves(&installs, game, &[]).into_iter().find(|f| save::player_outside_archive(&f.path)) else { continue };
        let root: PathBuf = std::env::temp_dir().join("kmse-tests").join("pifo").join(format!("{:?}", game));
        let _ = fs::remove_dir_all(&root);
        let original = root.join("000001 - AUTOSAVE");
        copy_dir(&folder.path, &original);
        let before = files(&original);

        let mut sv = Save::open(&original).unwrap();
        assert!(sv.player().is_some(), "{:?}: the save opens with its player", game);
        sv.rename("KMRP pifo test");
        assert!(sv.write(&root.join("backups")).is_err(), "{:?}: Save Changes is refused", game);
        assert!(sv.write_as(&root.join("000002 - Game1"), "again").is_err(), "{:?}: Save as a New Save is refused", game);
        assert!(!root.join("000002 - Game1").exists() && !root.join("backups").exists(), "{:?}: nothing new was made", game);
        assert_eq!(files(&original), before, "{:?}: the save is untouched", game);
        let copied = root.join("000002 - Game1");
        Save::copy_save(&original, &copied, "Copied read-only save").unwrap();
        assert!(save::player_outside_archive(&copied));
        assert_eq!(save::summary(&copied).unwrap().name, "Copied read-only save");
        for (name, bytes) in &before {
            if !name.eq_ignore_ascii_case("savenfo.res") {
                assert_eq!(fs::read(copied.join(name)).unwrap(), *bytes, "{:?}: Copy preserves {}", game, name);
            }
        }
        assert_eq!(files(&original), before);
        eprintln!("{:?}: read-only pifo save {}", game, folder.path.display());
        done += 1;
    }
    eprintln!("{} saves with the player in pifo.ifo checked", done);
}
