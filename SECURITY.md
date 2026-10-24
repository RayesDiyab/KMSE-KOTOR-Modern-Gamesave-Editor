# Security policy

> **Documentation standard.** This document follows
> [`docs/documentation-standard.md`](docs/documentation-standard.md). Read it before
> editing this file, and check the result still meets it: measured claims only, rejected
> alternatives and corrections kept visible, and anything untested labelled as untested.

## Scope

The KOTOR Modern Gamesave Editor (KMSE) is a desktop program for the saves of two
single-player games. It makes no network requests, has no telemetry and no service
component (checked 2026-10-24: no network crate in `Cargo.toml`, no network code in
`src/`). The realistic risks are two:

- **The program reads files it did not make.** A save can come from anywhere: another PC,
  a phone, a download. A save is a set of GFF files inside ERF archives; the editor also
  reads the game's own archives and tables, and TGA and TPC pictures (the save's
  screenshot, portraits, icons). A malformed file that crashes the editor, makes it hang,
  makes it use unbounded memory, or makes it read or write outside the data it was given
  is in scope. The readers are in `src/formats/` (`gff.rs`, `erf.rs`, `rim.rs`,
  `tables.rs`, `tga.rs`, `tpc.rs`).
- **The program writes into the player's save folders.** A bug that writes outside the
  save being edited, overwrites another save, loses the backup or writes a save the game
  can no longer load is in scope. That includes the names of new folders: Copy and Save
  as a New Save make a folder named after a name the player types, with the characters
  Windows forbids in a file name taken out (`src/main.rs`).

What the editor writes, and where:

| Written | Where | When |
| --- | --- | --- |
| The changed files of the save being edited | that save's folder; each file is written in full to `<name>.kmrp-new` first, and the new files replace the old ones only when every one is written | Save Changes |
| A backup of the whole save folder | `Save Editor Backup/<save folder>/<date time>/` in the folder that holds the save; for a KotOR 2 Steam Cloud save, the game's local `saves/Save Editor Backup` instead, so Steam does not upload it | before every write, and before a rename |
| A new save | the next free slot in the same saves folder | Copy, Save as a New Save |
| `settings.txt`, `spawned.txt` | `%LOCALAPPDATA%\KOTOR Modern Gamesave Editor` on Windows, `~/Library/Application Support/KOTOR Modern Gamesave Editor` on macOS, `$XDG_DATA_HOME` (or `~/.local/share`) `/KOTOR Modern Gamesave Editor` on Linux | when a setting changes, when a creature is spawned |

The game's own files are only read; inside the game's folder the editor writes only to
the saves and to the backups folder above. Details are in
[docs/save-editor.md](docs/save-editor.md), section 5.

## Reporting

Report suspected security issues, and any bug that can **damage a save or lose its
backup**, privately: open a
[GitHub security advisory](https://github.com/RayesDiyab/KMSE-KOTOR-Modern-Gamesave-Editor/security/advisories/new)
(GitHub's private vulnerability reporting) rather than a public issue, so it can be fixed
before it is described in public.

Please include the editor's version (shown at the bottom of the saves page and in
Settings), the game (KotOR 1 or KotOR 2) and where it came from (Steam, GOG, disc, Aspyr's
ports), and the operating system. A save that shows the problem helps most. **Do not
attach game files** (archives, textures, executables); a save is fine.

Expect a first answer within a few days. This is a hobby project kept by one person;
there is no paid support and no bounty.

## Things that are working as intended

These are deliberate refusals, not bugs:

- **An original Xbox save cannot be changed.** The Xbox checks a signature on every save,
  so a changed save would be called damaged. The editor opens it to look at only.
- **Nothing is written while the game runs.** The game can overwrite a save folder it did
  not expect, so Save Changes is refused while `swkotor.exe` or `swkotor2.exe` runs.
- **Without the game on the PC, some edits are refused.** A save opened on a PC without
  the game shows what it holds, but edits that need the game's rules or files (class and
  level changes, new items, Skip Ahead, travel, spawning, companions brought in) say why
  they cannot be made instead of guessing.
- **A backup that cannot be made stops the write.** If the save folder cannot be copied,
  nothing is written.

## For anyone building from source

Do not weaken those refusals or the backup to make an edit work; they are what stops the
editor from damaging a player's save. Tests write only to copies of saves, in the system's
temporary folder.
