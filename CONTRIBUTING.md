# Contributing to KMSE

> **Documentation standard.** This document follows
> [`docs/documentation-standard.md`](docs/documentation-standard.md). Read it before
> editing this file, and check the result still meets it: measured claims only, rejected
> alternatives and corrections kept visible, and anything untested labelled as untested.

Thanks for looking. This document covers how to build the KOTOR Modern Gamesave Editor
(KMSE), and the working rules it holds itself to. Most of them exist because breaking
them already cost work, here or in KMRP, where the editor lived until 2026-10-24.

## Ground rules

**Never break a save.** Nothing is written until Save Changes. The game has to be closed.
The whole save folder is copied to a backup before any file is written. Keep all three
when you change how a save is written; `docs/save-editor.md`, section 5, says how a write
stays safe.

**Work on copies.** Tests and experiments copy a save first and change only the copy. Do
not test on a save you would miss.

**Never commit game files.** No textures, movies, models, `dialog.tlk` text, archives
(`.erf`, `.bif`, `.key`, `.rim`, `.mod`), executables or saves. The editor reads what it
needs from the player's game when it runs. The quest databases in `data/quests` and
`data/xp` hold identifiers and numbers only; pages with the game's own text go to the
ignored `data/quests/local/`.

**Measure; do not judge by eye.** Read the value back out of the written save, run a test,
or load the save in the game, and say which you did.

**Say what the game will do.** The games recompute some fields when a save loads and
ignore what is stored there (`MaxHitPoints` is one). Change what the game computes the
value from, and say so in the interface and in the documentation.

**Record what was disproved.** When an idea is tried and fails, write down what it was and
why it failed, next to the thing it would have changed. It stops the same dead end being
explored twice.

**Documentation has a standard.** Read
[docs/documentation-standard.md](docs/documentation-standard.md) before creating or
updating a document.

**Corrections belong in the document, not only in the commit.** If a document states
something that later proves wrong, correct it and say what the earlier text said and why
it was wrong.

**Interface changes start with a mockup.** A mockup is a PNG made by editing a real
screenshot of the program. The source changes after the mockup is approved.

## Building

### Prerequisites

| | |
| --- | --- |
| Rust | stable, through rustup. On Windows the MSVC toolchain (`x86_64-pc-windows-msvc`), whose linker takes `/Brepro` |
| Linux | the window libraries the GitHub workflow installs: `libxkbcommon-dev libfontconfig1-dev libwayland-dev libx11-dev libxcursor-dev libxi-dev libxrandr-dev`. For the AppImage also curl and sha256sum, and binutils for its checks |
| Fonts | nothing to do: the four KMRP Original HD fonts are committed in `fonts/`. `KMRP_FONTS` can name another folder that holds all four |
| Optional: the game and ffmpeg | for the timeline's picture of Yavin Station, which `build.rs` makes from KotOR's movie `Live_1c.bik` (found through `KOTOR_DIR`, the Steam libraries or the usual GOG folder). Without them the build warns and the button shows the name alone |
| Optional: Python 3 | only to rebuild the quest and XP data (`tools/questdb`, `tools/xp`, standard library only, and a KotOR install), or the icon (`assets/icon`, with numpy, scipy and Pillow) |

### Commands

```powershell
# Windows: the program and the tests, built at the same time.
powershell -ExecutionPolicy Bypass -File build.ps1
```

`build.ps1` runs `cargo build --release` and, beside it, `cargo test --profile tests` in
`build\target-tests`. It stops if either fails, then copies the program to
`dist\KOTOR Modern Gamesave Editor.exe` and prints its size and SHA-256. Do not edit a
source file while it runs.

```sh
# macOS: tests, then dist/KMSE.app (Universal 2), dist/KMSE-<version>-macOS.dmg and .zip.
sh build-macos.sh            # --arm64-only, --no-tests
```

```sh
# Linux: tests, then dist/kotor-modern-gamesave-editor (on macOS it runs build-macos.sh).
sh build.sh
```

```sh
# Linux: tests, then dist/KMSE-Linux-<version>-x86_64.AppImage.
sh build-linux.sh            # --no-tests, --pack-only
```

