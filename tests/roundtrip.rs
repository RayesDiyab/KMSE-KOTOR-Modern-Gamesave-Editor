//! Round trips of every save on this PC, in memory only (the saves are read, never written):
//! every GFF file of the save folder, SAVEGAME.sav, every resource inside it, and every area
//! archive nested in it with its resources, read and written back.
//!
//! - written back and read again, each is the same structure as read (no field, type, value,
//!   struct id or order lost): asserted for every file;
//! - written twice, the bytes do not change (the writers are deterministic);
//! - archives (SAVEGAME.sav and the area saves in it) are written back byte for byte, apart
//!   from one header offset nothing reads; GFF files keep their structure but number their
//!   structs in another order than the game does (measured 2026-11-07, see the assert).
//!
//! The PC's saves are the fixtures: KotOR 1 and KotOR 2, early to late, the maintainer's own
//! and the editor's earlier output. On a PC without the games there are none and the test
//! passes without checking anything; it says so.

use kse_core::formats::erf::{Erf, RES_TYPE_SAV};
use kse_core::formats::gff::Gff;
use kse_core::games::{find_installs, find_saves, Game};
use std::fs;
use std::path::Path;

#[derive(Default)]
struct Tally {
    files: usize,
    byte_same: usize,
    differ: Vec<String>,
    erf_differ: Vec<String>,
    unreadable: Vec<String>,
}

fn gff(t: &mut Tally, what: &str, data: &[u8]) {
    // a GFF by its signature; anything else (a TGA, a 2DA) is not a GFF
    if data.len() < 8 || &data[4..8] != b"V3.2" {
        return;
    }
    let Ok(g) = Gff::read(data) else {
        t.unreadable.push(what.to_string());
        return;
    };
    t.files += 1;
    let out = g.write();
    let again = Gff::read(&out).unwrap_or_else(|e| panic!("{}: written back, it does not read: {}", what, e.0));
    assert_eq!(again, g, "{}: written back, it reads as a different structure", what);
    assert_eq!(again.write(), out, "{}: written twice, the bytes change", what);
    if out == data {
        t.byte_same += 1;
    } else {
        if let Ok(dir) = std::env::var("KMSE_DUMP") {
            if t.differ.is_empty() {
                let _ = fs::write(Path::new(&dir).join("orig.gff"), data);
                let _ = fs::write(Path::new(&dir).join("out.gff"), &out);
            }
        }
        t.differ.push(format!("{} ({} -> {} bytes)", what, data.len(), out.len()));
    }
}

/// An archive with the unused header field set to 0, in it and in every area archive inside
/// it: the offset of the description strings (bytes 20..24) when there are none (the language
/// count at 8..12 is 0). Header: 8 language count, 12 strings size, 16 entry count, 20 strings
/// offset, 24 keys offset, 28 resources offset; a key is 24 bytes with its type at 20, a
/// resource entry 8 bytes (offset, size).
fn unused_offset_cleared(data: &[u8]) -> Vec<u8> {
    let mut b = data.to_vec();
    clear_at(&mut b, 0, data.len(), 0);
    b
}

fn clear_at(b: &mut [u8], at: usize, len: usize, depth: usize) {
    let u32_at = |b: &[u8], o: usize| -> Option<usize> { b.get(o..o + 4).map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]]) as usize) };
    if len < 160 || depth > 1 {
        return;
    }
    if u32_at(b, at + 8) == Some(0) {
        b[at + 20..at + 24].fill(0);
    }
    let (Some(entries), Some(keys), Some(res)) = (u32_at(b, at + 16), u32_at(b, at + 24), u32_at(b, at + 28)) else { return };
    for i in 0..entries {
        let kind = b.get(at + keys + i * 24 + 20..at + keys + i * 24 + 22).map(|x| u16::from_le_bytes([x[0], x[1]]));
        let (Some(off), Some(size)) = (u32_at(b, at + res + i * 8), u32_at(b, at + res + i * 8 + 4)) else { return };
        if kind == Some(RES_TYPE_SAV) && at + off + size <= b.len() {
            clear_at(b, at + off, size, depth + 1);
        }
    }
}

