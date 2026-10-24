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
| Linux | the window libraries the GitHub workflow installs: `libxkbcommon-dev libfontconfig1-dev libwayland-dev libx11-dev libxcursor-dev libxi-dev libxrandr-dev` |
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
# Linux and macOS: tests, then the release build, into dist/.
sh build.sh
```

On macOS, `build.sh` makes `dist/KOTOR Modern Gamesave Editor.app`; on Linux,
`dist/kotor-modern-gamesave-editor`. Neither has been built on a Linux or Mac machine by
the maintainer. The GitHub workflow (`.github/workflows/build.yml`) is set up to test and
build on Windows, Linux and macOS; it has not run in this repository yet (2026-10-24).

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
