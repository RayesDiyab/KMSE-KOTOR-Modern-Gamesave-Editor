//! The 2026-11-07 Windows audit's small findings in the file readers and in finding the games
//! and their saves (F-3, F-4, F-5, F-7, F-8 with W-9, W-19, W-20, W-21). Every file here is
//! made up, in memory or in a temporary folder; no save or game file is read or written.
//!
//! The readers must not set memory aside for what a file only claims to hold. This test
//! program counts what each test's own thread asks for, so a reader that sets aside first
//! and checks later is seen here, not only when a PC runs out of memory.

use kse_core::formats::erf::{Erf, Resource};
use kse_core::formats::gff::{Gff, LocString, Struct, Value};
use kse_core::formats::rim::Rim;
use kse_core::formats::tables::{read_at, Key};
use kse_core::formats::tga;
use kse_core::games::{self, Game, Install, SaveKind};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::path::{Path, PathBuf};

struct Counting;

thread_local! {
    static NOW: Cell<usize> = const { Cell::new(0) };
    static PEAK: Cell<usize> = const { Cell::new(0) };
}

fn grow(n: usize) {
    let _ = NOW.try_with(|c| {
        let v = c.get().saturating_add(n);
        c.set(v);
        let _ = PEAK.try_with(|p| p.set(p.get().max(v)));
    });
}

fn shrink(n: usize) {
    let _ = NOW.try_with(|c| c.set(c.get().saturating_sub(n)));
}

/// More than this is refused outright, so a reader that still asks for gigabytes fails its
/// test instead of taking the PC's memory.
const REFUSE: usize = 1 << 30;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        grow(l.size());
        if l.size() > REFUSE {
            return std::ptr::null_mut();
        }
        System.alloc(l)
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        grow(l.size());
        if l.size() > REFUSE {
            return std::ptr::null_mut();
        }
        System.alloc_zeroed(l)
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        shrink(l.size());
        System.dealloc(p, l)
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, new: usize) -> *mut u8 {
        if new > l.size() {
            grow(new - l.size());
        } else {
            shrink(l.size() - new);
        }
        if new > REFUSE {
            return std::ptr::null_mut();
        }
        System.realloc(p, l, new)
    }
}

#[global_allocator]
static COUNTING: Counting = Counting;

/// What `f` returns, and the most memory it held at once on this thread.
fn peak<T>(f: impl FnOnce() -> T) -> (T, usize) {
    let base = NOW.with(|c| c.get());
    PEAK.with(|p| p.set(base));
    let out = f();
    (out, PEAK.with(|p| p.get()) - base)
}

fn le(v: &mut Vec<u8>, n: u32) {
    v.extend_from_slice(&n.to_le_bytes());
}

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("kmse-tests").join("p3").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A RIM V1.0 with one resource: its table claims `size` bytes at `offset`, and `body`
/// follows the table.
fn rim(name: &str, kind: u32, offset: u32, size: u32, body: &[u8]) -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(b"RIM V1.0");
    le(&mut v, 0);
    le(&mut v, 1);
    le(&mut v, 120);
    v.resize(120, 0);
    let mut n = [0u8; 16];
    n[..name.len()].copy_from_slice(name.as_bytes());
    v.extend_from_slice(&n);
    le(&mut v, kind);
    le(&mut v, 0);
    le(&mut v, offset);
    le(&mut v, size);
    v.extend_from_slice(body);
    v
}

// F-3: the readers of the game's archives set aside what a table claims before checking the
// file holds it.

#[test]
fn read_at_checks_the_file_is_that_large_first() {
    let dir = temp("read-at");
    let path = dir.join("sixteen.bin");
    std::fs::write(&path, (0u8..16).collect::<Vec<u8>>()).unwrap();
    assert_eq!(read_at(&path, 4, 8), Some((4u8..12).collect::<Vec<u8>>()));
    assert_eq!(read_at(&path, 0, 16).map(|v| v.len()), Some(16));
    assert_eq!(read_at(&path, 8, 16), None, "runs past the end");
    assert_eq!(read_at(&path, u64::MAX, 4), None, "starts past the end");
    let (got, held) = peak(|| read_at(&path, 0, usize::MAX));
    assert_eq!(got, None);
    assert!(held < 1 << 20, "{} bytes set aside for a 16-byte file", held);
}

