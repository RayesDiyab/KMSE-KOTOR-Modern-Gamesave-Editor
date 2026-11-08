//! The macOS port (2026-11-08): finding the game inside its app bundle, telling the games
//! apart without their Windows programs, seeing the macOS ports run, and the save checks
//! against what Finder leaves in a folder. Every write is to a copy in the system's
//! temporary folder. The tests that need a real save or a real install say so and pass
//! without checking when there is none; those that start programs run on macOS only.

use kse_core::games::{self, Game};
use kse_core::save::{self, Save, Who};
use std::fs;
use std::path::{Path, PathBuf};

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("kmse-tests").join("macos").join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    games::clean(&dir)
}

/// A folder of game files as the editor checks them: a KEY and a TLK by their first bytes,
/// and a modules folder with the game's first module.
fn game_files(dir: &Path, key_name: &str, first_module: &str) {
    fs::create_dir_all(dir.join("modules")).unwrap();
    fs::write(dir.join(key_name), b"KEY V1  \0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0").unwrap();
    fs::write(dir.join("dialog.tlk"), b"TLK V3.0\0\0\0\0").unwrap();
    fs::write(dir.join("modules").join(format!("{}.rim", first_module)), b"RIM V1.0").unwrap();
}

fn bundle(at: &Path, id: &str, data: &str) -> PathBuf {
    fs::create_dir_all(at.join("Contents/MacOS")).unwrap();
    fs::write(
        at.join("Contents/Info.plist"),
        format!("<?xml version=\"1.0\"?><plist><dict><key>CFBundleIdentifier</key><string>{}</string></dict></plist>", id),
    )
    .unwrap();
    let files = at.join("Contents").join(data);
    files
}

#[test]
fn an_app_bundle_stands_for_the_game_files_inside_it() {
    let dir = temp("bundle");
    // KotOR as the Mac App Store and Steam have it: Contents/Assets
    let k1 = dir.join("Knights of the Old Republic.app");
    let k1_files = bundle(&k1, "com.aspyr.kotor", "Assets");
    game_files(&k1_files, "chitin.key", "end_m01aa");
    assert_eq!(games::resource_root(&k1), Some(k1_files.clone()));
    assert_eq!(games::identify(&k1_files), Some(Game::K1));
    // the folder Steam keeps it in
    assert_eq!(games::resource_root(&dir), Some(k1_files.clone()));
    // the game files themselves
    assert_eq!(games::resource_root(&k1_files), Some(k1_files.clone()));

    // KotOR II: Contents/GameData, a small chitin.key (the old size rule took it for KotOR)
    let k2 = dir.join("KOTOR2.app");
    let k2_files = bundle(&k2, "com.aspyr.kotor2.appstore", "GameData");
    game_files(&k2_files, "chitin.key", "001ebo");
    assert_eq!(games::resource_root(&k2), Some(k2_files.clone()));
    assert_eq!(games::identify(&k2_files), Some(Game::K2));
    assert_eq!(games::bundle_game(&k2), Some(Game::K2));

    // picked through find_installs as an added place, the bundle is the install
    let found = games::find_installs(&[k2.clone()]);
    assert!(found.iter().any(|i| i.game == Game::K2 && i.path == games::clean(&k2_files)), "{:?}", found);
}

#[test]
fn the_game_is_told_by_its_files_without_a_bundle() {
    let dir = temp("loose");
    let k1 = dir.join("k1");
    game_files(&k1, "chitin.key", "end_m01aa");
    assert_eq!(games::identify(&k1), Some(Game::K1));
    let k2 = dir.join("k2");
    game_files(&k2, "chitin.key", "001ebo");
    assert_eq!(games::identify(&k2), Some(Game::K2));
    // neither module: the editor does not guess
    let unknown = dir.join("unknown");
    game_files(&unknown, "chitin.key", "something");
    assert_eq!(games::identify(&unknown), None);
}

