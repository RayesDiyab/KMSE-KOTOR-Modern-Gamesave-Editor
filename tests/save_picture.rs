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

use kse_core::formats::tga;
use kse_core::gamedata::GameData;
use kse_core::picture;

/// The picture is written as the game writes Screen.tga: the same 18 header bytes as a real
/// one, 196,626 bytes, read back as written.
#[test]
fn a_made_picture_is_laid_out_as_the_games_own() {
    let img = tga::Image { width: 2, height: 2, rgba: vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 10, 20, 30, 0] };
    let small = picture::screen_tga(&img);
    let back = tga::decode(&small).unwrap();
    assert_eq!((back.width, back.height), (2, 2));
    assert_eq!(&back.rgba[..12], &img.rgba[..12], "top row first after reading, colours kept");
    assert_eq!(&back.rgba[12..16], &[0, 0, 0, 255], "a clear pixel goes onto black");
    let big = picture::screen_tga(&picture::resize(&img, 256, 256));
    assert_eq!(big.len(), 196_626);
    // the header of a Screen.tga the game wrote, where there is one here
    let installs = find_installs(&[]);
    for game in [Game::K1, Game::K2] {
        if let Some(real) = find_saves(&installs, game, &[]).into_iter().find_map(|f| picture(&f.path)) {
            assert_eq!(&big[..18], &real[..18], "{:?}: the game's own header", game);
            assert_eq!(big.len(), real.len(), "{:?}", game);
        }
    }
}

#[test]
fn a_box_filter_averages() {
    let img = tga::Image { width: 2, height: 1, rgba: vec![0, 0, 0, 255, 200, 100, 50, 255] };
    assert_eq!(picture::resize(&img, 1, 1).rgba, vec![100, 50, 25, 255]);
    let up = picture::resize(&img, 4, 2);
    assert_eq!((up.width, up.height, up.rgba.len()), (4, 2, 32));
}

/// Each game's load screen for a module: its own (`load_<module>`), else load_default; a name
/// that is not a module is refused.
#[test]
fn load_screens_come_from_the_game() {
    let installs = find_installs(&[]);
    let mut done = 0;
    for (game, module) in [(Game::K1, "end_m01aa"), (Game::K2, "203tel")] {
        let Some(install) = installs.iter().find(|i| i.game == game) else { continue };
        let data = GameData::open(game, &install.path);
        let own = picture::load_screen(&data, module).expect("the module's load screen");
        let default = picture::load_screen(&data, "zz_none").expect("load_default");
        assert_ne!(own.rgba, default.rgba, "{:?}: {} has its own", game, module);
        let bytes = picture::for_module(&data, module).unwrap();
        let shown = tga::decode(&bytes).unwrap();
        assert_eq!((shown.width, shown.height), (256, 256));
        let mean = shown.rgba.chunks(4).map(|p| p[0] as u64 + p[1] as u64 + p[2] as u64).sum::<u64>() / (256 * 256 * 3);
        assert!(mean > 10, "{:?}: not a black picture (mean {})", game, mean);
        assert!(picture::load_screen(&data, "../x").is_none());
        done += 1;
    }
    if done == 0 {
        eprintln!("no game installed here: nothing checked");
    }
}

