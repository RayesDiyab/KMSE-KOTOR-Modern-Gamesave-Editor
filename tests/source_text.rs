//! The program's own text is free of encoding damage: a middle dot once went through the wrong
//! code page and reached the window as an Arabic letter before it ("0 \u{622}\u{b7} Dark"),
//! which also turned the label round (2026-11-06). This reads the sources that hold the text
//! the window shows and fails on the marks such damage leaves.

use std::path::Path;

fn files(dir: &Path, ext: &str, out: &mut Vec<std::path::PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap().filter_map(|e| e.ok()) {
        let p = e.path();
        if p.is_dir() {
            files(&p, ext, out);
        } else if p.extension().map_or(false, |x| x == ext) {
            out.push(p);
        }
    }
}

#[test]
fn the_windows_text_has_no_encoding_damage() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut all = Vec::new();
    files(&root.join("ui"), "slint", &mut all);
    files(&root.join("src"), "rs", &mut all);
    assert!(!all.is_empty());
    for f in all {
        let text = std::fs::read_to_string(&f).unwrap();
        for (n, line) in text.lines().enumerate() {
            // Arabic letters (what UTF-8 read as Windows-1256 leaves), and the marks of UTF-8
            // read as Windows-1252
            let bad = line.chars().any(|c| ('\u{600}'..='\u{6ff}').contains(&c)) || ["\u{e2}\u{20ac}", "\u{c3}\u{a9}", "\u{c2}\u{b7}", "\u{fffd}"].iter().any(|m| line.contains(m));
            assert!(!bad, "{}:{}: {}", f.display(), n + 1, line.trim());
        }
    }
}