`build-macos.sh` needs the Rust targets `aarch64-apple-darwin` and `x86_64-apple-darwin` and
Xcode's command line tools. It signs ad hoc unless `KMSE_SIGN_IDENTITY` names a Developer ID,
and it notarizes only when `KMSE_NOTARY_PROFILE` is set too. It was first run on 2026-11-08
([macOS audit](docs/audits/MACOS_PORT_AUDIT.md)).

`build-linux.sh` packs the program the way KMRP packs its Linux installer
(KMRP's `tools/build_linux_appimage.sh`): an AppDir with the program, a launcher (`AppRun`),
a desktop entry and the window's icon (`assets/save-editor-icon.png`), made into one file by
appimagetool 1.9.1, which it downloads once into `build/tools` and checks against its
published SHA-256. It needs curl and sha256sum besides Rust; with binutils it also checks what
the program links and prints the newest glibc it needs. `--pack-only` packs the program
already in `build/target/release`.

The AppImage carries the program alone, as the program is one file with its fonts and pictures
inside. What it loads from the system is a Linux desktop's own, and AppImages leave those to
the system:

| Library | Loaded | For |
| --- | --- | --- |
| glibc (`libc`, `libm`), `libgcc_s` | linked | the program. It needs the glibc it was built with or newer, so CI builds the AppImage on Ubuntu 22.04 (glibc 2.35) |
| `libfontconfig.so.1` | linked | finding the system's fonts (the window's own are inside the program) |
| `libX11`, `libX11-xcb`, `libxcb`, `libXcursor`, `libXi`, `libxkbcommon`, `libxkbcommon-x11` | opened at start, on X11 | the window, cursor and keyboard |
| `libwayland-client`, `libxkbcommon` | opened at start, on Wayland | the same |
| xdg-desktop-portal, over D-Bus | when a file or folder is picked | the file dialog (rfd's portal backend) |

The GitHub workflow (`.github/workflows/build.yml`) tests and builds on Windows and macOS and
makes the macOS app. On Linux its `appimage` job runs `build-linux.sh` on Ubuntu 22.04: the
tests, then the AppImage, uploaded as the `KMSE-Linux` artifact. Since 2026-11-09 it replaces
the `ubuntu-latest` build, which uploaded the plain program that the AppImage now holds. *Corrected 2026-11-09:* until then this section said the Linux build had
not been run on Linux and the workflow had not run in this repository (2026-10-24). The
workflow runs on every push; its Ubuntu job builds the program and passes the tests (for
example the run for `84eaeb2`, 2026-11-08). The maintainer has still not run the Linux build on
a Linux desktop.

### Reproducible builds

`.cargo/config.toml` passes `/Brepro` to the Windows linker, so the time stamp in the
program's header is a hash of its contents and two builds of the same sources are byte
for byte the same. The tests use their own profile, `[profile.tests]` in `Cargo.toml`, so
building them never changes the release build. A change to the build that should leave the
program alone is shown by an unchanged SHA-256.

## Tests

The tests in `tests/` read the KotOR 1 and KotOR 2 saves they find on the PC, the same way
the editor finds them, and write only to copies in the system's temporary folder. On a PC
without the games, and on GitHub's runners, they find no saves and pass without checking
those saves. `tests/no_game.rs` covers a save opened without the game.

The quest data is checked by the tests that apply it (`skip_whole_game_on_each_side`,
`skip_whole_k2_game_on_each_side` and others in `docs/save-editor.md`, section 3b), and
every data file says `"checked": false` until a person has played its chain.

## Pull requests

- One subject per pull request.
- Say what you measured, and how. Include the numbers.
- Say plainly what you could not check: a game not tested, a platform not built, a save
  not loaded in the game.
- Update [docs/save-editor.md](docs/save-editor.md) or
  [docs/save-editor-format.md](docs/save-editor-format.md) in the same change, including
  any earlier statement the change proves wrong.
- Add a [CHANGELOG.md](CHANGELOG.md) entry under `## [Unreleased]`.
- Run `python .github/scripts/check_links.py`.

Commit messages here are long on purpose: they explain the cause, not only the change.

## Reporting problems

Open an issue with the templates in `.github/ISSUE_TEMPLATE`. If a save stopped loading or
lost something after an edit, attach the save if you can, and the backup the editor made
of it: the editor's Settings, with a save open, shows the backups folder and an Open
Backups Folder button. For anything that can damage saves, see [SECURITY.md](SECURITY.md).
