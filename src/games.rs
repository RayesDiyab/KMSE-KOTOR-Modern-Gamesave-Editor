//! Finding the games and their saves without any setup: Steam libraries (Windows, Linux,
//! macOS, Flatpak), GOG and the retail discs through the registry, Windows'
//! VirtualStore copies, KotOR 2's Steam Cloud folder, the Aspyr ports' own folders, and
//! any folder the player adds by hand.

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Game {
    K1,
    K2,
}

impl Game {
    pub fn title(self) -> &'static str {
        match self {
            Game::K1 => "KotOR",
            Game::K2 => "KotOR II",
        }
    }

    fn steam_folder(self) -> &'static str {
        match self {
            Game::K1 => "swkotor",
            Game::K2 => "Knights of the Old Republic II",
        }
    }

    #[cfg_attr(not(windows), allow(dead_code))]
    fn exe(self) -> &'static str {
        match self {
            Game::K1 => "swkotor.exe",
            Game::K2 => "swkotor2.exe",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Install {
    pub game: Game,
    pub path: PathBuf,
    pub source: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveKind {
    Local,
    Cloud,
    Added,
}

#[derive(Clone, Debug)]
pub struct SaveFolder {
    pub game: Game,
    pub path: PathBuf,
    pub kind: SaveKind,
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from)
}

/// Where the editor keeps its settings (the backups of a save go beside the saves, in
/// `Save Editor Backup`: save::backup_root; here only when no save is open):
/// %LOCALAPPDATA%\KOTOR Modern Gamesave Editor, ~/Library/Application Support/KOTOR Modern Gamesave Editor, or
/// $XDG_DATA_HOME/KOTOR Modern Gamesave Editor. Never inside the game folder. Until 2026-10-24 it was named
/// KMRP Save Editor and kept them in KMRP\Save Editor there: that folder is moved here the
/// first time (if the move fails, it is used as it is, so nothing is lost).
pub fn data_dir() -> PathBuf {
    let base = if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        home().map(|h| h.join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| home().map(|h| h.join(".local/share")))
    };
    let base = base.unwrap_or_else(std::env::temp_dir);
    let dir = base.join("KOTOR Modern Gamesave Editor");
    let old = base.join("KMRP").join("Save Editor");
    if !dir.exists() && old.is_dir() && std::fs::rename(&old, &dir).is_err() {
        return old;
    }
    dir
}

/// The real path without Windows' \\?\ prefix, so it reads as the player knows it.
pub fn clean(path: &Path) -> PathBuf {
    strip_verbatim(&fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()))
}

/// A path in the form Explorer and the player know: \\?\C:\x is C:\x, and \\?\UNC\server\share
/// is \\server\share. Until 2026-11-08 the UNC form was kept, and Explorer could not open an
/// install on a network drive (2026-11-07 audit, W-19). Other \\?\ forms (a volume's GUID)
/// have no shorter spelling and are kept.
pub fn strip_verbatim(real: &Path) -> PathBuf {
    let text = real.to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{}", rest));
    }
    match text.strip_prefix(r"\\?\") {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => PathBuf::from(rest),
        _ => real.to_path_buf(),
    }
}

/// file_ci, for the rest of the library.
pub fn file_ci_pub(dir: &Path, name: &str) -> Option<PathBuf> {
    file_ci(dir, name)
}

/// A file directly in `dir` whatever the case of its name: a case-sensitive disk keeps
/// CHITIN.KEY and chitin.key apart (macOS and Linux).
fn file_ci(dir: &Path, name: &str) -> Option<PathBuf> {
    let direct = dir.join(name);
    if direct.is_file() {
        return Some(direct);
    }
    fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(name) && e.path().is_file())
        .map(|e| e.path())
}

/// Whether `dir` holds the game's resources the editor reads: a chitin.key that is a KEY
/// file, a dialog.tlk that is a TLK file, and a modules folder. A folder that has the files
/// in name only (an empty chitin.key, no modules) is not taken, so it cannot fail later in
/// the item, feat, portrait or map lists with no word.
pub fn has_game_files(dir: &Path) -> bool {
    let starts = |name: &str, magic: &[u8]| {
        file_ci(dir, name).map_or(false, |p| {
            use std::io::Read;
            let mut head = vec![0u8; magic.len()];
            fs::File::open(p).and_then(|mut f| f.read_exact(&mut head)).is_ok() && head == magic
        })
    };
    let tlk = tlk_path(dir).map_or(false, |p| {
        use std::io::Read;
        let mut head = [0u8; 8];
        fs::File::open(p).and_then(|mut f| f.read_exact(&mut head)).is_ok() && &head == b"TLK V3.0"
    });
    starts("chitin.key", b"KEY V1  ") && tlk && child_ci(dir, "modules").is_some()
}

