//! Compiles the interface (ui/app.slint) into the program.
//!
//! This repository builds on its own. The interface is set in the KMRP
//! Original HD fonts (KMRP's fonts/, SIL Open Font License 1.1), four of which are
//! committed in fonts/ here with their licence. The folder KMRP_FONTS names is used
//! instead when it holds them all. This script writes kmrp-fonts.slint into the build
//! folder, embedding the fonts; without them it is empty, the window falls back to the
//! system's font and Cargo prints a warning.
//!
//! The timeline's Yavin Station is a frame of KotOR's own movie Live_1c.bik, so it is made
//! here from the player's game, never committed (2026-10-24, the maintainer): ffmpeg takes
//! the frame, and assets/timeline/yavin-station-mask.png (the station's outline, made once
//! with rembg) cuts it out. timeline-art.slint, written into the build folder, holds the
//! picture; without the game or ffmpeg it holds none and the button shows the name alone.

use std::path::{Path, PathBuf};

const FONTS: [&str; 4] = [
    "KMRP-OriginalHD-Menu.otf",
    "KMRP-OriginalHD-MenuMedium.otf",
    "KMRP-OriginalHD-MenuSmallB.otf",
    "KMRP-OriginalHD-Descriptions.otf",
];

fn fonts_dir(here: &Path) -> Option<PathBuf> {
    let present = |d: &Path| FONTS.iter().all(|f| d.join(f).is_file());
    let named = std::env::var_os("KMRP_FONTS").map(PathBuf::from);
    named.into_iter().chain([here.join("fonts")]).find(|d| present(d))
}

/// KotOR's folder: KOTOR_DIR, else the Steam libraries (libraryfolders.vdf), else the
/// usual Steam and GOG folders. The one holding Movies/Live_1c.bik.
fn kotor_dir() -> Option<PathBuf> {
    let movie = |d: &Path| d.join("Movies").join("Live_1c.bik").is_file() || d.join("movies").join("live_1c.bik").is_file();
    let mut tries: Vec<PathBuf> = std::env::var_os("KOTOR_DIR").map(PathBuf::from).into_iter().collect();
    let steam = [
        PathBuf::from(r"C:\Program Files (x86)\Steam"),
        PathBuf::from(r"C:\Program Files\Steam"),
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support/Steam")).unwrap_or_default(),
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".steam/steam")).unwrap_or_default(),
    ];
    for s in steam {
        tries.push(s.join("steamapps/common/swkotor"));
        if let Ok(vdf) = std::fs::read_to_string(s.join("steamapps/libraryfolders.vdf")) {
            for line in vdf.lines() {
                let line = line.trim();
                if let Some(rest) = line.strip_prefix("\"path\"") {
                    let lib = rest.trim().trim_matches('"').replace(r"\\", r"\");
                    tries.push(PathBuf::from(lib).join("steamapps/common/swkotor"));
                }
            }
        }
    }
    tries.push(PathBuf::from(r"C:\GOG Games\Star Wars - KotOR"));
    tries.into_iter().find(|d| movie(d))
}

/// Yavin Station for the timeline, from the game's movie: frame 484 of Live_1c.bik, the
/// 250 by 144 crop at (256, 52), enlarged four times, brightened per channel towards the
/// picture the maintainer chose (a brighter copy of the same shot), then a tone curve that
/// softens the sunlit top left and lifts the shadowed hull, with the colour given back
/// (2026-10-24, the maintainer), cut out by the committed outline and made 400 pixels wide.
/// Returns where it was written.
fn yavin_station(here: &Path, out: &Path) -> Option<PathBuf> {
    let game = kotor_dir()?;
    let movie = [game.join("Movies").join("Live_1c.bik"), game.join("movies").join("live_1c.bik")].into_iter().find(|p| p.is_file())?;
    let mask = here.join("assets").join("timeline").join("yavin-station-mask.png");
    let target = out.join("yavin-station.png");
    let filter = concat!(
        r"[0:v]select=eq(n\,484),crop=250:144:256:52,scale=1000:576:flags=lanczos,crop=903:515:68:32,",
        r"lutrgb=r='clip(val*1.1186+7.46\,0\,255)':g='clip(val*1.1395+6.55\,0\,255)':b='clip(val*1.3511+4.8\,0\,255)',",
        r"curves=master='0/0 0.08/0.2 0.25/0.45 0.5/0.66 0.8/0.82 1/0.9',eq=saturation=1.35,format=rgba[c];",
        r"[1:v]format=gray[m];[c][m]alphamerge,scale=400:-1:flags=lanczos"
    );
    let ok = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-y", "-i"])
        .arg(&movie)
        .arg("-i")
        .arg(&mask)
        .args(["-filter_complex", filter, "-frames:v", "1"])
        .arg(&target)
        .status()
        .map_or(false, |s| s.success());
    (ok && target.is_file()).then_some(target)
}

