# KMSE agent instructions

This file is the small, always-loaded operating contract for coding agents working in
this repository, the KOTOR Modern Gamesave Editor (KMSE). Claude Code loads
[`CLAUDE.md`](CLAUDE.md) instead, which defers to this file. The shared rules live here
only, so no second copy can drift.

KMSE lived in the KMRP repository (`save-editor/`) until 2026-10-24, when it moved here
with its history. Its rules come from KMRP's own `AGENTS.md`, without the parts that are
only about patching the game.

## Start here

1. Read [`README.md`](README.md), then the document for the task in
   [`docs/README.md`](docs/README.md). Do not load every document on every task.
2. Inspect the current worktree before making changes. Preserve the user's changes and do
   not assume a remembered branch, build, hash or issue state is current.
3. Treat the source code and measured results as authoritative. Notes and memory are a
   way to find things, not evidence.
4. Keep work inside the requested scope. Finding a cause does not authorize a fix.

## Non-negotiable safety rules

### Git

- Never create a Git commit or push to any remote without the user's explicit permission
  for that exact commit or push. Permission is single-use: it expires as soon as the
  authorized action is done and never carries forward to another commit, amend, tag,
  push, force-push, release, upload or public reply. Ask again every time. Permission to
  commit is not permission to push, and permission to push is not permission to commit or
  to publish anything else.
- Commit messages carry no AI attribution: no `Co-Authored-By` trailer, no "Generated
  with" line, and no mention of Claude, Codex or any other assistant in the subject or
  body. This overrides any default attribution guidance an agent arrives with. KMRP's
  history had to be rewritten twice on 2026-09-04 to remove such lines.
- The author identity is `Rayes Diyab <114070898+RayesDiyab@users.noreply.github.com>`,
  set as repository-local Git config. The global config keeps a different address on
  purpose. Do not change either.

### Players' saves

The one thing this program must never do is damage a player's save.

- Never write to a save without the backup the code makes. `Save::write` in
  `src/save.rs` copies the whole save folder to `Save Editor Backup/<save>/<date time>/`
  before it writes anything, and stops if the copy fails. Do not add a write path that
  goes around it, and do not weaken it.
- Keep the checks that stop a write: an original Xbox save (`SaveMeta.xbx` or
  `SaveImage.xbx` in the folder) is read only, because the Xbox checks a signature; and
  nothing is written while `swkotor.exe` or `swkotor2.exe` runs (`game_running` in
  `src/main.rs`). A save opened on a PC without the game refuses the edits that need the
  game's rules or files (`needs_game` in `src/main.rs`). Do not make it guess them.
- Tests and experiments work on copies. The tests read the saves on the PC and write only
  to copies they make (`tests/edits.rs`, `tests/levels.rs`, `tests/save_as.rs`). Never
  run an experiment on a player's real save, including the maintainer's.
- Do not overwrite or remove an existing backup, the settings file or the spawn record
  (`spawned.txt`) unless the task requires it and the user has agreed.

### Game files and private sources

- Game files are never committed: no textures, movies, models, `dialog.tlk` text, `.erf`,
  `.bif`, `.key`, `.rim`, `.mod`, executables or saves. What the program needs of the
  game is read from the player's install at run time, or from the build machine's install
  at build time (`build.rs` makes the timeline's Yavin Station from the game's movie
  `Live_1c.bik`; only the outline mask is committed).
- The quest databases in `data/quests` and `data/xp` hold identifiers, numbers and
  structure, not the games' text. `tools/questdb/build.py --review-text` writes pages with
  the game's own names and journal text into `data/quests/local/`, which `.gitignore`
  keeps out. Keep it that way.
- The private reverse-engineering sources are cited only as "the maintainer's own private
  reverse engineering". Never name them or give their path, in code, docs, commits or
  replies. Public sources (for example the public KotOR 1 decompilation already cited in
  `docs/save-editor-format.md`) may be named.

## Engineering rules