#[test]
fn chitin_key_is_found_in_any_case() {
    let dir = temp("case");
    game_files(&dir, "CHITIN.KEY", "end_m01aa");
    assert!(games::has_game_files(&dir));
    assert_eq!(games::identify(&dir), Some(Game::K1));
}

#[test]
fn a_folder_with_the_names_but_not_the_files_is_refused() {
    let dir = temp("hollow");
    fs::create_dir_all(dir.join("modules")).unwrap();
    fs::write(dir.join("chitin.key"), b"").unwrap();
    fs::write(dir.join("dialog.tlk"), b"TLK V3.0").unwrap();
    assert!(!games::has_game_files(&dir), "an empty chitin.key is not a KEY");
    assert_eq!(games::resource_root(&dir), None);
    let no_modules = temp("no-modules");
    fs::write(no_modules.join("chitin.key"), b"KEY V1  ").unwrap();
    fs::write(no_modules.join("dialog.tlk"), b"TLK V3.0").unwrap();
    assert!(!games::has_game_files(&no_modules), "no modules folder");
    // a bundle that is not a game
    let other = dir.join("Other.app");
    bundle(&other, "com.example.other", "Resources");
    assert_eq!(games::resource_root(&other), None);
    assert_eq!(games::bundle_game(&other), None);
}

#[test]
fn backups_never_go_inside_an_app_bundle() {
    let dir = temp("backup-root");
    let files = bundle(&dir.join("KOTOR2.app"), "com.aspyr.kotor2", "GameData");
    let save = files.join("saves").join("000001 - QuickSave");
    fs::create_dir_all(&save).unwrap();
    let root = save::backup_root(&save, Some(&files));
    assert!(!root.ancestors().any(|a| a.extension().map_or(false, |x| x == "app")), "{}", root.display());
    assert_eq!(root, dir.join("Save Editor Backup"), "beside the bundle");
    // outside a bundle a save in the game folder keeps them in the game folder, as before
    let plain = dir.join("game");
    fs::create_dir_all(plain.join("saves").join("000001 - QuickSave")).unwrap();
    assert_eq!(save::backup_root(&plain.join("saves").join("000001 - QuickSave"), Some(&plain)), plain.join("Save Editor Backup"));
}

#[test]
fn the_ports_programs_are_told_apart() {
    let none = |_: &Path| None;
    let k1_bundle = |_: &Path| Some(Game::K1);
    let k2_bundle = |_: &Path| Some(Game::K2);
    // the Windows programs, under Wine or CrossOver
    assert_eq!(games::game_of_process("", "swkotor.exe", &none), Some(Game::K1));
    assert_eq!(games::game_of_process("C:/x/swkotor2.exe", "", &none), Some(Game::K2));
    // Aspyr's KotOR: the launcher and the game, told by the bundle
    assert_eq!(games::game_of_process("/Applications/Knights of the Old Republic.app/Contents/MacOS/AppBundleExe", "AppBundleExe", &k1_bundle), Some(Game::K1));
    assert_eq!(games::game_of_process("/x/Knights of the Old Republic.app/Contents/MacOS/KOTOR_Exe", "KOTOR_Exe", &none), Some(Game::K1));
    // KotOR II and its helper
    assert_eq!(games::game_of_process("/Applications/KOTOR2.app/Contents/MacOS/KOTOR2", "KOTOR2", &none), Some(Game::K2));
    assert_eq!(games::game_of_process("/Applications/KOTOR2.app/Contents/MacOS/KOTOR2sub", "KOTOR2sub", &k2_bundle), Some(Game::K2));
    // AppBundleExe is every Aspyr port's launcher: another game's is not KotOR
    assert_eq!(games::game_of_process("/Applications/Jade Empire.app/Contents/MacOS/AppBundleExe", "AppBundleExe", &none), None);
    // the editor, Steam, a shell
    assert_eq!(games::game_of_process("/Applications/KMSE.app/Contents/MacOS/KMSE", "KMSE", &none), None);
    assert_eq!(games::game_of_process("/bin/zsh", "zsh", &none), None);
}