/// The language folder a game keeps its text in, when it has them: the Mac App Store's KotOR
/// II (Aspyr, 2015) has no dialog.tlk beside chitin.key but one in each of
/// GameData/Localized/English, French, German, Italian and Spanish, each with its own
/// Override. Which one the game shows is its own setting, which the editor cannot read, so it
/// takes English (the maintainer's choice, 2026-11-08), else the first there is.
pub fn localized_dir(root: &Path) -> Option<PathBuf> {
    let localized = child_ci(root, "Localized")?;
    if let Some(english) = child_ci(&localized, "English") {
        return Some(english);
    }
    let mut all: Vec<PathBuf> = fs::read_dir(&localized).ok()?.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_dir()).collect();
    all.sort();
    all.into_iter().next()
}

/// The game's dialog.tlk: beside chitin.key, else in the language folder (localized_dir).
pub fn tlk_path(root: &Path) -> Option<PathBuf> {
    file_ci(root, "dialog.tlk").or_else(|| localized_dir(root).and_then(|d| file_ci(&d, "dialog.tlk")))
}

/// The folder of game files a picked path stands for. A game folder itself; a macOS app
/// bundle (`Knights of the Old Republic.app` keeps them in Contents/Assets, `KOTOR2.app` in
/// Contents/GameData, Steam's older KotOR build in Contents/KOTOR Data); or a folder that
/// holds such a bundle (Steam's `steamapps/common/swkotor`). None when there are none.
pub fn resource_root(path: &Path) -> Option<PathBuf> {
    if has_game_files(path) {
        return Some(path.to_path_buf());
    }
    let in_bundle = |app: &Path| -> Option<PathBuf> {
        let contents = child_ci(app, "Contents")?;
        for sub in ["Assets", "GameData", "KOTOR Data", "Resources"] {
            if let Some(d) = child_ci(&contents, sub).filter(|d| has_game_files(d)) {
                return Some(d);
            }
        }
        None
    };
    if is_bundle(path) {
        return in_bundle(path);
    }
    // a folder holding the bundle: Steam's install folder, or one the player made
    let mut apps: Vec<PathBuf> = fs::read_dir(path)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| is_bundle(p))
        .collect();
    apps.sort();
    apps.iter().find_map(|a| in_bundle(a))
}

fn is_bundle(path: &Path) -> bool {
    path.extension().map_or(false, |x| x.eq_ignore_ascii_case("app")) && path.is_dir()
}

/// The app bundle a path is in (the innermost one), if any.
fn enclosing_bundle(path: &Path) -> Option<PathBuf> {
    path.ancestors().find(|a| is_bundle(a)).map(Path::to_path_buf)
}

/// Which KotOR an app bundle is, by its bundle identifier: Aspyr's ports are
/// com.aspyr.kotor (Mac App Store), com.aspyr.kotor.steam (Steam) and
/// com.aspyr.kotor2.appstore, or com.aspyr.kotor2 elsewhere. The Info.plist is searched as
/// bytes, so a binary property list reads as well as an XML one.
pub fn bundle_game(app: &Path) -> Option<Game> {
    let plist = fs::read(app.join("Contents").join("Info.plist")).ok()?;
    let has = |needle: &[u8]| plist.windows(needle.len()).any(|w| w.eq_ignore_ascii_case(needle));
    if has(b"com.aspyr.kotor2") {
        Some(Game::K2)
    } else if has(b"com.aspyr.kotor") {
        Some(Game::K1)
    } else {
        None
    }
}

fn is_install(game: Game, path: &Path) -> bool {
    has_game_files(path) && identify(path) == Some(game)
}

/// Which game a folder of game files is: by the Windows program or the game's ini there, then
/// by the macOS bundle it is in, then by its own first module (KotOR's Endar Spire,
/// `end_m01aa`, or KotOR II's Ebon Hawk, `001ebo`). Until 2026-11-08 the last step was the
/// size of chitin.key (KotOR II above 700,000 bytes), which takes the Mac App Store's KotOR II
/// (406,035 bytes) for KotOR.
pub fn identify(path: &Path) -> Option<Game> {
    file_ci(path, "chitin.key")?;
    if file_ci(path, "swkotor2.exe").is_some() || file_ci(path, "swkotor2.ini").is_some() {
        return Some(Game::K2);
    }
    if file_ci(path, "swkotor.exe").is_some() || file_ci(path, "swkotor.ini").is_some() {
        return Some(Game::K1);
    }
    if let Some(game) = enclosing_bundle(path).and_then(|a| bundle_game(&a)) {
        return Some(game);
    }
    let modules = child_ci(path, "modules")?;
    let names: Vec<String> = fs::read_dir(modules).ok()?.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().to_ascii_lowercase()).collect();
    let has = |stem: &str| names.iter().any(|n| n.split('.').next() == Some(stem) || n.starts_with(&format!("{}_", stem)));
    match (has("end_m01aa"), has("001ebo")) {
        (true, false) => Some(Game::K1),
        (false, true) => Some(Game::K2),
        _ => None,
    }
}

/// Where an install came from, as Settings shows it: Steam, the Mac App Store (its bundle
/// has a receipt) or another Mac copy; the source given otherwise.
fn mac_source(path: &Path, given: &str) -> String {
    let steam = path.ancestors().any(|a| a.file_name().map_or(false, |n| n.eq_ignore_ascii_case("steamapps")));
    match enclosing_bundle(path) {
        _ if steam => "Steam".to_string(),
        Some(app) if app.join("Contents").join("_MASReceipt").is_dir() => "Mac App Store".to_string(),
        Some(_) if given == "Added" => "Added".to_string(),
        Some(_) => "Mac".to_string(),
        None => given.to_string(),
    }
}

