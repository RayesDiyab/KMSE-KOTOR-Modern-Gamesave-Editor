# Third-party notices

> **Documentation standard.** This document follows
> [`docs/documentation-standard.md`](docs/documentation-standard.md). Read it before
> editing this file, and check the result still meets it: measured claims only, rejected
> alternatives and corrections kept visible, and anything untested labelled as untested.

What the KOTOR Modern Gamesave Editor (KMSE) commits to this repository or carries in the
program, where each part comes from, and its licence. Checked against the repository on
2026-10-24. KMSE itself is licensed under the GNU General Public License, version 3 or
later ([LICENSE](LICENSE); `license = "GPL-3.0-or-later"` in `Cargo.toml`).

## The games' own material

*Star Wars: Knights of the Old Republic* and *Star Wars Knights of the Old Republic II:
The Sith Lords*, their names, characters, art and text belong to Lucasfilm and Disney, and
to BioWare and Obsidian Entertainment, who made them. KMSE is an unofficial tool, not
affiliated with or endorsed by any of them. It uses what it learns of the games' files for
interoperability: to read and write the saves the games make.

**No file taken from either game is committed or carried in the program.** What the editor
shows of the games (names, journal text, portraits, icons, the map pictures, the planets)
it reads from the player's own install when it runs. On a PC without the game it shows
what the save holds, by number and tag.

| Made from the player's game | When | How |
| --- | --- | --- |
| Names, journal text, portraits, icons, area maps, planet globes, Pazaak cards | at run time | read from the install's archives, tables and `dialog.tlk`, with Override first; never written anywhere |
| The timeline's picture of Yavin Station | at build time, on the build machine | `build.rs` takes frame 484 of KotOR's movie `Live_1c.bik` with ffmpeg and cuts it out with the committed outline `assets/timeline/yavin-station-mask.png`. The picture goes into the build folder and the program, never into the repository. Without the game or ffmpeg the program has no picture and the button shows the name alone |

A build made with the game present therefore carries one frame of the game's movie,
reduced and cut out. A build made without it (GitHub's runners have no KotOR install)
carries none.

**What is committed that describes the games:**

| Committed | What it holds | What it does not hold |
| --- | --- | --- |
| `data/quests/*.json` (quest chains, milestones, rewards), `data/xp/*.json` (enemies and XP scenarios) | identifiers from the games' files (quest tags, stage numbers, module, dialog, script, item, creature and global variable names, speaker tags), numbers (XP, credits, challenge ratings, node numbers, positions), short expressions decompiled from the games' compiled scripts (`AddPartyMember(...)`, conditions on globals), and the maintainer's own notes and chapter titles. Made by `tools/questdb` and `tools/xp` from the game's files | the games' journal text, dialogue or descriptions |
| `data/quests/*-review.md`, `data/quests/k1-overview.md` | the same identifiers as readable pages | the same: each page says "IDs only". `tools/questdb/build.py --review-text` writes a copy with the game's own names and journal text to `data/quests/local/`, which `.gitignore` keeps out |
| `src/catalog.rs` | for a save opened without the game: the names of the eight skills, the classes and the companions of each game (for example "Awareness", "Jedi Guardian", "Bastila"), and the XP formula | any other text of the games |
| `docs/save-editor-format.md` and `docs/save-editor.md` | the save format, field names, table and resource names, and engine addresses, from saves, the game's files, the public KotOR 1 decompilation [swkotor1-reconstruction](https://github.com/mattlgroff/swkotor1-reconstruction), disassemblies made for the documents, and the maintainer's own private reverse engineering | game files or the games' text, beyond a few short labels quoted to say which screen is meant |

The program's own words never quote the games beyond those names. KSE (KotOR Savegame
Editor) was used only as a list of what players expect; no code of KSE or its forks is in
KMSE (`docs/save-editor.md`, provenance).

## Fonts

### KMRP Original HD fonts