/// A save without a picture gets the one given when it is written: Save Changes, Save As and
/// Copy. A save with one keeps its own. A write that is refused adds nothing, and a revert to
/// a backup without one takes it away again. Copies only.
#[test]
fn a_save_without_a_picture_gets_one_when_written() {
    let installs = find_installs(&[]);
    let mut done = 0;
    for game in [Game::K1, Game::K2] {
        let Some(install) = installs.iter().find(|i| i.game == game) else { continue };
        let Some(found) = find_saves(&installs, game, &[]).into_iter().find(|f| !save::player_outside_archive(&f.path) && Save::open(&f.path).is_ok()) else { continue };
        let data = GameData::open(game, &install.path);
        let root: PathBuf = std::env::temp_dir().join("kmse-tests").join("save-picture-added").join(format!("{:?}", game));
        let _ = fs::remove_dir_all(&root);
        let original = root.join("saves").join("000700 - Game699");
        copy_dir(&found.path, &original);
        for e in fs::read_dir(&original).unwrap().filter_map(|e| e.ok()) {
            if e.file_name().to_string_lossy().eq_ignore_ascii_case("screen.tga") {
                fs::remove_file(e.path()).unwrap();
            }
        }
        let backups = root.join("Save Editor Backup");
        let mut sv = Save::open(&original).unwrap();
        let module = sv.queued_trip().unwrap_or_else(|| sv.module_name().to_string());
        let made = picture::for_module(&data, &module).expect("a load screen for the save's module");
        let first = sv.back_up(&backups).unwrap();

        // a refused write adds nothing
        sv.give_picture_if_missing(made.clone());
        let xp = sv.player().map(kse_core::creature::experience).unwrap_or(0);
        sv.edit(Who::Player, "xp", |c| kse_core::creature::set_experience(c, xp + 1));
        fs::write(original.join("extra.txt"), b"the game wrote").unwrap();
        assert!(sv.write(&backups).is_err());
        assert!(picture(&original).is_none(), "{:?}: no picture from a refused write", game);
        fs::remove_file(original.join("extra.txt")).unwrap();

        // Save Changes
        let mut sv = Save::open(&original).unwrap();
        sv.give_picture_if_missing(made.clone());
        sv.edit(Who::Player, "xp", |c| kse_core::creature::set_experience(c, xp + 2));
        sv.write(&backups).unwrap();
        assert_eq!(picture(&original).as_deref(), Some(made.as_slice()), "{:?}: Save Changes gives the picture", game);
        assert!(save::screenshot(&original).is_some());
        // the save, picture and all, writes again without "changed on disk"
        sv.edit(Who::Player, "xp", |c| kse_core::creature::set_experience(c, xp + 3));
        sv.write(&backups).unwrap();
        assert!(save::interrupted_write(&original).is_none());

        // a save with a picture keeps its own
        let mut sv = Save::open(&original).unwrap();
        let other = picture::screen_tga(&picture::resize(&tga::Image { width: 1, height: 1, rgba: vec![9, 9, 9, 255] }, 256, 256));
        sv.give_picture_if_missing(other.clone());
        sv.edit(Who::Player, "xp", |c| kse_core::creature::set_experience(c, xp + 4));
        sv.write(&backups).unwrap();
        assert_eq!(picture(&original).as_deref(), Some(made.as_slice()), "{:?}: its own picture stays", game);
        // a picture that is not 256 by 256 is not taken
        let mut sv = Save::open(&original).unwrap();
        sv.give_picture_if_missing(b"not a picture".to_vec());

        // revert to the backup without a picture: it goes again
        save::restore(&original, &first, &backups, game).unwrap();
        assert!(picture(&original).is_none(), "{:?}: the revert takes the picture away", game);

        // Copy and Save As of a save without one
        let copied = root.join("saves").join("000701 - Game700");
        Save::copy_save_with_picture(&original, &copied, "Copied", Some(made.clone())).unwrap();
        assert_eq!(picture(&copied).as_deref(), Some(made.as_slice()), "{:?}: Copy gives the picture", game);
        let target = root.join("saves").join("000702 - Game701");
        let mut sv = Save::open(&original).unwrap();
        sv.give_picture_if_missing(made.clone());
        sv.write_as(&target, "Saved As").unwrap();
        assert_eq!(picture(&target).as_deref(), Some(made.as_slice()), "{:?}: Save As gives the picture", game);
        assert!(picture(&original).is_none(), "{:?}: Save As leaves the original as it was", game);

        let _ = fs::remove_dir_all(&root);
        done += 1;
    }
    if done == 0 {
        eprintln!("no game and save here: nothing checked");
    }
}