fn steam_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    #[cfg(windows)]
    {
        use winreg::enums::*;
        use winreg::RegKey;
        if let Ok(key) = RegKey::predef(HKEY_CURRENT_USER).open_subkey("Software\\Valve\\Steam") {
            if let Ok(p) = key.get_value::<String, _>("SteamPath") {
                roots.push(PathBuf::from(p));
            }
        }
        // where the installer put Steam, for every user of the PC; HKCU's SteamPath above is
        // missing until Steam has run for this user (2026-11-07 audit, W-20)
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        for key in ["SOFTWARE\\WOW6432Node\\Valve\\Steam", "SOFTWARE\\Valve\\Steam"] {
            if let Ok(p) = hklm.open_subkey(key).and_then(|k| k.get_value::<String, _>("InstallPath")) {
                roots.push(PathBuf::from(p));
            }
        }
        for v in ["ProgramFiles(x86)", "ProgramFiles"] {
            if let Some(p) = std::env::var_os(v) {
                roots.push(PathBuf::from(p).join("Steam"));
            }
        }
    }
    if let Some(h) = home() {
        roots.push(h.join(".steam/steam"));
        roots.push(h.join(".local/share/Steam"));
        roots.push(h.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"));
        roots.push(h.join("Library/Application Support/Steam"));
    }
    roots
}

/// Every Steam library folder, from each Steam root's libraryfolders.vdf.
fn steam_libraries() -> Vec<PathBuf> {
    let mut libraries: Vec<PathBuf> = Vec::new();
    for root in steam_roots() {
        let vdf = root.join("steamapps").join("libraryfolders.vdf");
        let mut found = vec![root.clone()];
        if let Ok(text) = fs::read_to_string(&vdf) {
            found.extend(library_paths(&text));
        }
        for f in found {
            let canonical = clean(&f);
            if canonical.is_dir() && !libraries.contains(&canonical) {
                libraries.push(canonical);
            }
        }
    }
    libraries
}

