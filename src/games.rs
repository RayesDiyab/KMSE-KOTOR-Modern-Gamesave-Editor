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

fn is_install(game: Game, path: &Path) -> bool {
    path.join("chitin.key").is_file()
        && (path.join(game.exe()).is_file()
            || fs::read_dir(path).map_or(false, |d| {
                d.filter_map(|e| e.ok()).any(|e| {
                    let n = e.file_name().to_string_lossy().to_ascii_lowercase();
                    n.starts_with(&game.exe()[..game.exe().len() - 4]) || n.ends_with(".app")
                })
            }))
}

/// Which game a folder holds, by its executable or, for the ports, its dialog.tlk size.
pub fn identify(path: &Path) -> Option<Game> {
    if !path.join("chitin.key").is_file() {
        return None;
    }
    if path.join("swkotor2.exe").is_file() || path.join("swkotor2.ini").is_file() {
        return Some(Game::K2);
    }
    if path.join("swkotor.exe").is_file() || path.join("swkotor.ini").is_file() {
        return Some(Game::K1);
    }
    // KotOR 2's ports name things differently; its chitin.key lists far more resources.
    let len = fs::metadata(path.join("chitin.key")).map(|m| m.len()).unwrap_or(0);
    Some(if len > 700_000 { Game::K2 } else { Game::K1 })
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
    let mut add = |game: Game, path: PathBuf, source: &str| {
        let path = clean(&path);
        if is_install(game, &path) && !installs.iter().any(|i| i.path == path) {
            installs.push(Install { game, path, source: source.to_string() });
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
    if let Some(h) = home() {
        // The Aspyr ports keep their files inside the app bundle on macOS.
        for (game, rel) in [
            (Game::K1, "Library/Application Support/Steam/steamapps/common/swkotor/KOTOR.app/Contents/KOTOR Data"),
            (Game::K2, "Library/Application Support/Steam/steamapps/common/Knights of the Old Republic II/KOTOR2.app/Contents/GameData"),
        ] {
            add(game, h.join(rel), "Steam");
        }
    }
    for path in extra {
        if let Some(game) = identify(path) {
            add(game, path.clone(), "Added");
        }
    }
    installs
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
    if let Some(cloud) = child_ci(&install.path, "cloudsaves") {
        if let Ok(accounts) = fs::read_dir(&cloud) {
            for a in accounts.filter_map(|e| e.ok()).filter(|e| e.path().is_dir()) {
                roots.push((a.path(), SaveKind::Cloud));
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

/// The Aspyr ports' own save folders, which live outside the game.
fn port_save_roots(game: Game) -> Vec<PathBuf> {
    let Some(h) = home() else { return Vec::new() };
    match game {
        Game::K1 => vec![
            h.join("Library/Application Support/Knights of the Old Republic/saves"),
            h.join("Library/Containers/com.aspyr.kotor/Data/Library/Application Support/KOTOR/saves"),
        ],
        Game::K2 => vec![
            h.join(".local/share/aspyr-media/kotor2/saves"),
            h.join("Library/Application Support/Star Wars Knights of the Old Republic II/saves"),
            h.join("Library/Application Support/Aspyr/Star Wars Knights of the Old Republic II/saves"),
        ],
    }
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
