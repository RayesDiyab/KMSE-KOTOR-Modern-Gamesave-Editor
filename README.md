<div align="center">

<img src="assets/kmse-logo.png" alt="KMSE crest" width="142">

# KMSE - KOTOR Modern Gamesave Editor

**A save editor for *Star Wars: Knights of the Old Republic* and *KotOR II: The Sith Lords*.**

It opens any KotOR 1 or KotOR 2 save, shows what it holds in plain words, and changes it
without breaking it. For Windows, Linux and macOS. It works with or without
[KMRP](https://github.com/RayesDiyab/KMRP-KOTOR-Modern-Restoration-Patch) installed.

[What it does](#what-it-does) · [Your saves are safe](#your-saves-are-safe) · [Without the game](#without-the-game) · [Get it](#get-it) · [Build from source](#build-from-source) · [Documentation](#documentation) · [Licence and credits](#licence-and-credits)

</div>

---

Until 2026-10-24 this program was the KMRP Save Editor and lived in the KMRP repository.
It moved here with its history and its own name, so nobody takes it to need KMRP. Its
changes before that day are in KMRP's CHANGELOG.

## What it does

* **Finds your saves by itself**: Steam (any library), Steam Cloud saves of KotOR II, GOG and
  disc installs, the Aspyr Mac and Linux ports. Saves from anywhere else (a phone, a Switch,
  another PC) can be added once in Settings, with Add a Save Folder. Only Steam on Windows
  has been tried by the maintainer; the other places are found from their documented paths
  and are untested ([save-editor.md](docs/save-editor.md), section 2).
* **Looks like the game**: the save list is the game's Load screen, with each save's picture
  and party; the editor is the in-game menu's tabs. KotOR II in its own colours; dark and
  light mode and three text sizes in Settings (the gear, top right).
* **Character**: name, experience (the game then offers the level-up itself), a second
  class, alignment, attributes, Health and Force, portrait and body, position.
* **Abilities**: skills, and feats and Force powers added or removed one at a time.
* **Inventory**: credits, components and chemicals, any item the game (or a mod) has, stack
  sizes, and what each party member wears.
* **Party**: recruited, can be picked, influence, party experience, and bringing a member
  over from another save.
* **Story**: a timeline of the game's chapters with every save on it, Skip Ahead to a later
  chapter and Go Back to an earlier one; a quest tracker; the galaxy map; Pazaak cards.
* **Map**: the area's map with its people, doors and containers; moving the player, travel
  to any area, and spawning a creature.
* **Advanced**: every global variable, and a comparison with another save to fix a stuck
  quest; doors, containers, stores and creatures of the current area; the save's name,
  time played and cheat mark.
* **Copy, rename, Save as a New Save**: a save can be copied into the next free slot,
  renamed on the game's Load screen, or saved under a new name while the original stays as
  it was.

What it changes, field by field, and how each part was tested:
[docs/save-editor.md](docs/save-editor.md). The save format of both games:
[docs/save-editor-format.md](docs/save-editor-format.md).

## Your saves are safe

* **Nothing is written until Save Changes.** Every change waits in memory; Revert reads the
  save from disk again.
* **The game has to be closed.** Save Changes is refused while `swkotor.exe` or
  `swkotor2.exe` runs, because the game can overwrite a save it did not expect.
* **A backup first, every time.** Before anything is written, the whole save folder is
  copied to `Save Editor Backup` in the folder that holds your saves (a KotOR II Steam Cloud
  save's backup goes to the game's local `saves` folder, so Steam does not upload it).
  Settings shows the folder, with an Open Backups Folder button. To undo an edit, copy the
  backup's files over the save folder.
* **Only well-formed changes.** Every field, its type and its order is kept as read. Where
  the game recomputes a value when the save loads (the most Health and Force, for example),
  the editor changes what the game computes it from and says so.

Original Xbox saves can be looked at but not changed: the Xbox checks a signature on every
save and would call a changed one damaged.

## Without the game

A save opened on a PC without the game, from a folder added in Settings, shows what it
holds: the skills, class names, levels and XP, companions, items by tag, feats and powers
by number, quests by tag and stage, and body, voice and gender by number. Edits that need
the game's rules or files are refused with a message saying why: class and level changes,
the most Health and Force, new items and equipping, Skip Ahead, Go Back, travel, spawning,
and companions brought in from another save. The current Health and Force can still be set.
Not yet tried on a PC without the game (2026-10-24); `tests/no_game.rs` checks the part
the program can check by itself.

## Get it

No release has been published yet (2026-10-24). Until then, build it from source as below.
On Windows the result is one file, `KOTOR Modern Gamesave Editor.exe`, with no installer.
It keeps its settings in `%LOCALAPPDATA%\KOTOR Modern Gamesave Editor`
(`~/Library/Application Support/...` on macOS, `~/.local/share/...` on Linux), never in
the game's folder.

## Build from source

This folder builds on its own; nothing outside it is needed. Both scripts need Rust
(stable, through rustup).

```powershell
# Windows: makes dist\KOTOR Modern Gamesave Editor.exe and runs the tests beside it.
powershell -ExecutionPolicy Bypass -File build.ps1
```

```sh
# Linux and macOS: tests, then dist/kotor-modern-gamesave-editor
# or dist/KOTOR Modern Gamesave Editor.app.
sh build.sh
```

On Linux the window also needs the usual X11, Wayland and fontconfig development packages;
the list is in [CONTRIBUTING.md](CONTRIBUTING.md). The Linux and macOS builds have not
been run by the maintainer yet.

The window is set in the KMRP Original HD fonts: the four it uses are in `fonts/`, copies
of KMRP's own, under the SIL Open Font License 1.1 ([fonts/OFL.txt](fonts/OFL.txt)). A
folder named by `KMRP_FONTS` is used instead when it holds all four.

If KotOR and ffmpeg are on the build machine, the build makes the timeline's picture of
Yavin Station from the game's own movie; it is never committed. Without them the button
shows the name alone.

The icon, a holocron with a datapad, and how to remake it:
[assets/icon](assets/icon/README.md).

> [!NOTE]
> **Game files are never committed.** The editor reads names, text, portraits and icons
> from the player's own game when it runs. The quest databases in `data/quests` hold
> identifiers and numbers, not the games' text.

See [CONTRIBUTING.md](CONTRIBUTING.md) for the working rules this project holds itself to.

## Documentation

| | |
| --- | --- |
| [docs/](docs/README.md) | Every document, starting at [docs/README.md](docs/README.md) |
| [docs/save-editor.md](docs/save-editor.md) | What the editor reads and writes, field by field, and what was tested |
| [docs/save-editor-format.md](docs/save-editor-format.md) | The save format of both games |
| [CHANGELOG.md](CHANGELOG.md) | What changed |
| [SECURITY.md](SECURITY.md) | Reporting a bug that damages saves, or a vulnerability |
| [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) | Everything here that is not the maintainer's own work, and its licence |

## Licence and credits

KMSE is distributed under the **GNU General Public License v3.0 or later**; see
[LICENSE](LICENSE). The fonts are under the SIL Open Font License 1.1.

Created by **RaymanGT**, the maker of KMRP.

* The **KMRP Original HD fonts**, from KMRP (OFL 1.1).
* **Slint**, the interface toolkit, used under the GPL-3.0, and the other Rust crates
  listed in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
* The **Ebon Hawk** on the timeline is a free-to-use render from Deadly Stream; its
  author is not recorded here.
* **Aurebesh AF** by AurekFonts (CC0), for the icon's datapad.
* The public KotOR 1 decompilation
  [swkotor1-reconstruction](https://github.com/mattlgroff/swkotor1-reconstruction), for
  some of the engine's rules.
* **KSE** (KotOR Savegame Editor), as the list of what players expect a save editor to do.
  No code of KSE or its forks is used.

*Star Wars: Knights of the Old Republic* and *KotOR II: The Sith Lords* belong to
Lucasfilm and Disney, BioWare and Obsidian Entertainment. This is an unofficial community
tool, not affiliated with or endorsed by any of them. You need the game to make most
edits.