/// The library folders a libraryfolders.vdf lists. Since 2021 each is a numbered block with a
/// "path" line; before that each was one line, "1" "D:\\SteamLibrary", which was not read
/// until 2026-11-08 (2026-11-07 audit, W-20). The numbers in a newer file's "apps" blocks are
/// numbered lines too, but no path, and are left out.
pub fn library_paths(vdf: &str) -> Vec<PathBuf> {
    let absolute = |p: &str| {
        let b = p.as_bytes();
        p.starts_with('/') || p.starts_with(r"\\") || (b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/'))
    };
    let mut out = Vec::new();
    for line in vdf.lines() {
        let parts: Vec<&str> = line.split('"').collect();
        if parts.len() < 4 {
            continue;
        }
        let (key, value) = (parts[1], parts[3].replace(r"\\", r"\"));
        let numbered = !key.is_empty() && key.bytes().all(|b| b.is_ascii_digit());
        if key.eq_ignore_ascii_case("path") || (numbered && absolute(&value)) {
            out.push(PathBuf::from(value));
        }
    }
    out
}

#[cfg(windows)]
fn registry_installs() -> Vec<(Game, PathBuf, &'static str)> {
    use winreg::enums::*;
    use winreg::RegKey;
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let mut out = Vec::new();
    let places: [(Game, &str, &str, &'static str); 6] = [
        (Game::K1, "SOFTWARE\\WOW6432Node\\GOG.com\\Games\\1207666283", "path", "GOG"),
        (Game::K2, "SOFTWARE\\WOW6432Node\\GOG.com\\Games\\1421404581", "path", "GOG"),
        (Game::K1, "SOFTWARE\\WOW6432Node\\BioWare\\SW\\KOTOR", "Path", "Disc"),
        (Game::K1, "SOFTWARE\\BioWare\\SW\\KOTOR", "Path", "Disc"),
        (Game::K2, "SOFTWARE\\WOW6432Node\\LucasArts\\KotOR2", "Path", "Disc"),
        (Game::K2, "SOFTWARE\\LucasArts\\KotOR2", "Path", "Disc"),
    ];
    for (game, key, value, source) in places {
        if let Ok(k) = hklm.open_subkey(key) {
            if let Ok(p) = k.get_value::<String, _>(value) {
                out.push((game, PathBuf::from(p), source));
            }
        }
    }
    out
}

#[cfg(not(windows))]
fn registry_installs() -> Vec<(Game, PathBuf, &'static str)> {
    Vec::new()
}

pub fn find_installs(extra: &[PathBuf]) -> Vec<Install> {
    let mut installs: Vec<Install> = Vec::new();
    // each place is taken through resource_root, so a macOS bundle (or the folder holding
    // one) stands for the game files inside it; elsewhere the folder is itself the root
    let mut add = |game: Game, path: PathBuf, source: &str| {
        let Some(root) = resource_root(&path) else { return };
        let path = clean(&root);
        if is_install(game, &path) && !installs.iter().any(|i| i.path == path) {
            let source = if cfg!(target_os = "macos") { mac_source(&path, source) } else { source.to_string() };
            installs.push(Install { game, path, source });
        }
    };
    for library in steam_libraries() {
        for game in [Game::K1, Game::K2] {
            add(game, library.join("steamapps").join("common").join(game.steam_folder()), "Steam");
        }
    }
    for (game, path, source) in registry_installs() {
        add(game, path, source);
    }
    if cfg!(target_os = "macos") {
        for app in mac_bundles() {
            if let Some(game) = bundle_game(&app) {
                add(game, app, "Mac");
            }
        }
    }
    for path in extra {
        if let Some(root) = resource_root(path) {
            if let Some(game) = identify(&root) {
                add(game, root, "Added");
            }
        }
    }
    installs
}

/// The KotOR app bundles on this Mac: those in /Applications and ~/Applications (the Mac App
/// Store's and any copied there), and any other Spotlight knows of by its bundle identifier
/// (another disk, a folder of the player's). Steam's are found through its libraries.
fn mac_bundles() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    let mut dirs = vec![PathBuf::from("/Applications")];
    if let Some(h) = home() {
        dirs.push(h.join("Applications"));
    }
    for dir in dirs {
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        for p in entries.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| is_bundle(p)) {
            if bundle_game(&p).is_some() {
                out.push(p);
            }
        }
    }
    if cfg!(target_os = "macos") {
        out.extend(spotlight_bundles());
    }
    let mut seen = std::collections::HashSet::new();
    out.retain(|p| seen.insert(clean(p)));
    out
}

/// Spotlight's list of KotOR bundles anywhere it indexes, by bundle identifier. By its full
/// path, never a bare name; nothing when Spotlight is off, slow (two seconds at most) or
/// says nothing.
fn spotlight_bundles() -> Vec<PathBuf> {
    // asked once while the editor runs: find_installs runs on the window's thread
    static FOUND: std::sync::OnceLock<Vec<PathBuf>> = std::sync::OnceLock::new();
    FOUND.get_or_init(spotlight_ask).clone()
}

fn spotlight_ask() -> Vec<PathBuf> {
    let mdfind = Path::new("/usr/bin/mdfind");
    if !mdfind.is_file() {
        return Vec::new();
    }
    let child = std::process::Command::new(mdfind)
        .arg("kMDItemCFBundleIdentifier == 'com.aspyr.kotor*'")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn();
    let Ok(mut child) = child else { return Vec::new() };
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if start.elapsed() < std::time::Duration::from_secs(2) => std::thread::sleep(std::time::Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Vec::new();
            }
        }
    }
    let mut text = String::new();
    if let Some(mut out) = child.stdout.take() {
        use std::io::Read;
        let _ = out.read_to_string(&mut text);
    }
    // not one in a Trash
    let trashed = |p: &Path| p.components().any(|c| matches!(c.as_os_str().to_str(), Some(".Trash") | Some(".Trashes")));
    text.lines().map(PathBuf::from).filter(|p| is_bundle(p) && !trashed(p)).collect()
}

fn child_ci(dir: &Path, name: &str) -> Option<PathBuf> {
    fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(name) && e.path().is_dir())
        .map(|e| e.path())
}

pub fn is_save_folder(path: &Path) -> bool {
    path.is_dir()
        && fs::read_dir(path).map_or(false, |d| {
            d.filter_map(|e| e.ok()).any(|e| {
                let n = e.file_name().to_string_lossy().to_ascii_lowercase();
                // savenfo.kmrp-old: a write cut off part way, which Save::open explains
                n == "savenfo.res" || n == "savenfo.kmrp-old"
            })
        })
}

/// A folder as one text however it is spelled: its real path, one kind of slash, no slash at
/// the end, and on Windows and macOS (whose folders ignore case) in lower case. An added
/// folder spelled otherwise than the one found (C:/..., \\?\C:\..., another case) listed the
/// same save twice (2026-11-07 audit, W-21).
fn folder_key(path: &Path) -> String {
    let mut text = clean(path).to_string_lossy().to_string();
    if cfg!(windows) {
        text = text.replace('/', r"\");
    }
    while text.len() > 1 && (text.ends_with('/') || text.ends_with('\\')) {
        text.pop();
    }
    if cfg!(windows) || cfg!(target_os = "macos") {
        text = text.to_lowercase();
    }
    text
}

/// Lists `path` as a save of `game` if it is one and is not listed yet. A save of the other
/// game is left out, wherever it is: a KotOR II save copied into KotOR's saves folder was
/// listed as a KotOR save (2026-11-07 audit, W-21). One whose savenfo.res cannot be read is
/// kept in the game's own folders, where Save::open explains it, but not in an added folder.
fn add_save(path: PathBuf, game: Game, kind: SaveKind, seen: &mut std::collections::HashSet<String>, out: &mut Vec<SaveFolder>) {
    let of_game = match crate::save::peek_game(&path) {
        Some(g) => g == game,
        None => kind != SaveKind::Added,
    };
    if of_game && seen.insert(folder_key(&path)) {
        out.push(SaveFolder { game, path, kind });
    }
}

fn saves_in(dir: &Path, game: Game, kind: SaveKind, seen: &mut std::collections::HashSet<String>, out: &mut Vec<SaveFolder>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if is_save_folder(&path) {
            add_save(path, game, kind, seen, out);
        }
    }
}