fn main() {
    // read when the script runs, not fixed when it was compiled: a moved or renamed checkout
    // would otherwise look for its fonts and art in the old folder
    let here = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    println!("cargo:rerun-if-changed=ui");
    println!("cargo:rerun-if-changed=fonts");
    println!("cargo:rerun-if-env-changed=KMRP_FONTS");

    // The quest databases in data/quests (k1-*.json, k2-*.json), compiled in; questdb.rs
    // includes the list written here.
    println!("cargo:rerun-if-changed=data/quests");
    let mut files: Vec<PathBuf> = std::fs::read_dir(here.join("data").join("quests"))
        .map(|d| d.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().map_or(false, |x| x == "json")).collect())
        .unwrap_or_default();
    files.sort();
    let mut list = String::from("&[\n");
    for f in &files {
        let name = f.file_name().unwrap().to_string_lossy().to_lowercase();
        let game = if name.starts_with("k2") { "Game::K2" } else { "Game::K1" };
        list.push_str(&format!("    ({}, include_str!({:?})),\n", game, f.display().to_string()));
    }
    list.push_str("]\n");
    std::fs::write(out.join("quest_files.rs"), list).unwrap();

    let mut slint = String::from("// Written by build.rs.\n");
    match fonts_dir(&here) {
        Some(dir) => {
            for f in FONTS {
                let path = dir.join(f).to_string_lossy().replace('\\', "/");
                let path = path.trim_start_matches("//?/").to_string();
                slint.push_str(&format!("import \"{}\";\n", path));
            }
        }
        None => println!(
            "cargo:warning=KMRP's fonts are not in fonts/ (or KMRP_FONTS); the window uses the system font"
        ),
    }
    slint.push_str("export global KmrpFonts { }\n");
    std::fs::write(out.join("kmrp-fonts.slint"), slint).unwrap();

    // The timeline's Yavin Station, made from the player's game (see the top of this file).
    println!("cargo:rerun-if-changed=assets/timeline/yavin-station-mask.png");
    println!("cargo:rerun-if-env-changed=KOTOR_DIR");
    let art = match yavin_station(&here, &out) {
        Some(p) => {
            let path = p.to_string_lossy().replace('\\', "/");
            let path = path.trim_start_matches("//?/").to_string();
            format!("export global TimelineArt {{\n    out property <image> yavin: @image-url(\"{}\");\n    out property <bool> has-yavin: true;\n}}\n", path)
        }
        None => {
            println!("cargo:warning=KotOR's movie Live_1c.bik or ffmpeg was not found (set KOTOR_DIR); the timeline's Yavin Station shows its name only");
            "export global TimelineArt {\n    out property <image> yavin;\n    out property <bool> has-yavin: false;\n}\n".to_string()
        }
    };
    std::fs::write(out.join("timeline-art.slint"), format!("// Written by build.rs.\n{}", art)).unwrap();

    let config = slint_build::CompilerConfiguration::new().with_include_paths(vec![out.clone()]);
    slint_build::compile_with_config("ui/app.slint", config).expect("the user interface did not compile");

    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/save-editor.ico");
        res.set("ProductName", "KOTOR Modern Gamesave Editor");
        res.set("FileDescription", "KOTOR Modern Gamesave Editor");
        res.set("CompanyName", "RaymanGT");
        let _ = res.compile();
    }
}