/// A program running from a made-up bundle, as the game would: a copy of `sleep` named as
/// the game's program. Ended when dropped.
#[cfg(target_os = "macos")]
struct Decoy(std::process::Child);

#[cfg(target_os = "macos")]
impl Drop for Decoy {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// A copy of a system program keeps Apple's signature, which no longer fits it once renamed
/// and moved: macOS ends it at once ("Code Signature Invalid") and the test would see only a
/// dead process. Signed ad hoc, it runs.
#[cfg(target_os = "macos")]
fn ad_hoc_sign(program: &Path) {
    let ok = std::process::Command::new("/usr/bin/codesign").args(["--force", "--sign", "-"]).arg(program).output().map_or(false, |o| o.status.success());
    assert!(ok, "{} could not be signed", program.display());
}

/// Whether process `pid` is alive and running (not ended and waiting to be reaped).
#[cfg(target_os = "macos")]
fn alive(pid: u32) -> bool {
    std::process::Command::new("/bin/ps").args(["-o", "stat=", "-p", &pid.to_string()]).output().map_or(false, |o| {
        let stat = String::from_utf8_lossy(&o.stdout).trim().to_string();
        !stat.is_empty() && !stat.starts_with('Z')
    })
}

#[cfg(target_os = "macos")]
fn decoy(program: &Path) -> Decoy {
    fs::create_dir_all(program.parent().unwrap()).unwrap();
    fs::copy("/bin/sleep", program).unwrap();
    ad_hoc_sign(program);
    let child = std::process::Command::new(program).arg("30").spawn().unwrap();
    // let it start
    for _ in 0..50 {
        if games::running_games().map_or(false, |g| !g.is_empty()) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(alive(child.id()), "the stand-in for the game runs");
    Decoy(child)
}

/// The programs running are listed on macOS, and a game started from its bundle is seen
/// while it runs and not after. One test, so no other decoy runs beside it.
#[cfg(target_os = "macos")]
#[test]
fn a_running_port_is_seen() {
    let before = games::running_games().expect("the programs running could be listed");
    if !before.is_empty() {
        return eprintln!("a KotOR is running on this Mac ({:?}): not checked", before);
    }
    assert!(!games::game_running(Game::K1) && !games::game_running(Game::K2));
    let dir = temp("decoys");
    {
        let k2 = dir.join("KOTOR2.app");
        bundle(&k2, "com.aspyr.kotor2.appstore", "GameData");
        let _running = decoy(&k2.join("Contents/MacOS/KOTOR2"));
        assert!(games::game_running(Game::K2), "KotOR II from its bundle");
        assert!(!games::game_running(Game::K1), "KotOR II is not KotOR");
    }
    {
        // the launcher's generic name, told by its bundle's identifier
        let k1 = dir.join("Knights of the Old Republic.app");
        bundle(&k1, "com.aspyr.kotor.steam", "Assets");
        let _running = decoy(&k1.join("Contents/MacOS/AppBundleExe"));
        assert!(games::game_running(Game::K1), "KotOR's launcher from its bundle");
        assert!(!games::game_running(Game::K2));
    }
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert!(!games::game_running(Game::K1) && !games::game_running(Game::K2), "ended, no longer seen");
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap().filter_map(|e| e.ok()) {
        if e.path().is_file() {
            fs::copy(e.path(), to.join(e.file_name())).unwrap();
        }
    }
}

/// A real save of each game found here, copied into `dest` (None: no such save here).
fn copied_save(game: Game, dest: &Path) -> Option<PathBuf> {
    let installs = games::find_installs(&[]);
    let folder = games::find_saves(&installs, game, &[])
        .into_iter()
        .filter(|f| !save::player_outside_archive(&f.path) && kse_core::save::Save::open(&f.path).is_ok())
        .find(|f| Save::open(&f.path).is_ok())?;
    copy_dir(&folder.path, dest);
    Some(dest.to_path_buf())
}

/// Finder's .DS_Store and AppleDouble files in a save folder, made after it was opened, do not
/// stop the write ("changed on disk"), are not copied into the backup, and are left alone by a
/// revert. Before the port they refused the write.
#[test]
fn what_finder_leaves_in_a_save_folder_is_not_the_save() {
    assert!(save::is_system_litter(".DS_Store") && save::is_system_litter("._SAVEGAME.sav"));
    assert!(!save::is_system_litter("SAVEGAME.sav") && !save::is_system_litter("savenfo.res"));
    let mut done = 0;
    for game in [Game::K1, Game::K2] {
        let root = temp(&format!("litter-{:?}", game));
        let Some(folder) = copied_save(game, &root.join("saves").join("000500 - Game499")) else { continue };
        let backups = root.join("Save Editor Backup");
        let mut sv = Save::open(&folder).unwrap();
        let first = sv.back_up(&backups).unwrap();
        // Finder looks at the folder after it was opened
        fs::write(folder.join(".DS_Store"), b"Bud1 layout").unwrap();
        fs::write(folder.join("._SAVEGAME.sav"), b"\0\x05\x16\x07").unwrap();
        let xp = sv.player().map(kse_core::creature::experience).unwrap_or(0);
        sv.edit(Who::Player, "xp", |c| kse_core::creature::set_experience(c, xp + 7));
        let used = sv.write(&backups).unwrap_or_else(|e| panic!("{:?}: the write went through: {}", game, e));
        assert_eq!(used, first, "{:?}: the backup made on opening still checks", game);
        assert!(!first.join(".DS_Store").exists(), "{:?}: no .DS_Store copied into a backup", game);
        let again = Save::open(&folder).unwrap();
        assert_eq!(again.player().map(kse_core::creature::experience), Some(xp + 7), "{:?}", game);
        // a revert to the first backup puts the save back and leaves Finder's files be
        let restored = save::restore(&folder, &first, &backups, game).unwrap_or_else(|e| panic!("{:?}: revert: {}", game, e));
        assert!(save::same_files(&folder, &first), "{:?}", game);
        assert!(folder.join(".DS_Store").exists(), "{:?}: the revert did not take Finder's file away", game);
        assert!(restored.before.is_dir());
        let _ = fs::remove_dir_all(&root);
        done += 1;
    }
    if done == 0 {
        eprintln!("no saves here: only the names were checked");
    }
}

/// A save in a folder whose names have spaces, accents, other scripts and an emoji opens,
/// backs up, writes, reverts and opens again (copies only).
#[test]
fn unicode_paths_round_trip() {
    let mut done = 0;
    for game in [Game::K1, Game::K2] {
        let root = temp(&format!("unicode-{:?}", game)).join("Sävé Földer ディレクトリ 💾");
        let Some(folder) = copied_save(game, &root.join("saves").join("000501 - Spiel Ünïcode")) else { continue };
        let backups = save::backup_root(&folder, None);
        assert_eq!(backups, root.join("Save Editor Backup"));
        let mut sv = Save::open(&folder).unwrap();
        let first = sv.back_up(&backups).unwrap();
        let xp = sv.player().map(kse_core::creature::experience).unwrap_or(0);
        sv.edit(Who::Player, "xp", |c| kse_core::creature::set_experience(c, xp + 3));
        sv.write(&backups).unwrap();
        assert_eq!(Save::open(&folder).unwrap().player().map(kse_core::creature::experience), Some(xp + 3));
        assert_eq!(save::backup_list(&backups, &folder), vec![first.clone()]);
        save::restore(&folder, &first, &backups, game).unwrap();
        assert_eq!(Save::open(&folder).unwrap().player().map(kse_core::creature::experience), Some(xp));
        // Save As into another Unicode folder
        let target = root.join("saves").join("000502 - Kopie «Ω»");
        let mut sv = Save::open(&folder).unwrap();
        sv.write_as(&target, "Kopie «Ω»").unwrap();
        assert!(Save::open(&target).is_ok());
        let _ = fs::remove_dir_all(root.parent().unwrap());
        done += 1;
    }
    if done == 0 {
        eprintln!("no saves here: nothing checked");
    }
}

/// A symbolic link inside a save folder is never followed by a backup, on any platform.
#[cfg(unix)]
#[test]
fn a_link_in_a_save_folder_stops_the_backup() {
    let root = temp("link");
    let Some(folder) = copied_save(Game::K1, &root.join("saves").join("000503 - Linked")).or_else(|| copied_save(Game::K2, &root.join("saves").join("000503 - Linked"))) else {
        return eprintln!("no saves here: nothing checked");
    };
    let mut sv = Save::open(&folder).unwrap();
    std::os::unix::fs::symlink("/etc", folder.join("elsewhere")).unwrap();
    let err = sv.back_up(&root.join("Save Editor Backup")).unwrap_err();
    assert!(err.contains("link"), "{}", err);
    let _ = fs::remove_dir_all(&root);
}

/// The settings folder is the user's Application Support on macOS, never the program's own
/// folder or the working directory.
#[cfg(target_os = "macos")]
#[test]
fn settings_live_in_application_support() {
    let dir = games::data_dir();
    let home = PathBuf::from(std::env::var_os("HOME").unwrap());
    assert!(dir.starts_with(home.join("Library/Application Support")), "{}", dir.display());
    assert_eq!(dir.file_name().unwrap(), "KOTOR Modern Gamesave Editor");
}

/// The installs on this Mac, when it has them: each found is a folder of game files of the
/// game it is listed as, and the ones known to be here are found.
#[cfg(target_os = "macos")]
#[test]
fn the_installs_on_this_mac_are_found() {
    let installs = games::find_installs(&[]);
    for i in &installs {
        assert!(games::has_game_files(&i.path), "{:?}", i);
        assert_eq!(games::identify(&i.path), Some(i.game), "{:?}", i);
        assert!(!i.path.to_string_lossy().contains("Save Editor Backup"));
        println!("{:?} {}: {}", i.game, i.source, i.path.display());
    }
    let expect = [
        ("/Applications/Knights of the Old Republic.app/Contents/Assets", Game::K1),
        ("/Applications/KOTOR2.app/Contents/GameData", Game::K2),
    ];
    for (path, game) in expect {
        if games::has_game_files(Path::new(path)) {
            let path = games::clean(Path::new(path));
            assert!(installs.iter().any(|i| i.path == path && i.game == game), "{} not found: {:?}", path.display(), installs);
        }
    }
    if let Some(h) = std::env::var_os("HOME") {
        let steam = PathBuf::from(h).join("Library/Application Support/Steam/steamapps/common/swkotor/Knights of the Old Republic.app/Contents/Assets");
        if steam.join("chitin.key").is_file() {
            let steam = games::clean(&steam);
            assert!(installs.iter().any(|i| i.path == steam && i.game == Game::K1 && i.source == "Steam"), "Steam's KotOR not found: {:?}", installs);
        }
    }
}

/// Under Wine, CrossOver or Whisky the game's process is the Wine loader: the game is told
/// by the program in its arguments (review of the port, P1).
#[test]
fn a_windows_game_under_wine_is_told_by_its_arguments() {
    let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(games::game_in_args(&args(&["wine64-preloader", r"C:\GOG Games\Star Wars - KotOR2\swkotor2.exe"])), Some(Game::K2));
    assert_eq!(games::game_in_args(&args(&["wine", "/Users/x/drive_c/swkotor/SWKOTOR.EXE"])), Some(Game::K1));
    assert_eq!(games::game_in_args(&args(&["wine", "C:/Games/other.exe", "swkotor.exe.bak"])), None);
    assert!(games::is_wine("/Applications/CrossOver.app/Contents/SharedSupport/CrossOver/lib/wine/x86_64-unix/wine64-preloader", "wine64-preloader"));
    assert!(games::is_wine("", "swkotor.exe"));
    assert!(!games::is_wine("/bin/zsh", "zsh"));
}

/// A Wine loader running KotOR II's Windows program is seen; one running something else is
/// not. The loader is a copy of bash named `wine` (its arguments are what count).
#[cfg(target_os = "macos")]
#[test]
fn a_running_wine_game_is_seen() {
    if games::running_games().map_or(true, |g| !g.is_empty()) {
        return eprintln!("a KotOR is running on this Mac: not checked");
    }
    let dir = temp("wine");
    let wine = dir.join("wine");
    fs::copy("/bin/bash", &wine).unwrap();
    ad_hoc_sign(&wine);
    let start = |exe: &str| {
        // "; true": bash would otherwise become sleep itself
        std::process::Command::new(&wine).args(["-c", "sleep 30; true", exe]).spawn().unwrap()
    };
    let wait_for = |want: bool| {
        for _ in 0..100 {
            if games::game_running(Game::K2) == want {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
        false
    };
    let mut other = start(r"C:\Games\other.exe");
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(alive(other.id()), "the stand-in for Wine runs");
    let other_seen = games::game_running(Game::K2) || games::game_running(Game::K1);
    let _ = other.kill();
    let _ = other.wait();
    assert!(!other_seen, "another Windows program is not a game");
    let mut k2 = start(r"C:\Games\KotOR2\swkotor2.exe");
    let seen = wait_for(true);
    assert!(alive(k2.id()), "seen while it runs");
    let _ = k2.kill();
    let _ = k2.wait();
    assert!(seen, "KotOR II under Wine is seen");
    assert!(wait_for(false), "and no longer once it ended");
}

/// Steam's KotOR II on the Mac keeps its cloudsaves beside KOTOR2.app, not in the game files
/// inside it (review of the port, P2).
#[test]
fn saves_beside_the_bundle_are_listed() {
    let dir = temp("k2-steam");
    let app = dir.join("KOTOR2.app");
    let files = bundle(&app, "com.aspyr.kotor2", "GameData");
    game_files(&files, "chitin.key", "001ebo");
    let account = dir.join("cloudsaves").join("12345");
    fs::create_dir_all(&account).unwrap();
    let install = games::Install { game: Game::K2, path: files.clone(), source: "Steam".into() };
    let roots = games::save_roots(&install);
    assert!(roots.iter().any(|(p, k)| p == &account && *k == games::SaveKind::Cloud), "{:?}", roots);
    // and their backups go beside the bundle too, outside Steam Cloud's folder
    let save = account.join("000001 - QuickSave");
    fs::create_dir_all(&save).unwrap();
    assert_eq!(save::backup_root(&save, Some(&files)), dir.join("Save Editor Backup"));
}

/// The Mac App Store's KotOR II keeps a dialog.tlk per language. The editor cannot tell which
/// the game shows, so it reads English's and writes names in English (the maintainer's
/// choice, 2026-11-08).
#[test]
fn a_game_with_a_text_per_language_is_read_in_english() {
    let dir = temp("localized");
    let files = bundle(&dir.join("KOTOR2.app"), "com.aspyr.kotor2.appstore", "GameData");
    fs::create_dir_all(files.join("modules")).unwrap();
    fs::write(files.join("chitin.key"), b"KEY V1  ").unwrap();
    fs::write(files.join("modules").join("001ebo.rim"), b"RIM V1.0").unwrap();
    for lang in ["German", "English", "French"] {
        fs::create_dir_all(files.join("Localized").join(lang)).unwrap();
        fs::write(files.join("Localized").join(lang).join("dialog.tlk"), b"TLK V3.0\0\0\0\0").unwrap();
    }
    assert!(games::has_game_files(&files));
    assert_eq!(games::localized_dir(&files), Some(files.join("Localized").join("English")));
    assert_eq!(games::tlk_path(&files), Some(files.join("Localized").join("English").join("dialog.tlk")));
    let data = kse_core::gamedata::GameData::open(Game::K2, &files);
    assert_eq!(data.name_languages(), vec![0]);
    // a game with its own dialog.tlk beside chitin.key: that one, as before
    fs::write(files.join("dialog.tlk"), b"TLK V3.0\x02\0\0\0").unwrap();
    assert_eq!(games::tlk_path(&files), Some(files.join("dialog.tlk")));
}

/// The copies on this Mac and how each stands, as the pop-up lists them: every copy found is
/// one row (the Mac App Store copies Spotlight finds in other folders share one container, so
/// one row), a closed container is a red row, and the Steam copies' saves are counted.
#[cfg(target_os = "macos")]
#[test]
fn each_copy_on_this_mac_is_one_row() {
    let installs = games::find_installs(&[]);
    let copies = games::game_copies(&installs, &[]);
    for c in &copies {
        println!("{:<22} {:?} {:?}", c.name(), c.state, c.blocked);
    }
    let mut names: Vec<String> = copies.iter().map(|c| c.name()).collect();
    names.sort();
    let before = names.len();
    names.dedup();
    assert_eq!(names.len(), before, "one row per copy");
    for c in &copies {
        assert_eq!(c.state == games::CopyState::Blocked, !c.blocked.is_empty(), "{}", c.name());
    }
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        let container = home.join("Library/Containers/com.aspyr.kotor/Data");
        let closed = matches!(fs::read_dir(&container), Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied);
        if closed && installs.iter().any(|i| i.game == Game::K1 && i.source == "Mac App Store") {
            let row = copies.iter().find(|c| c.name() == "App Store KotOR").expect("the App Store KotOR row");
            assert_eq!(row.state, games::CopyState::Blocked);
            assert!(row.blocked.contains(&container));
        }
    }
}

/// A folder inside a closed place that was picked and can be read now (Choose the Folder)
/// opens that copy: its row is no longer red, and the saves there count for it.
#[cfg(unix)]
#[test]
fn a_picked_folder_opens_a_closed_copy() {
    use std::os::unix::fs::PermissionsExt;
    let dir = temp("copies");
    let files = bundle(&dir.join("KOTOR2.app"), "com.aspyr.kotor2", "GameData");
    game_files(&files, "chitin.key", "001ebo");
    let cloud = dir.join("cloudsaves");
    let account = cloud.join("1");
    fs::create_dir_all(&account).unwrap();
    let install = games::Install { game: Game::K2, path: files.clone(), source: "Steam".into() };
    // no saves yet: grey
    let rows = games::game_copies(&[install.clone()], &[]);
    let row = rows.iter().find(|c| c.name() == "Steam KotOR II").unwrap();
    assert_eq!(row.state, games::CopyState::NoSaves);
    // closed: red
    fs::set_permissions(&account, fs::Permissions::from_mode(0o000)).unwrap();
    let closed = fs::read_dir(&account).is_err();
    let rows = games::game_copies(&[install.clone()], &[]);
    fs::set_permissions(&account, fs::Permissions::from_mode(0o755)).unwrap();
    if closed {
        let row = rows.iter().find(|c| c.name() == "Steam KotOR II").unwrap();
        assert_eq!(row.state, games::CopyState::Blocked);
        assert_eq!(row.blocked, vec![account.clone()]);
    }
    // a save there, the folder picked and readable: counted, not red
    let save = account.join("000001 - Game0");
    fs::create_dir_all(&save).unwrap();
    let mut root = kse_core::formats::gff::Struct::new(u32::MAX);
    root.fields.push(("SAVEGAMENAME".into(), kse_core::formats::gff::Value::String(b"T".to_vec())));
    root.fields.push(("PCNAME".into(), kse_core::formats::gff::Value::String(b"Exile".to_vec())));
    fs::write(save.join("savenfo.res"), kse_core::formats::gff::Gff { file_type: *b"NFO ", root }.write()).unwrap();
    let rows = games::game_copies(&[install], &[account.clone()]);
    let row = rows.iter().find(|c| c.name() == "Steam KotOR II").unwrap();
    assert_eq!(row.state, games::CopyState::Saves(1));
}