/// Folders that hold save folders for an install: its own saves folder, the
/// VirtualStore copy Windows makes for a game under Program Files, and KotOR 2's
/// cloudsaves\<account> folders.
pub fn save_roots(install: &Install) -> Vec<(PathBuf, SaveKind)> {
    let mut roots = Vec::new();
    if let Some(s) = child_ci(&install.path, "saves") {
        roots.push((s, SaveKind::Local));
    }
    if cfg!(windows) {
        if let (Some(local), Some(rest)) = (std::env::var_os("LOCALAPPDATA"), virtual_store_rest(&install.path)) {
            let store = PathBuf::from(local).join("VirtualStore").join(rest);
            if let Some(s) = child_ci(&store, "saves") {
                roots.push((s, SaveKind::Local));
            }
        }
    }
    // The game folder, and on macOS also the folder the app is in: Steam's KotOR II keeps
    // its saves and cloudsaves beside KOTOR2.app, not inside it (seen 2026-11-08).
    let mut places = vec![install.path.clone()];
    if let Some(outside) = enclosing_bundle(&install.path).and_then(|a| a.parent().map(Path::to_path_buf)) {
        places.push(outside);
    }
    for (i, place) in places.iter().enumerate() {
        if i > 0 {
            if let Some(s) = child_ci(place, "saves") {
                roots.push((s, SaveKind::Local));
            }
        }
        if let Some(cloud) = child_ci(place, "cloudsaves") {
            if let Ok(accounts) = fs::read_dir(&cloud) {
                for a in accounts.filter_map(|e| e.ok()).filter(|e| e.path().is_dir()) {
                    roots.push((a.path(), SaveKind::Cloud));
                }
            }
        }
    }
    roots
}

fn virtual_store_rest(path: &Path) -> Option<PathBuf> {
    // C:\Program Files (x86)\LucasArts\SWKotOR is redirected to
    // %LOCALAPPDATA%\VirtualStore\Program Files (x86)\LucasArts\SWKotOR.
    let text = path.to_string_lossy().trim_start_matches("\\\\?\\").to_string();
    if text.get(1..).map_or(false, |r| r.to_ascii_lowercase().starts_with(":\\program files")) {
        Some(PathBuf::from(&text[3..]))
    } else {
        None
    }
}

/// The Aspyr ports' own save folders, which live outside the game. On macOS the Steam copy
/// of KotOR keeps them in ~/Library/Application Support/Knights of the Old Republic/saves
/// (seen 2026-11-08); the Mac App Store copies are sandboxed, so theirs are in their
/// containers, under Application Support in a folder named for the game (every such folder
/// with a `saves` folder in it is taken, since the name was not seen for every copy).
fn port_save_roots(game: Game) -> Vec<PathBuf> {
    let Some(h) = home() else { return Vec::new() };
    let mut roots = match game {
        Game::K1 => vec![
            h.join("Library/Application Support/Knights of the Old Republic/saves"),
            h.join("Library/Containers/com.aspyr.kotor/Data/Library/Application Support/KOTOR/saves"),
        ],
        Game::K2 => vec![
            h.join(".local/share/aspyr-media/kotor2/saves"),
            h.join("Library/Application Support/Star Wars Knights of the Old Republic II/saves"),
            h.join("Library/Application Support/Aspyr/Star Wars Knights of the Old Republic II/saves"),
        ],
    };
    for container in mac_containers(game) {
        let support = container.join("Data/Library/Application Support");
        if let Ok(entries) = fs::read_dir(&support) {
            for e in entries.filter_map(|e| e.ok()).filter(|e| e.path().is_dir()) {
                if let Some(s) = child_ci(&e.path(), "saves") {
                    if !roots.contains(&s) {
                        roots.push(s);
                    }
                }
            }
        }
    }
    roots
}

/// The sandbox containers of the Mac App Store copies of a game (~/Library/Containers/<id>).
fn mac_containers(game: Game) -> Vec<PathBuf> {
    if !cfg!(target_os = "macos") {
        return Vec::new();
    }
    let Some(h) = home() else { return Vec::new() };
    let ids: &[&str] = match game {
        Game::K1 => &["com.aspyr.kotor"],
        Game::K2 => &["com.aspyr.kotor2.appstore", "com.aspyr.kotor2"],
    };
    ids.iter().map(|id| h.join("Library/Containers").join(id)).filter(|p| p.is_dir()).collect()
}