#[test]
fn rim_entry_larger_than_the_file_is_refused_without_the_memory() {
    let dir = temp("rim-size");
    let path = dir.join("big.rim");
    std::fs::write(&path, rim("big", 2023, 152, 0xFFFF_FFF0, b"GIT V3.2")).unwrap();
    let r = Rim::open(&path).expect("the table itself is sound");
    let (got, held) = peak(|| r.read("big", 2023));
    assert_eq!(got, None);
    assert!(held < 1 << 20, "{} bytes set aside for a {} byte archive", held, 160);
}

#[test]
fn bif_count_larger_than_the_file_is_refused_without_the_memory() {
    let game = temp("bif-count");
    // chitin.key: one BIF, data\x.bif, and one resource in it
    let mut key = Vec::new();
    key.extend_from_slice(b"KEY V1  ");
    le(&mut key, 1);
    le(&mut key, 1);
    le(&mut key, 64);
    le(&mut key, 88);
    key.resize(64, 0);
    le(&mut key, 0);
    le(&mut key, 76);
    key.extend_from_slice(&11u16.to_le_bytes());
    key.extend_from_slice(&0u16.to_le_bytes());
    key.extend_from_slice(b"data\\x.bif\0");
    key.push(0);
    let mut n = [0u8; 16];
    n[..4].copy_from_slice(b"item");
    key.extend_from_slice(&n);
    key.extend_from_slice(&2025u16.to_le_bytes());
    le(&mut key, 0);
    std::fs::write(game.join("chitin.key"), &key).unwrap();
    // the BIF claims 0x1000_0000 resources, a 4 GB table, in 20 bytes
    std::fs::create_dir_all(game.join("data")).unwrap();
    let mut bif = Vec::new();
    bif.extend_from_slice(b"BIFFV1  ");
    le(&mut bif, 0x1000_0000);
    le(&mut bif, 0);
    le(&mut bif, 20);
    std::fs::write(game.join("data").join("x.bif"), &bif).unwrap();
    let k = Key::open(&game).expect("the key itself is sound");
    let (got, held) = peak(|| k.read("item", 2025));
    assert_eq!(got, None);
    assert!(held < 1 << 20, "{} bytes set aside for a 20-byte BIF", held);
}

// F-4: labels and resource names are read one byte to a character, and were written back as
// UTF-8.

#[test]
fn gff_labels_are_written_back_byte_for_byte() {
    let mut root = Struct::new(u32::MAX);
    root.fields.push(("Lab\u{e9}".into(), Value::Byte(1)));
    root.fields.push(("ABCDEFGHIJKLMNO\u{e9}".into(), Value::Byte(2)));
    let gff = Gff { file_type: *b"RES ", root };
    let bytes = gff.write();
    assert!(bytes.windows(5).any(|w| w == b"Lab\xe9\0"), "the label is one byte a letter");
    assert!(bytes.windows(16).any(|w| w == b"ABCDEFGHIJKLMNO\xe9"), "a 16-letter label keeps its last");
    let back = Gff::read(&bytes).unwrap();
    assert_eq!(back, gff);
    assert_eq!(back.write(), bytes, "read then written is the same file");
}

#[test]
fn erf_names_are_written_back_byte_for_byte() {
    let erf = Erf {
        file_type: *b"MOD ",
        version: *b"V1.0",
        language_count: 0,
        localized: Vec::new(),
        build_year: 0,
        build_day: 0,
        description_strref: u32::MAX,
        reserved: vec![0; 116],
        resources: vec![
            Resource { name: "r\u{e9}s".into(), kind: 2017, id: 0, data: vec![1, 2, 3] },
            Resource { name: "abcdefghijklmno\u{e9}".into(), kind: 2017, id: 1, data: vec![4] },
        ],
    };
    let bytes = erf.write();
    assert_eq!(&bytes[160..164], b"r\xe9s\0");
    assert_eq!(&bytes[184..200], b"abcdefghijklmno\xe9");
    let back = Erf::read(&bytes).unwrap();
    assert_eq!(back, erf);
    assert_eq!(back.write(), bytes);
}

// F-5: a localized string with more than 64 substrings lost the rest without a word.

#[test]
fn locstring_keeps_every_substring() {
    let strings: Vec<(u32, Vec<u8>)> = (0..100).map(|i| (i, format!("text {}", i).into_bytes())).collect();
    let mut root = Struct::new(u32::MAX);
    root.fields.push(("Name".into(), Value::LocString(LocString { strref: u32::MAX, strings })));
    let gff = Gff { file_type: *b"UTI ", root };
    assert_eq!(Gff::read(&gff.write()).unwrap(), gff);
}