fn erf(t: &mut Tally, what: &str, data: &[u8], depth: usize) {
    let Ok(e) = Erf::read(data) else {
        t.unreadable.push(what.to_string());
        return;
    };
    t.files += 1;
    let out = e.write();
    let again = Erf::read(&out).unwrap_or_else(|x| panic!("{}: written back, it does not read: {}", what, x.0));
    assert_eq!(again, e, "{}: written back, it reads as a different archive", what);
    assert_eq!(again.write(), out, "{}: written twice, the bytes change", what);
    // The one header field allowed to differ: the offset of the description strings when
    // there are none (bytes 20..24, with a language count of 0 at 8..12). Some writers leave
    // it 0; the editor writes where the strings would start. Nothing reads it.
    if out == data {
        t.byte_same += 1;
    } else if unused_offset_cleared(&out) != unused_offset_cleared(data) {
        t.erf_differ.push(format!("{} ({} -> {} bytes)", what, data.len(), out.len()));
    }
    for r in &e.resources {
        let inner = format!("{}/{}.{}", what, r.name, r.kind);
        if r.kind == RES_TYPE_SAV && depth == 0 {
            erf(t, &inner, &r.data, depth + 1);
        } else {
            gff(t, &inner, &r.data);
        }
    }
}

fn folder(t: &mut Tally, dir: &Path) {
    for e in fs::read_dir(dir).unwrap().filter_map(|e| e.ok()) {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        let what = format!("{}/{}", dir.file_name().unwrap().to_string_lossy(), name);
        let Ok(data) = fs::read(&p) else { continue };
        if name.to_ascii_lowercase().ends_with(".sav") {
            erf(t, &what, &data, 0);
        } else {
            gff(t, &what, &data);
        }
    }
}

#[test]
fn every_save_on_this_pc_round_trips() {
    let installs = find_installs(&[]);
    let mut saves = 0;
    let mut all = Tally::default();
    for game in [Game::K1, Game::K2] {
        let mut t = Tally::default();
        let list = find_saves(&installs, game, &[]);
        for s in &list {
            folder(&mut t, &s.path);
        }
        eprintln!(
            "{:?}: {} saves, {} files read and written back: {} byte for byte the same, {} GFF and {} archives laid out differently, {} not readable",
            game,
            list.len(),
            t.files,
            t.byte_same,
            t.differ.len(),
            t.erf_differ.len(),
            t.unreadable.len()
        );
        for d in t.erf_differ.iter().take(15) {
            eprintln!("  archive differs: {}", d);
        }
        for d in t.unreadable.iter().take(15) {
            eprintln!("  unreadable: {}", d);
        }
        saves += list.len();
        all.files += t.files;
        all.byte_same += t.byte_same;
        all.differ.extend(t.differ);
        all.erf_differ.extend(t.erf_differ);
        all.unreadable.extend(t.unreadable);
    }
    if saves == 0 {
        eprintln!("no saves on this PC: nothing checked");
        return;
    }
    // Archives: byte for byte. GFF files: the same structure (asserted above for each); their
    // bytes differ only in the order the structs are numbered in, measured on 2026-11-07
    // (every section the same size and count), which the game reads through offsets.
    // and no real file is refused (the readers' limits against made-up files hold for real ones)
    assert!(all.unreadable.is_empty(), "{} real files do not read: {:?}", all.unreadable.len(), all.unreadable.iter().take(5).collect::<Vec<_>>());
    assert!(all.erf_differ.is_empty(), "{} archives are not written back byte for byte: {:?}", all.erf_differ.len(), all.erf_differ.iter().take(5).collect::<Vec<_>>());
}
