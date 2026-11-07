//! The readers against damaged and made-up files (2026-11-07 audit). A save folder is
//! untrusted: the save list reads every savenfo.res it finds, and opening reads the archive.
//!
//! - made-up files that ask for gigabytes (one large field named by many fields, archive
//!   entries that each cover the whole archive) are refused, quickly, without the memory;
//! - every GFF and archive of a save on this PC, cut short and with random bytes changed many
//!   thousand times, either reads or is refused: never a panic (the release build aborts on
//!   one). Without saves on the PC only the made-up files are checked.

use kse_core::formats::erf::Erf;
use kse_core::formats::gff::Gff;
use kse_core::games::{find_installs, find_saves, Game};
use std::time::Instant;

fn le(v: &mut Vec<u8>, n: u32) {
    v.extend_from_slice(&n.to_le_bytes());
}

/// A GFF whose root names field 0 `fields` times, field 0 a VOID of `size` bytes.
fn gff_field_bomb(fields: u32, size: u32) -> Vec<u8> {
    let structs = 56u32;
    let field = structs + 12;
    let label = field + 12;
    let data = label + 16;
    let indices = data + 4 + size;
    let lists = indices + fields * 4;
    let mut v = Vec::new();
    v.extend_from_slice(b"GFF V3.2");
    for (o, c) in [(structs, 1), (field, 1), (label, 1), (data, 4 + size), (indices, fields * 4), (lists, 0)] {
        le(&mut v, o);
        le(&mut v, c);
    }
    le(&mut v, 0xFFFF_FFFF);
    le(&mut v, 0);
    le(&mut v, fields);
    le(&mut v, 13);
    le(&mut v, 0);
    le(&mut v, 0);
    let mut l = [0u8; 16];
    l[0] = b'A';
    v.extend_from_slice(&l);
    le(&mut v, size);
    v.resize(v.len() + size as usize, 0);
    v.resize(v.len() + fields as usize * 4, 0);
    v
}

/// An archive of `count` entries, each covering the whole archive.
fn erf_overlap_bomb(count: u32) -> Vec<u8> {
    let keys = 160u32;
    let list = keys + count * 24;
    let total = list + count * 8;
    let mut v = Vec::new();
    v.extend_from_slice(b"SAV V1.0");
    for n in [0, 0, count, 0, keys, list, 126, 1, 0] {
        le(&mut v, n);
    }
    v.resize(160, 0);
    for i in 0..count {
        let mut k = [0u8; 24];
        let name = format!("r{}", i);
        k[..name.len()].copy_from_slice(name.as_bytes());
        k[16..20].copy_from_slice(&i.to_le_bytes());
        k[20..22].copy_from_slice(&2017u16.to_le_bytes());
        v.extend_from_slice(&k);
    }
    for _ in 0..count {
        le(&mut v, 0);
        le(&mut v, total);
    }
    v
}

#[test]
fn made_up_files_asking_for_gigabytes_are_refused() {
    // 512 KB naming a 256 KB field 65,536 times: 16 GB if copied each time
    let g = gff_field_bomb(65_536, 256 * 1024);
    let t = Instant::now();
    assert!(Gff::read(&g).is_err(), "the field bomb is refused");
    assert!(t.elapsed().as_secs() < 5, "and quickly ({:?})", t.elapsed());
    // the same shape at a size a real file has reads: one field named once
    assert!(Gff::read(&gff_field_bomb(1, 1024)).is_ok(), "a sound file of the same shape reads");
    // 640 KB of entries each covering the archive: 12.8 GB if copied each time
    let e = erf_overlap_bomb(20_000);
    let t = Instant::now();
    assert!(Erf::read(&e).is_err(), "the overlapping archive is refused");
    assert!(t.elapsed().as_secs() < 5, "and quickly ({:?})", t.elapsed());
}

/// A small, repeatable random source (no crate needed).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

fn mutate_all(name: &str, data: &[u8], rounds: usize, rng: &mut Rng, read: &dyn Fn(&[u8])) {
    for r in 0..rounds {
        let mut d = data.to_vec();
        match r % 4 {
            0 => d.truncate(rng.below(d.len())),
            1 => {
                for _ in 0..1 + rng.below(8) {
                    let i = rng.below(d.len());
                    d[i] = rng.next() as u8;
                }
            }
            2 => {
                // a header number (offsets and counts sit in the first 160 bytes)
                let i = rng.below(d.len().min(160) / 4) * 4;
                let n = [0u32, 1, 0x7FFF_FFFF, 0xFFFF_FFFF, rng.next() as u32][rng.below(5)];
                if i + 4 <= d.len() {
                    d[i..i + 4].copy_from_slice(&n.to_le_bytes());
                }
            }
            _ => {
                let i = rng.below(d.len());
                let n = rng.below(64);
                let end = (i + n).min(d.len());
                d.drain(i..end);
            }
        }
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| read(&d)));
        assert!(caught.is_ok(), "{}: round {} panicked", name, r);
    }
}

#[test]
fn damaged_save_files_never_panic() {
    let installs = find_installs(&[]);
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut files = 0;
    for game in [Game::K1, Game::K2] {
        for s in find_saves(&installs, game, &[]).into_iter().take(2) {
            for e in std::fs::read_dir(&s.path).unwrap().filter_map(|e| e.ok()) {
                let name = e.file_name().to_string_lossy().to_ascii_lowercase();
                let Ok(data) = std::fs::read(e.path()) else { continue };
                if data.is_empty() {
                    continue;
                }
                if name.ends_with(".sav") {
                    mutate_all(&name, &data, 1500, &mut rng, &|d| {
                        if let Ok(a) = Erf::read(d) {
                            for r in a.resources.iter().take(8) {
                                let _ = Gff::read(&r.data);
                            }
                            let _ = a.write();
                        }
                    });
                    files += 1;
                } else if name.ends_with(".res") {
                    mutate_all(&name, &data, 3000, &mut rng, &|d| {
                        if let Ok(g) = Gff::read(d) {
                            let _ = g.write();
                        }
                    });
                    files += 1;
                }
            }
        }
    }
    eprintln!("{} save files damaged and read", files);
}