/// The folders where a game's saves may be that the system will not let the editor read:
/// on macOS, another app's container is guarded ("access data from other apps") and a folder
/// may be closed to this user. Each is there but cannot be listed. The save list says so,
/// rather than showing fewer saves with no word.
pub fn blocked_save_folders(installs: &[Install], game: Game) -> Vec<PathBuf> {
    let denied = |p: &Path| matches!(fs::read_dir(p), Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied);
    let mut out: Vec<PathBuf> = Vec::new();
    let mut candidates: Vec<PathBuf> = port_save_roots(game);
    for c in mac_containers(game) {
        candidates.push(c.join("Data"));
    }
    for install in installs.iter().filter(|i| i.game == game) {
        candidates.push(install.path.join("saves"));
    }
    for p in candidates {
        // one closed container is named once, not again for each folder inside it
        if denied(&p) && !out.iter().any(|o| p.starts_with(o)) {
            out.retain(|o| !o.starts_with(&p));
            out.push(p);
        }
    }
    out
}

pub fn find_saves(installs: &[Install], game: Game, added: &[PathBuf]) -> Vec<SaveFolder> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for install in installs.iter().filter(|i| i.game == game) {
        for (root, kind) in save_roots(install) {
            saves_in(&root, game, kind, &mut seen, &mut out);
        }
    }
    for root in port_save_roots(game) {
        saves_in(&root, game, SaveKind::Local, &mut seen, &mut out);
    }
    for path in added {
        if is_save_folder(path) {
            add_save(path.clone(), game, SaveKind::Added, &mut seen, &mut out);
        } else {
            saves_in(path, game, SaveKind::Added, &mut seen, &mut out);
        }
    }
    out
}

/// Which game a running program is, from its executable's path and its short name: the
/// Windows programs by name (under Wine, CrossOver or Proton), the macOS ports by the bundle
/// their executable is in (KotOR's `Knights of the Old Republic.app` runs `AppBundleExe`, a
/// launcher, then `KOTOR_Exe`; KotOR II's `KOTOR2.app` runs `KOTOR2` and `KOTOR2sub`), and the
/// KMRP launcher for the Mac App Store copy as KotOR, since it starts the game. `plist` reads
/// a bundle's game (bundle_game); it is a parameter so the tests can give one.
pub fn game_of_process(path: &str, name: &str, plist: &dyn Fn(&Path) -> Option<Game>) -> Option<Game> {
    let base = Path::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    for n in [base.as_str(), name] {
        let n = n.to_ascii_lowercase();
        if n == "swkotor2.exe" || n == "swkotor2" || n == "kotor2" || n == "kotor2sub" {
            return Some(Game::K2);
        }
        if n == "swkotor.exe" || n == "swkotor" || n == "kotor_exe" {
            return Some(Game::K1);
        }
    }
    let app = Path::new(path).ancestors().find(|a| a.extension().map_or(false, |x| x.eq_ignore_ascii_case("app")))?;
    if let Some(g) = plist(app) {
        return Some(g);
    }
    let id = fs::read(app.join("Contents").join("Info.plist")).ok()?;
    let needle = b"io.github.rayesdiyab.kmrp";
    id.windows(needle.len()).any(|w| w == needle).then_some(Game::K1)
}

/// Whether the game may be running, so a save is not changed under it. On macOS a file the
/// game holds open can still be replaced (Windows refuses with a sharing error), so this check
/// carries all of it there. A check that cannot be made counts as running: nothing is written.
#[cfg(not(windows))]
pub fn game_running(game: Game) -> bool {
    match running_games() {
        Some(list) => list.contains(&game),
        None => true,
    }
}

/// Windows: the audited check of 2026-11-07, unchanged (moved here from main.rs).
#[cfg(windows)]
pub fn game_running(game: Game) -> bool {
    use std::os::windows::process::CommandExt;
    let root = std::env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("C:\\Windows"));
    // CREATE_NO_WINDOW: no console flashes up.
    // By its full path: a bare name is looked for in the program's own folder first, so a
    // tasklist.exe beside the editor (in Downloads) would have run (2026-11-07 audit).
    let out = std::process::Command::new(root.join("System32\\tasklist.exe"))
        .args(["/FI", &format!("IMAGENAME eq {}", game.exe()), "/NH"])
        .creation_flags(0x0800_0000)
        .output();
    // A check that failed says nothing, so it counts as running: nothing is written.
    out.map_or(true, |o| !o.status.success() || String::from_utf8_lossy(&o.stdout).to_ascii_lowercase().contains(game.exe()))
}

/// The game a Windows program runs as, from a process's arguments: under Wine, CrossOver or
/// Whisky the process is the Wine loader (`wine64-preloader`, `wine`), its path and name
/// say nothing, and the game is the `swkotor.exe` or `swkotor2.exe` it was given. `args` is
/// the process's argument list.
pub fn game_in_args(args: &[String]) -> Option<Game> {
    for a in args {
        let base = a.rsplit(|c| c == '/' || c == '\\').next().unwrap_or(a).to_ascii_lowercase();
        if base == "swkotor2.exe" {
            return Some(Game::K2);
        }
        if base == "swkotor.exe" {
            return Some(Game::K1);
        }
    }
    None
}

/// Whether a process is a Wine loader, whose arguments name the program it runs.
pub fn is_wine(path: &str, name: &str) -> bool {
    let p = path.to_ascii_lowercase();
    let n = name.to_ascii_lowercase();
    n.starts_with("wine") || p.rsplit('/').next().map_or(false, |b| b.starts_with("wine")) || n.ends_with(".exe")
}