`fonts/KMRP-OriginalHD-Menu.otf`, `-MenuMedium.otf`, `-MenuSmallB.otf` and
`-Descriptions.otf`, embedded in the program by `build.rs`. Copyright 2026 RaymanGT (the
maintainer), from KMRP's `fonts/otf/` (the four files have the same SHA-256 as KMRP's on
2026-10-24). Licensed under the **SIL Open Font License 1.1**, copied with them in
[fonts/OFL.txt](fonts/OFL.txt), the same file as KMRP's. How the fonts were drawn is in
KMRP's
[fonts/README.md](https://github.com/RayesDiyab/KMRP-KOTOR-Modern-Restoration-Patch/blob/master/fonts/README.md).
The OFL lets them be bundled with software under another licence; they stay under the OFL.

### Aurebesh AF

`assets/icon/AurebeshAF-CanonTech.otf`, **Aurebesh AF** by AurekFonts, CC0 (public
domain); its name table says "CC-0 public domain, created by AurekFonts". It is a
build-time input of the icon only (`assets/icon/datapad.py` draws the datapad's screen
with it). The font is committed but not carried in the program.

## Pictures

| File | What | Source and licence |
| --- | --- | --- |
| `assets/timeline/ebon-hawk.png` | the Ebon Hawk on the timeline | a render from Deadly Stream, free to use (the maintainer, 2026-10-24, in `assets/timeline/README.md`). **The author's name and the page it came from are not recorded in this repository.** Cut from its background with rembg's BiRefNet model, with the engines' glow put back from the original |
| `assets/timeline/stars.png` | the starfield behind the timeline | generated (about 2,600 stars and two soft glows) |
| `assets/timeline/yavin-station-mask.png` | the outline of Yavin Station, a grey-scale mask | made once with rembg's BiRefNet model from a frame of the game's movie; an outline only, not the picture (see the games' own material above) |
| `assets/sigil.png` | the KMRP sigil | the maintainer's own artwork, from KMRP (`assets/branding/sigil.png`, the same file) |
| `assets/kmse-logo.png` | the crest in the header: the sigil over "KMSE" | the maintainer's own artwork, made the way the KMRP crest is (2026-10-24) |
| `assets/settings.png` | the settings gear | from the KMRP installer's gear (KMRP `assets/branding/ui-icons/Settings.png`), the maintainer's own |
| `assets/icon/icon.png`, `assets/save-editor-icon.png`, `assets/save-editor.ico` | the program's icon | the maintainer's: a holocron with a datapad drawn from geometry in front of it (`assets/icon/README.md`) |
| `assets/icon/holocron-clean.png` | the holocron the icon is made from | the maintainer's picture, its white fringe removed (`clean_holocron.py`). How the original picture was made is not recorded in this repository |

## Rust crates

The program is built from the crates listed below, as resolved in `Cargo.lock` on
2026-10-24. The list is the normal (not build or test) dependencies for Windows
(`x86_64-pc-windows-msvc`), Linux (`x86_64-unknown-linux-gnu`) and macOS (`aarch64-` and
`x86_64-apple-darwin`), from

```sh
cargo tree -e normal --prefix none --format "{p}|{l}" --target <target>
```

with the licence each crate declares in its manifest, grouped by licence. A crate marked
(Windows), (Linux) or (macOS) is used on those systems only; the rest on all three. Some
are procedural macros that run only while the program compiles. The full licence texts are
in each crate's source, which Cargo downloads from crates.io.

**Slint**, the interface toolkit, is offered under GPL-3.0-only, the Slint Royalty-free
licence 2.0 or the Slint Software licence 3.0. KMSE uses it under the GPL-3.0, so the
program as built is under the GPL-3.0.

Build-time only, not in the program: `slint-build` (the same licence choice as Slint),
`miniz_oxide` (also used at run time) and, on Windows, `winresource` (MIT), with their own
dependencies.

372 crate versions in all (a few crates are used in two versions).

### Apache-2.0 OR MIT (212)

`ahash` 0.8.12 (Linux), `allocator-api2` 0.2.21, `annotate-snippets` 0.12.17, `anstyle` 1.0.14, `arboard` 3.7.0, `arrayvec` 0.7.8, `as-raw-xcb-connection` 1.0.1 (Linux), `async-broadcast` 0.7.2 (Linux), `async-channel` 2.5.0 (Linux), `async-executor` 1.14.0 (Linux), `async-io` 2.6.0 (Linux), `async-lock` 3.4.2 (Linux), `async-process` 2.5.0 (Linux), `async-recursion` 1.2.0 (Linux), `async-signal` 0.2.14 (Linux), `async-task` 4.7.1 (Linux), `async-trait` 0.1.92 (Linux), `atomic-waker` 1.1.2 (Linux), `base64` 0.23.1, `bitflags` 1.3.2 (macOS), `bitflags` 2.13.2, `blocking` 1.7.0 (Linux), `by_address` 1.2.1, `cfg-if` 1.0.5, `chrono` 0.4.45, `concurrent-queue` 2.5.0 (Linux), `const-field-offset` 0.2.1, `const-field-offset-macro` 0.2.1, `core-foundation` 0.9.4 (macOS), `core-foundation-sys` 0.8.7 (macOS), `core-graphics` 0.23.2 (macOS), `core-graphics-types` 0.1.3 (macOS), `countme` 3.0.1, `crc32fast` 1.5.3, `critical-section` 1.2.0, `crossbeam-channel` 0.5.17 (Windows, macOS), `crossbeam-deque` 0.8.8, `crossbeam-epoch` 0.9.21, `crossbeam-utils` 0.8.23, `ctor` 0.10.1 (Linux), `data-url` 0.3.2, `displaydoc` 0.2.7, `downcast-rs` 1.2.1 (Linux), `either` 1.19.0, `enumflags2` 0.7.12 (Linux), `enumflags2_derive` 0.7.12 (Linux), `equivalent` 1.0.3, `errno` 0.3.14 (Linux), `euclid` 0.22.14, `event-listener` 5.4.2 (Linux), `event-listener-strategy` 0.5.4 (Linux), `fastrand` 2.5.0 (Linux), `fdeflate` 0.3.7, `field-offset` 0.3.6, `fixedbitset` 0.5.7 (Linux), `flate2` 1.1.10, `font-types` 0.12.7, `fontique` 0.11.1, `foreign-types` 0.5.0 (macOS), `foreign-types-macros` 0.2.4 (macOS), `foreign-types-shared` 0.3.1 (macOS), `form_urlencoded` 1.2.2, `futures` 0.3.34 (Linux), `futures-channel` 0.3.34 (Linux), `futures-core` 0.3.34 (Linux), `futures-executor` 0.3.34 (Linux), `futures-io` 0.3.34 (Linux), `futures-lite` 2.6.1 (Linux), `futures-macro` 0.3.34 (Linux), `futures-sink` 0.3.34 (Linux), `futures-task` 0.3.34 (Linux), `futures-util` 0.3.34 (Linux), `getrandom` 0.3.4 (Linux), `gif` 0.14.2, `hashbrown` 0.14.5, `hashbrown` 0.15.5 (Linux), `hashbrown` 0.16.1, `hashbrown` 0.17.1, `heck` 0.5.0, `hex` 0.4.3 (Linux), `htmlparser` 0.2.1, `iana-time-zone` 0.1.65 (Linux, macOS), `idna` 1.1.0, `idna_adapter` 1.2.2, `image` 0.25.10, `image-webp` 0.2.4, `indexmap` 2.14.2, `itertools` 0.15.0, `itoa` 1.0.18, `keyboard-types` 0.7.0 (Windows, macOS), `kurbo` 0.13.1, `lazy_static` 1.5.1, `libc` 0.2.191 (Linux, macOS), `linebender_resource_handle` 0.1.1, `lock_api` 0.4.14 (Linux), `log` 0.4.34, `lyon_algorithms` 1.0.22, `lyon_extra` 1.1.0, `lyon_geom` 1.0.20, `lyon_path` 1.0.19, `memmap2` 0.9.12, `muda` 0.19.3 (Windows, macOS), `num-traits` 0.2.19, `once_cell` 1.21.4, `ordered-stream` 0.2.0 (Linux), `parking` 2.2.1 (Linux), `parking_lot` 0.12.5 (Linux), `parking_lot_core` 0.9.12 (Linux), `parlance` 0.1.1, `parley` 0.11.1, `parley_data` 0.11.1, `percent-encoding` 2.3.2, `petgraph` 0.8.3 (Linux), `pin-project` 1.1.13, `pin-project-internal` 1.1.13, `pin-project-lite` 0.2.17, `pin-utils` 0.1.1, `piper` 0.2.5 (Linux), `png` 0.18.1, `polling` 3.11.0 (Linux), `pollster` 0.4.0 (Linux), `polycool` 0.4.0, `portable-atomic` 1.15.0, `ppv-lite86` 0.2.21 (Linux), `proc-macro-crate` 3.5.0, `proc-macro2` 1.0.107, `quick-error` 2.0.1, `quote` 1.0.47, `rand` 0.9.5 (Linux), `rand_chacha` 0.9.0 (Linux), `rand_core` 0.9.5 (Linux), `rayon` 1.12.0, `rayon-core` 1.13.0, `read-fonts` 0.41.0, `resvg` 0.48.1, `rowan` 0.17.0, `roxmltree` 0.21.1, `rustc-hash` 1.1.0, `rustversion` 1.0.23, `scoped-tls` 1.0.1 (Linux), `scoped-tls-hkt` 0.1.5, `scopeguard` 1.2.0, `serde` 1.0.229, `serde_core` 1.0.229, `serde_derive` 1.0.229, `serde_json` 1.0.154, `serde_repr` 0.1.21 (Linux), `signal-hook-registry` 1.4.8 (Linux), `simplecss` 0.2.2, `siphasher` 1.0.4, `skrifa` 0.44.0, `smallvec` 1.16.3, `smol_str` 0.2.2, `smol_str` 0.3.7, `snafu` 0.8.9, `snafu-derive` 0.8.9, `socket2` 0.6.5 (Linux), `softbuffer` 0.4.8, `spin_on` 0.1.1, `stable_deref_trait` 1.2.1, `svgtypes` 0.16.1, `swash` 0.2.10, `syn` 2.0.119, `syn` 3.0.9, `sys-locale` 0.3.2, `text-size` 1.1.1, `thiserror` 1.0.69 (Linux), `thiserror` 2.0.21, `thiserror-impl` 1.0.69 (Linux), `thiserror-impl` 2.0.21, `toml_datetime` 1.1.2+spec-1.1.0, `toml_edit` 0.25.17+spec-1.1.0, `toml_parser` 1.1.5+spec-1.1.0, `toml_writer` 1.1.3+spec-1.1.0, `ttf-parser` 0.25.1 (Linux), `typed-index-collections` 3.5.0, `unicase` 2.10.0, `unicode-bidi` 0.3.18, `unicode-script` 0.5.8, `unicode-segmentation` 1.13.3, `unicode-vo` 0.1.0, `unicode-width` 0.2.2, `unicode-xid` 0.2.6, `url` 2.5.8, `usvg` 0.48.1, `utf8_iter` 1.0.4, `uuid` 1.28.0 (Linux), `vtable` 0.5.0, `vtable-macro` 0.5.0, `webbrowser` 1.2.4, `weezl` 0.1.12, `windows` 0.62.2, `windows-collections` 0.3.2, `windows-core` 0.62.2, `windows-future` 0.3.2, `windows-implement` 0.60.2, `windows-interface` 0.59.3, `windows-link` 0.2.1, `windows-numerics` 0.3.1, `windows-result` 0.4.1, `windows-strings` 0.5.1, `windows-sys` 0.52.0 (Windows), `windows-sys` 0.59.0 (Windows), `windows-sys` 0.61.2 (Windows), `windows-targets` 0.52.6 (Windows), `windows-threading` 0.2.1, `windows_x86_64_msvc` 0.52.6 (Windows), `wl-clipboard-rs` 0.9.4 (Linux), `x11rb` 0.13.2 (Linux), `x11rb-protocol` 0.13.2 (Linux), `yazi` 0.2.1, `zeno` 0.3.3.

### MIT (77)

`ashpd` 0.11.1 (Linux), `block2` 0.5.1 (macOS), `block2` 0.6.2 (macOS), `bytes` 1.12.1 (Linux), `calloop` 0.13.0 (Linux), `calloop-wayland-source` 0.3.0 (Linux), `clru` 0.6.3, `color_quant` 1.1.0, `convert_case` 0.10.0, `derive_more` 2.1.1, `derive_more-impl` 2.1.1, `dispatch` 0.2.0 (macOS), `dlib` 0.5.3 (Linux), `endi` 1.1.1 (Linux), `float-cmp` 0.9.0, `fontdb` 0.24.0, `harfrust` 0.12.0, `imagesize` 0.15.0, `libm` 0.2.16, `memoffset` 0.9.1, `mio` 1.2.4 (Linux), `natord` 1.0.9, `nom` 8.0.0 (Linux), `objc-sys` 0.3.5 (macOS), `objc2` 0.5.3 (macOS), `objc2` 0.6.5 (macOS), `objc2-app-kit` 0.2.2 (macOS), `objc2-encode` 4.1.0 (macOS), `objc2-foundation` 0.2.2 (macOS), `objc2-foundation` 0.3.2 (macOS), `os_pipe` 1.2.3 (Linux), `pico-args` 0.5.0, `pin-weak` 1.1.0, `pulldown-cmark` 0.13.5, `quick-xml` 0.41.0 (Linux), `rfd` 0.15.4, `rgb` 0.8.53, `rspolib` 0.1.2, `sctk-adwaita` 0.10.1 (Linux), `simd-adler32` 0.3.10, `slab` 0.4.12, `smithay-client-toolkit` 0.19.2 (Linux), `strict-num` 0.1.1, `strum` 0.28.0, `strum_macros` 0.28.0, `synstructure` 0.14.0, `taffy` 0.10.1, `tokio` 1.54.1 (Linux), `tracing` 0.1.44, `tracing-attributes` 0.1.31 (Linux), `tracing-core` 0.1.36, `tree_magic_mini` 3.2.2 (Linux), `urlencoding` 2.1.3 (Linux), `wayland-backend` 0.3.17 (Linux), `wayland-client` 0.31.15 (Linux), `wayland-csd-frame` 0.3.0 (Linux), `wayland-cursor` 0.31.14 (Linux), `wayland-protocols` 0.32.13 (Linux), `wayland-protocols-plasma` 0.3.12 (Linux), `wayland-protocols-wlr` 0.3.12 (Linux), `wayland-scanner` 0.31.11 (Linux), `wayland-sys` 0.31.11 (Linux), `winnow` 1.0.4, `winreg` 0.56.0 (Windows), `x11-dl` 2.21.0 (Linux), `xcursor` 0.3.11 (Linux), `xkbcommon-dl` 0.4.2 (Linux), `xmlwriter` 0.1.0, `yeslogic-fontconfig-sys` 6.0.1 (Linux), `zbus` 5.19.0 (Linux), `zbus_macros` 5.19.0 (Linux), `zbus_names` 4.3.5 (Linux), `zcheapstr` 1.1.0 (Linux), `zmij` 1.0.23, `zvariant` 5.15.1 (Linux), `zvariant_derive` 5.15.1 (Linux), `zvariant_utils` 4.2.0 (Linux).

### Unicode-3.0 (25)

`fixed_decimal` 0.7.2, `icu_collections` 2.3.0, `icu_decimal` 2.3.0, `icu_decimal_data` 2.3.0, `icu_locale_core` 2.3.0, `icu_locale_fallback` 2.3.0, `icu_locale_fallback_data` 2.3.0, `icu_normalizer` 2.3.0, `icu_normalizer_data` 2.3.0, `icu_properties` 2.3.0, `icu_properties_data` 2.3.0, `icu_provider` 2.3.1, `icu_segmenter` 2.3.0, `icu_segmenter_data` 2.3.0, `litemap` 0.8.3, `potential_utf` 0.1.6, `tinystr` 0.8.4, `writeable` 0.6.4, `yoke` 0.8.3, `yoke-derive` 0.8.4, `zerofrom` 0.1.8, `zerofrom-derive` 0.1.8, `zerotrie` 0.2.5, `zerovec` 0.11.8, `zerovec-derive` 0.11.6.

### Apache-2.0 OR MIT OR Zlib (17)

`bytemuck` 1.25.2, `bytemuck_derive` 1.12.1, `cursor-icon` 1.2.0, `dispatch2` 0.3.1 (macOS), `miniz_oxide` 0.8.9, `miniz_oxide` 0.9.1, `objc2-app-kit` 0.3.2 (macOS), `objc2-core-foundation` 0.3.2 (macOS), `objc2-core-graphics` 0.3.2 (macOS), `objc2-core-text` 0.3.2 (macOS), `objc2-quartz-core` 0.3.2 (macOS), `raw-window-handle` 0.6.2, `tiny-xlib` 0.2.5 (Linux), `tinyvec` 1.13.3, `xkeysym` 0.2.1 (Linux), `zune-core` 0.5.3, `zune-jpeg` 0.5.15.

### GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0 (9)

`i-slint-backend-selector` 1.18.1, `i-slint-backend-winit` 1.18.1, `i-slint-common` 1.18.1, `i-slint-compiler` 1.18.1, `i-slint-core` 1.18.1, `i-slint-core-macros` 1.18.1, `i-slint-renderer-software` 1.18.1, `slint` 1.18.1, `slint-macros` 1.18.1.

### Apache-2.0 (6)

`ab_glyph` 0.2.32 (Linux), `ab_glyph_rasterizer` 0.1.10 (Linux), `gethostname` 1.1.0 (Linux), `owned_ttf_parser` 0.25.1 (Linux), `unicode-linebreak` 0.1.5, `winit` 0.30.13.

### Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT (4)

`linux-raw-sys` 0.12.1 (Linux), `linux-raw-sys` 0.4.15 (Linux), `rustix` 0.38.44 (Linux), `rustix` 1.1.5 (Linux).

### BSD-3-Clause (4)

`tiny-skia` 0.11.4 (Linux), `tiny-skia` 0.12.0, `tiny-skia-path` 0.11.4 (Linux), `tiny-skia-path` 0.12.0.

### Zlib (3)

`foldhash` 0.1.5 (Linux), `foldhash` 0.2.0, `slotmap` 1.1.1.

### Apache-2.0 OR BSD-3-Clause (2)

`moxcms` 0.8.1, `pxfm` 0.1.30.

### Apache-2.0 OR BSD-3-Clause OR MIT (2)

`num_enum` 0.7.6, `num_enum_derive` 0.7.6.

### BSL-1.0 (2)

`clipboard-win` 5.4.1 (Windows), `error-code` 3.4.0 (Windows).

### MIT OR Unlicense (2)

`byteorder-lite` 0.1.0, `memchr` 2.8.3.

### (MIT OR Apache-2.0) AND Unicode-3.0 (1)

`unicode-ident` 1.0.27.

### 0BSD OR Apache-2.0 OR MIT (1)

`adler2` 2.0.1.

### Apache-2.0 AND MIT (1)

`dpi` 0.1.2.

### Apache-2.0 OR BSD-2-Clause OR MIT (1)

`zerocopy` 0.8.63 (Linux).

### Apache-2.0 OR CC0-1.0 (1)

`imgref` 1.12.3.

### BSD-2-Clause (1)

`arrayref` 0.3.9.

### ISC (1)

`libloading` 0.8.9 (Linux).