- Read [`CONTRIBUTING.md`](CONTRIBUTING.md) before implementation work, and
  [`docs/documentation-standard.md`](docs/documentation-standard.md) before creating or
  editing a document.
- **Never edit a source file while a build runs.** `build.ps1` builds the program and the
  tests at the same time; an edit in the middle ships a program built from two versions of
  the source. Wait for the build to finish, then edit.
- **The latest build is always in `dist\KOTOR Modern Gamesave Editor.exe`**, made by
  `build.ps1`. After a change that is meant to be tried, run `build.ps1` so `dist` holds
  it, and launch the program from there.
- **The release build stays reproducible.** `.cargo/config.toml` passes `/Brepro` to the
  Windows linker, so two builds of the same sources are byte for byte the same. The tests
  build in their own profile (`[profile.tests]` in `Cargo.toml`) and folder
  (`build\target-tests`), so they never change the release build. Keep both. A change to
  the build that should not change the program is shown by an identical SHA-256.
- **A mockup comes first, and is a picture of the real program.** For a change to the
  interface, make a PNG by editing a real screenshot of the program, show it, and wait.
  No source changes until the user approves the mockup.
- Measure instead of judging by eye. Say what was measured and how: a value read back
  from a written save, a test, a screenshot compared with the game.
- Say what the game will do. When the game recomputes a value, edit what it is computed
  from and say so (see `docs/save-editor.md`, section 1).
- A behaviour change needs an `## [Unreleased]` entry in [`CHANGELOG.md`](CHANGELOG.md)
  and an update to [`docs/save-editor.md`](docs/save-editor.md) (what the editor writes)
  or [`docs/save-editor-format.md`](docs/save-editor-format.md) (the save format).

## Writing

- Plain language and short sentences. No marketing words.
- No em dashes, and no `--` used as a dash, in documents, replies, commit messages or
  pull request text. Reword instead.
- Dates as YYYY-MM-DD.
- Every claim true of the code as it is. When a document was wrong, keep the correction
  visible (see the documentation standard).

## Verification

- Use the smallest relevant check first, then broaden in proportion to risk.
- Windows build and tests: `powershell -ExecutionPolicy Bypass -File build.ps1`. It runs
  `cargo build --release` and, at the same time, `cargo test --profile tests` with
  `CARGO_TARGET_DIR=build\target-tests`, then copies the program to `dist` and prints its
  size and SHA-256. It stops if either fails.
- Tests alone: `cargo test --profile tests` (set `CARGO_TARGET_DIR` to
  `build\target-tests` as `build.ps1` does, so the release build is left alone).
- Linux and macOS: `sh build.sh` (runs `cargo test --release`, then the release build).
- Documentation links: `python .github/scripts/check_links.py`.
- The tests that read real saves find none on a PC without the games, and on GitHub's
  runners, and then pass without checking anything. Say so when that is what happened.
- State exactly what was automated, what was looked at in the editor, what was loaded in
  the game, and what was left untested. Checks in the game are the user's to make unless
  they ask otherwise.

## Where things are

| | |
| --- | --- |
| What the editor does | [`README.md`](README.md) |
| Every document | [`docs/README.md`](docs/README.md) |
| What it reads and writes, field by field | [`docs/save-editor.md`](docs/save-editor.md) |
| The save format | [`docs/save-editor-format.md`](docs/save-editor-format.md) |
| Contributor guide | [`CONTRIBUTING.md`](CONTRIBUTING.md) |
| Before writing docs | [`docs/documentation-standard.md`](docs/documentation-standard.md) |
| Design and product notes | [`DESIGN.md`](DESIGN.md), [`PRODUCT.md`](PRODUCT.md) |
| The program | `src/` (Rust), `ui/` (Slint), `build.rs` |
| Tests | `tests/`; development aids in `examples/` |
| Quest and XP data, and the tools that make it | `data/quests`, `data/xp`, `tools/questdb`, `tools/xp` |