/// The argument list of process `pid` (KERN_PROCARGS2), or None when it cannot be read.
#[cfg(target_os = "macos")]
fn process_args(pid: i32) -> Option<Vec<String>> {
    use std::os::raw::{c_int, c_uint, c_void};
    extern "C" {
        fn sysctl(name: *mut c_int, namelen: c_uint, oldp: *mut c_void, oldlenp: *mut usize, newp: *mut c_void, newlen: usize) -> c_int;
    }
    const CTL_KERN: c_int = 1;
    const KERN_ARGMAX: c_int = 8;
    const KERN_PROCARGS2: c_int = 49;
    let mut argmax: c_int = 0;
    let mut size = std::mem::size_of::<c_int>();
    let mut mib = [CTL_KERN, KERN_ARGMAX];
    // Safe: the buffers are as long as the sizes passed, and only `size` bytes are read back.
    if unsafe { sysctl(mib.as_mut_ptr(), 2, &mut argmax as *mut c_int as *mut c_void, &mut size, std::ptr::null_mut(), 0) } != 0 || argmax <= 0 {
        return None;
    }
    let mut buf = vec![0u8; argmax as usize];
    let mut size = buf.len();
    let mut mib = [CTL_KERN, KERN_PROCARGS2, pid];
    if unsafe { sysctl(mib.as_mut_ptr(), 3, buf.as_mut_ptr() as *mut c_void, &mut size, std::ptr::null_mut(), 0) } != 0 {
        return None;
    }
    let buf = &buf[..size.min(buf.len())];
    // argc, the program's path, NULs, then argc arguments (the environment after them)
    let argc = i32::from_ne_bytes(buf.get(0..4)?.try_into().ok()?).max(0) as usize;
    let mut rest = &buf[4..];
    let end = rest.iter().position(|&b| b == 0)?;
    rest = &rest[end..];
    let start = rest.iter().position(|&b| b != 0).unwrap_or(rest.len());
    rest = &rest[start..];
    let args: Vec<String> = rest.split(|&b| b == 0).take(argc).map(|a| String::from_utf8_lossy(a).to_string()).collect();
    Some(args)
}

/// The games running now, or None when the programs running could not be listed, or one
/// that may be a game could not be read (then a write is refused, never risked).
#[cfg(target_os = "macos")]
pub fn running_games() -> Option<Vec<Game>> {
    use std::os::raw::{c_char, c_int, c_void};
    extern "C" {
        // libproc, part of libSystem
        fn proc_listallpids(buffer: *mut c_void, buffersize: c_int) -> c_int;
        fn proc_pidpath(pid: c_int, buffer: *mut c_void, buffersize: u32) -> c_int;
        fn proc_name(pid: c_int, buffer: *mut c_void, buffersize: u32) -> c_int;
    }
    // Safe: each buffer is as long as the size passed with it, and only the bytes the call
    // says it wrote are read.
    let mut pids: Vec<c_int> = Vec::new();
    for _ in 0..4 {
        let count = unsafe { proc_listallpids(std::ptr::null_mut(), 0) };
        if count <= 0 {
            return None;
        }
        pids = vec![0; count as usize * 2 + 64];
        let bytes = (pids.len() * std::mem::size_of::<c_int>()) as c_int;
        let n = unsafe { proc_listallpids(pids.as_mut_ptr() as *mut c_void, bytes) };
        if n <= 0 {
            return None;
        }
        // a full buffer may have been cut short: listed again, larger
        if (n as usize) < pids.len() {
            pids.truncate(n as usize);
            break;
        }
        pids.clear();
    }
    if pids.is_empty() {
        return None;
    }
    let cache: std::cell::RefCell<std::collections::HashMap<PathBuf, Option<Game>>> = Default::default();
    let mut found = Vec::new();
    let me = std::process::id() as c_int;
    for pid in pids.into_iter().filter(|p| *p > 0 && *p != me) {
        let mut path = vec![0u8; 4096];
        let len = unsafe { proc_pidpath(pid, path.as_mut_ptr() as *mut c_void, path.len() as u32) };
        let path = if len > 0 { String::from_utf8_lossy(&path[..len as usize]).to_string() } else { String::new() };
        let mut name = [0 as c_char; 256];
        let nlen = unsafe { proc_name(pid, name.as_mut_ptr() as *mut c_void, name.len() as u32) };
        let name: String = if nlen > 0 { name[..nlen as usize].iter().map(|&c| c as u8 as char).collect() } else { String::new() };
        if path.is_empty() && name.is_empty() {
            continue; // gone since the list was made, or the system's own
        }
        let mut game = game_of_process(&path, &name, &|app: &Path| *cache.borrow_mut().entry(app.to_path_buf()).or_insert_with(|| bundle_game(app)));
        if game.is_none() && path.is_empty() && name.eq_ignore_ascii_case("AppBundleExe") {
            // an Aspyr launcher whose bundle could not be read: it may be either game
            return None;
        }
        if game.is_none() && is_wine(&path, &name) {
            match process_args(pid) {
                Some(args) => game = game_in_args(&args),
                None => return None, // a Wine program whose game cannot be told
            }
        }
        if let Some(g) = game {
            if !found.contains(&g) {
                found.push(g);
            }
        }
    }
    Some(found)
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn running_games() -> Option<Vec<Game>> {
    // Under Proton the process is swkotor.exe. pgrep exits 1 for no match, above that it failed.
    let mut found = Vec::new();
    for (game, names) in [(Game::K1, ["swkotor.exe", "KOTOR", "swkotor"]), (Game::K2, ["swkotor2.exe", "KOTOR2", "swkotor2"])] {
        for n in names {
            let out = std::process::Command::new("pgrep").args(["-x", n]).output().ok()?;
            match out.status.code() {
                Some(0) => {
                    found.push(game);
                    break;
                }
                Some(1) => {}
                _ => return None,
            }
        }
    }
    Some(found)
}

/// How a copy of a game stands, for the list of copies the editor shows when one of them
/// cannot be read (2026-11-08, the maintainer's design).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CopyState {
    /// Its saves can be read: this many.
    Saves(usize),
    /// Its folders can be read and hold no saves, so nothing of it was checked.
    NoSaves,
    /// A folder of its saves cannot be read (macOS keeps another app's container closed).
    Blocked,
}