#[test]
fn locstring_count_its_size_cannot_hold_is_refused() {
    // one empty substring, then a Dword64 of 0 whose eight bytes look like a second, empty one
    let mut root = Struct::new(u32::MAX);
    root.fields.push(("Name".into(), Value::LocString(LocString { strref: 7, strings: vec![(0, Vec::new())] })));
    root.fields.push(("Big".into(), Value::Dword64(0)));
    let mut bytes = Gff { file_type: *b"UTI ", root }.write();
    let field_data = u32::from_le_bytes(bytes[32..36].try_into().unwrap()) as usize;
    assert_eq!(&bytes[field_data..field_data + 4], &16u32.to_le_bytes(), "the size: strref, count, one entry");
    bytes[field_data + 8..field_data + 12].copy_from_slice(&2u32.to_le_bytes());
    assert!(Gff::read(&bytes).is_err(), "a second substring cannot fit in 16 bytes");
}

// F-7: Screen.tga.

fn tga_rle(width: u16, height: u16, packets: usize) -> Vec<u8> {
    let mut v = vec![0u8, 0, 10, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    v.extend_from_slice(&width.to_le_bytes());
    v.extend_from_slice(&height.to_le_bytes());
    v.push(24);
    v.push(0x20);
    for i in 0..packets {
        // a run of 128 of one pixel
        v.extend_from_slice(&[0xFF, i as u8, 2, 3]);
    }
    v
}

#[test]
fn tga_rle_claiming_more_than_it_can_hold_sets_nothing_aside() {
    // 8192 by 8192 claimed; 131072 run packets (4 bytes each) give 16.8 million pixels at most
    let data = tga_rle(8192, 8192, 131_072);
    let (got, held) = peak(|| tga::decode(&data));
    assert!(got.is_none());
    assert!(held < 1 << 20, "{} bytes set aside for a {} byte file", held, data.len());
}

#[test]
fn tga_rle_run_past_the_last_pixel_grows_nothing() {
    // 1000 by 1000 is 7812.5 runs of 128: the last runs 64 pixels past the end
    let data = tga_rle(1000, 1000, 7813);
    let (got, held) = peak(|| tga::decode(&data));
    let image = got.expect("a sound picture");
    assert_eq!((image.width, image.height, image.rgba.len()), (1000, 1000, 4_000_000));
    assert_eq!(&image.rgba[0..4], &[3, 2, 0, 255]);
    assert_eq!(&image.rgba[3_999_996..], &[3, 2, (7812 % 256) as u8, 255]);
    // the pixels and the picture, 4 MB each; growing the pixels for the run's extra 64 took 8 MB
    assert!(held <= 8_000_000 + (64 << 10), "{} bytes held", held);
}

// F-8 and W-9: a module name from a save or a .git becomes a path.

#[test]
fn module_names_are_plain_names() {
    for good in ["danm13", "end_m01aa", "001EBO", "STUNT_00", "abcdefghijklmnop"] {
        assert!(kse_core::map::is_module_name(good), "{}", good);
    }
    for bad in ["", "C:x", "C:", "..", "../x", "a/b", "a\\b", "abcdefghijklmnopq", "a b", "a.mod", "\u{e9}t\u{e9}"] {
        assert!(!kse_core::map::is_module_name(bad), "{}", bad);
    }
}

/// On Windows "C:evil" joined to a folder replaces it: C:evil.rim is evil.rim in the current
/// folder of drive C:, outside the game.
#[cfg(windows)]
#[test]
fn module_name_with_a_drive_does_not_leave_the_game() {
    let outside = temp("drive-relative");
    let game = outside.join("game");
    std::fs::create_dir_all(game.join("modules")).unwrap();
    std::fs::write(outside.join("evil.rim"), rim("evil", 2023, 152, 8, b"GIT V3.2")).unwrap();
    let drive = outside.to_string_lossy().chars().next().unwrap();
    let was = std::env::current_dir().unwrap();
    std::env::set_current_dir(&outside).unwrap();
    let files = kse_core::map::ModuleFiles::open(&game, &format!("{}:evil", drive));
    let names = files.names(2023);
    std::env::set_current_dir(was).unwrap();
    assert!(names.is_empty(), "read {:?} from outside the game folder", names);
}

// W-19: a path on a network share kept Windows' \\?\UNC\ prefix, which Explorer cannot open.

#[test]
fn shown_paths_lose_the_verbatim_prefix() {
    assert_eq!(games::strip_verbatim(Path::new(r"\\?\UNC\server\share\Games\swkotor")), PathBuf::from(r"\\server\share\Games\swkotor"));
    assert_eq!(games::strip_verbatim(Path::new(r"\\?\C:\Games\swkotor")), PathBuf::from(r"C:\Games\swkotor"));
    assert_eq!(games::strip_verbatim(Path::new(r"C:\Games\swkotor")), PathBuf::from(r"C:\Games\swkotor"));
    assert_eq!(games::strip_verbatim(Path::new(r"\\server\share")), PathBuf::from(r"\\server\share"));
}

// W-20: Steam's libraryfolders.vdf, both forms.

#[test]
fn steam_library_folders_in_both_forms() {
    let old = "\"LibraryFolders\"\n{\n\t\"TimeNextStatsReport\"\t\t\"1600000000\"\n\t\"ContentStatsID\"\t\t\"-123\"\n\t\"1\"\t\t\"D:\\\\SteamLibrary\"\n\t\"2\"\t\t\"/mnt/games/SteamLibrary\"\n}\n";
    assert_eq!(games::library_paths(old), vec![PathBuf::from(r"D:\SteamLibrary"), PathBuf::from("/mnt/games/SteamLibrary")]);
    let new = "\"libraryfolders\"\n{\n\t\"0\"\n\t{\n\t\t\"path\"\t\t\"C:\\\\Program Files (x86)\\\\Steam\"\n\t\t\"apps\"\n\t\t{\n\t\t\t\"32370\"\t\t\"5374373434\"\n\t\t}\n\t}\n\t\"1\"\n\t{\n\t\t\"path\"\t\t\"E:\\\\SteamLibrary\"\n\t}\n}\n";
    assert_eq!(games::library_paths(new), vec![PathBuf::from(r"C:\Program Files (x86)\Steam"), PathBuf::from(r"E:\SteamLibrary")]);
}

// W-21: the save list.

fn write_nfo(folder: &Path, game: Game) {
    std::fs::create_dir_all(folder).unwrap();
    let mut root = Struct::new(u32::MAX);
    root.fields.push(("SAVEGAMENAME".into(), Value::String(b"Test".to_vec())));
    if game == Game::K2 {
        root.fields.push(("PCNAME".into(), Value::String(b"Exile".to_vec())));
    }
    std::fs::write(folder.join("savenfo.res"), Gff { file_type: *b"NFO ", root }.write()).unwrap();
}

#[test]
fn a_save_of_the_other_game_in_an_install_is_not_listed() {
    let dir = temp("other-game");
    write_nfo(&dir.join("saves").join("000001 - K1"), Game::K1);
    write_nfo(&dir.join("saves").join("000002 - K2"), Game::K2);
    let install = Install { game: Game::K1, path: games::clean(&dir), source: "Test".into() };
    // only this test's folder: the Mac ports' own save folders are always searched too, and
    // on a Mac that has played KotOR they hold real saves (macOS port, 2026-11-08)
    let k1: Vec<_> = games::find_saves(&[install.clone()], Game::K1, &[]).into_iter().filter(|f| f.path.starts_with(&install.path)).collect();
    assert_eq!(k1.len(), 1, "{:?}", k1);
    assert!(k1[0].path.ends_with("000001 - K1"));
}

#[test]
fn an_added_save_spelled_differently_is_listed_once() {
    let dir = temp("spelled");
    let save = dir.join("saves").join("000001 - K1");
    write_nfo(&save, Game::K1);
    let install = Install { game: Game::K1, path: games::clean(&dir), source: "Test".into() };
    let text = games::clean(&save).to_string_lossy().to_string();
    let parent = games::clean(&dir.join("saves")).to_string_lossy().to_string();
    let mut spellings = vec![format!("{}{}", text, std::path::MAIN_SEPARATOR), format!("{}{}", parent, std::path::MAIN_SEPARATOR)];
    if cfg!(windows) {
        spellings.push(text.to_uppercase());
        spellings.push(text.replace('\\', "/"));
        spellings.push(format!(r"\\?\{}", text));
        spellings.push(parent.to_lowercase());
    }
    for s in spellings {
        let found: Vec<_> = games::find_saves(&[install.clone()], Game::K1, &[PathBuf::from(&s)]).into_iter().filter(|f| games::clean(&f.path).starts_with(&install.path)).collect();
        assert_eq!(found.len(), 1, "{} listed it {} times", s, found.len());
        assert_eq!(found[0].kind, SaveKind::Local, "{}", s);
    }
}