/// A copy of a game: one distribution (Steam, the Mac App Store, GOG...) of KotOR or KotOR II,
/// with its saves' folders. Two installs of one distribution share their save folders (the Mac
/// App Store copies all save in one container) and are one copy.
#[derive(Clone, Debug)]
pub struct GameCopy {
    pub game: Game,
    /// "Steam", "App Store", "GOG"...
    pub source: String,
    pub state: CopyState,
    /// The folders of it that cannot be read.
    pub blocked: Vec<PathBuf>,
}

impl GameCopy {
    /// "Steam KotOR", "App Store KotOR II".
    pub fn name(&self) -> String {
        format!("{} {}", self.source, self.game.title())
    }
}

/// The folders an install's saves are in, and the folders to check can be read: its own
/// (save_roots); on macOS a Mac App Store copy's container and its folders, and for any other
/// Mac or Linux copy the port's own save folders outside a container.
fn copy_folders(install: &Install) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut roots: Vec<PathBuf> = save_roots(install).into_iter().map(|(p, _)| p).collect();
    let mut guards: Vec<PathBuf> = Vec::new();
    let app_store = install.source == "Mac App Store";
    if app_store {
        for c in mac_containers(install.game) {
            guards.push(c.join("Data"));
        }
    }
    for r in port_save_roots(install.game) {
        let in_container = r.components().any(|c| c.as_os_str() == "Containers");
        if in_container == app_store && !roots.contains(&r) {
            roots.push(r);
        }
    }
    guards.extend(roots.iter().cloned());
    (roots, guards)
}

/// Every copy of the games found, with how its saves stand. `added` are the folders the player
/// added: a folder picked inside a closed container (Choose the Folder) that can be read now
/// opens that copy, and its saves count for it.
pub fn game_copies(installs: &[Install], added: &[PathBuf]) -> Vec<GameCopy> {
    let denied = |p: &Path| matches!(fs::read_dir(p), Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied);
    // one copy per game and distribution, its folders merged
    let mut groups: Vec<(Game, String, Vec<PathBuf>, Vec<PathBuf>)> = Vec::new();
    for install in installs {
        let source = match install.source.as_str() {
            "Mac App Store" => "App Store".to_string(),
            s => s.to_string(),
        };
        let (roots, guards) = copy_folders(install);
        match groups.iter_mut().find(|g| g.0 == install.game && g.1 == source) {
            Some(g) => {
                g.2.extend(roots.into_iter().filter(|r| !g.2.contains(r)).collect::<Vec<_>>());
                g.3.extend(guards.into_iter().filter(|r| !g.3.contains(r)).collect::<Vec<_>>());
            }
            None => groups.push((install.game, source, roots, guards)),
        }
    }
    let mut out = Vec::new();
    for (game, source, roots, guards) in groups {
        let opened = |g: &Path| added.iter().any(|a| a.starts_with(g) && fs::read_dir(a).is_ok());
        let mut blocked: Vec<PathBuf> = Vec::new();
        for g in &guards {
            if denied(g) && !opened(g) && !blocked.iter().any(|b| g.starts_with(b)) {
                blocked.retain(|b| !b.starts_with(g));
                blocked.push(g.clone());
            }
        }
        let places: Vec<&PathBuf> = roots.iter().chain(guards.iter()).collect();
        let saves = find_saves(installs, game, added).into_iter().filter(|f| places.iter().any(|p| f.path.starts_with(p))).count();
        let state = if !blocked.is_empty() {
            CopyState::Blocked
        } else if saves > 0 {
            CopyState::Saves(saves)
        } else {
            CopyState::NoSaves
        };
        out.push(GameCopy { game, source, state, blocked });
    }
    // what works first, what cannot be read last; KotOR before KotOR II
    let rank = |c: &GameCopy| match c.state {
        CopyState::Saves(_) => 0,
        CopyState::NoSaves => 1,
        CopyState::Blocked => 2,
    };
    out.sort_by_key(|c| (rank(c), c.game == Game::K2));
    out
}
