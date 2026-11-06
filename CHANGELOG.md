# Changelog

The KOTOR Modern Gamesave Editor (KMSE) for KotOR 1 and 2, until 2026-10-24 the KMRP Save Editor. Until 2026-10-24 it lived in the KMRP repository
(`save-editor/`); its history came along, and its earlier changes are in KMRP's own
CHANGELOG.

## [Unreleased]

- **The audit's findings fixed** (2026-11-02; [the worklist](docs/audit-2026-11-02-worklist.md)
  has each one and how it was checked). No work is lost without asking: a timeline click
  is refused and closing asks once. Cancel in Copy and Save as a New Save cancels. A save the
  game changed since it was opened is not written over. A write that fails part way puts the
  old files back and says truly that nothing changed; a failed Save as leaves no folder. A
  skip-recruited companion's item comes off as the whole item. A panic shows a message and
  leaves `crash.txt`. Skip Ahead reaches a quest stage by one way only, side chapters stand
  alone, Go Back restores the star maps and capture, leaves side chapters and takes the
  fourth planet with the Leviathan, dark runs read as played, removed quests stay removed,
  XP counts each chapter once and is never raised by Go Back, and what could not be applied
  is reported. Two-handed weapons empty the left hand. Stealth Field Generators can be added,
  map notes show, total level is capped, and read-only saves refuse edits at once. Rename
  backups go to `Save Editor Backup`. 78 tests pass (`tests/audit_fixes.rs`). **In the game,
  untested; in the program, started once and not clicked.**
- **Backups leave what Steam syncs** (2026-11-02): they go to `Save Editor Backup` in the game's
  folder, not inside `Saves`. Backups already made stay where they are.
- **Load-screen pictures follow the party** after a skip, a return or an import (2026-11-02).
- **Attribute decreases count** (2026-11-02): effects of type 37 lower what the Health and Force
  figures are worked from; the floor of 3 is confirmed for both games.
- **Skip Ahead finds one star map at a time** (2026-11-02). A skip that included Tatooine
  or Korriban found all four maps, opened the Unknown World and left `K_STAR_MAP` at 10, so
  the Leviathan capture never played. Closing a quest replayed every branch of the game's
  star map script. Each planet now sets its own flag and the count follows, in any order.
  The Leviathan goes in before a fourth planet, as in the game, and a skip to the Leviathan
  leaves the fourth planet open, and a stage skip after the Leviathan does not set the
  capture up again. Saves already skipped keep the wrong values. Measured on
  in-memory copies (`tests/star_maps.rs`); in the game, untested.
- **Long save names scroll while editing** (2026-10-29). The rename field follows the
  caret when moving with the keyboard or dragging a selection past its visible edge.
  The shared single-line field keeps the caret visible inside its padded viewport.
- **External player-state saves open, read only** (2026-10-29). A KotOR 1 arrival
  autosave keeps its player only in `pifo.ifo`, so it failed to open. The reader now falls
  back to that file when the archive has no player. Saves with `pifo.ifo` are read only,
  including the measured KotOR II autosave that also has archived player copies. Save
  Changes, Save as a New Save and rename are refused. Copy preserves the player files
  and changes only the new copy's Load-screen name. Normal saves remain editable.
- **New save folders named as the game names them** (2026-10-29). Copy and Save as a New
  Save name the new folder `<number> - Game<number - 1>` again, not after the save's name.
  The game overwrites a save by writing to that pattern, so overwriting a copy named
  "000920 - KMRPPazaak" left it in place beside a new "000920 - Game919" of the same name.
  The typed name still shows on the game's Load screen.
- **No extra story or travel success screen** (2026-10-25). Skip Ahead, Go Back and
  map travel return directly to the editor with the pending change count. Removed the
  remaining "Skipped to" report and travel result popup. Save Changes still writes
  the edits with a backup.
- **Choose alignment when going back** (2026-10-25). Go Back defaults to 50, Neutral.
  The chapter preview has a red-to-cyan slider, a numeric field and Reset to Neutral.
  Apply uses that chosen value, including returns inside a quest. Removed the estimated
  chapter alignment subtraction that changed a reported save from 79 to 10.
- **Go Back restores recorded earlier levels** (2026-10-25). Rebuilds classes from
  the retained level history, removes later recorded gains and restores the earlier XP.
  Later class edits no longer leave high levels behind with low XP. Removing a future
  companion also removes their active-party entry. Returning to a side quest includes
  companions who joined at the chapter's entry. Verified on copied saves; the reported
  Taris loading crash still needs an in-game retry.
- **Go Back returns directly to the editor** (2026-10-25). Removed the extra
  "Gone back" result popup. The applied changes remain unsaved until Save Changes.
- **Timeline chapter preview** (2026-10-25). The wider chapter popup keeps expandable
  quests and their stages on the left, with the selected point's details on the right.
  It prepares the change on an independent in-memory save and shows the actual before
  and after values, abilities and companion changes. Inventory gains and losses use the
  game's icons in a scrollable two-column list, with small green plus and red minus
  quantities. Unchanged totals appear once. Apply uses the prepared result; Cancel
  discards it. Going back always uses the story's inventory and credits, with no policy
  selector. Row dividers leave the full yellow selection and hover outline visible.
  Save Changes still makes the backup before writing.
- **Cancel a pending rename** (2026-10-24). Clicking outside the save's name field
  and save icon closes the field and discards the draft. Switching games or selecting
  another save cancels it too. Enter and the save icon still apply the rename.
- **Its own repository, and its own name** (2026-10-24). Moved out of KMRP with its
  history, and renamed KOTOR Modern Gamesave Editor (KMSE) so nobody takes it to need KMRP: the window,
  the program file (`KOTOR Modern Gamesave Editor.exe`), the crest (the KMRP sigil over KMSE, `assets/sigil.png`
  and `assets/kmse-logo.png`). Settings and backups move from `KMRP\Save Editor` to
  `KOTOR Modern Gamesave Editor` on the first start. Builds, tests and the GitHub workflow work from the
  repository's root.
- **A save opened without the game** (2026-10-24). On a PC without KotOR, a save from an
  added folder now shows what it holds instead of empty tabs: the eight skills, class names,
  levels and XP (the games' XP table is a formula), companions, items by tag, owned feats
  and powers by number, quests by tag and stage, body, voice and gender by number. Edits
  that need the game's rules or files (class and level changes, the most Health and Force,
  new items and equipping, Skip Ahead, Go Back, travel, spawning, companions brought in)
  are refused with a message saying why; the current Health and Force can still be set
  (`tests/no_game.rs`). Not yet seen on a PC without the game.
- **Lighter program** (2026-10-24). 13.5 MB instead of 22.8 MB: the quest databases are
  carried with their white space taken out and deflated (build.rs), unpacked once. An edit
  in Story refills only the section on screen instead of all six; the game's journal is
  read once per game; the map's arrow is cached; Skip Ahead no longer copies every
  chapter's data on each click. Measured: about 50 MB in memory after start, no work while
  idle.
- **Kerned text and larger sizes** (2026-10-24). The window is set in KMRP's fonts as kerned
  that day (a modder saw the small a stand apart: "Aw areness", "G a m" in Gamesave). In
  Settings, Medium text is what Large was and Large is larger still.
- **Copy and rename** (2026-10-24). A Copy button (two pages) beside a save's name on the
  saves page and beside Settings in the editor copies the save as it is on disk into the
  next free slot, under a name asked for ("<name> - Copy"); changes not saved yet stay out
  of it; its folder carries that name after the slot's number ("000913 - Game26 - Copy"),
  as does a save made with Save as a New Save (undone 2026-10-29: the game's overwrite
  then made a second save; see above). A pen at the name's top right turns the name into a text box: Enter or the save
  icon renames the save on the game's Load screen (backed up first), Escape cancels. The
  character, time played and date sit on the party's row. Save as a New Save is greyed
  out until something is changed, like Save Changes, which is wider now with its arrow in
  the same colour and a line between them. With a save open the header says only
  "Gamesave Editor".
- **Feats and Force powers by name alone** (2026-10-24). The line under each (the
  description's first line, "Prerequisites:" or "Universal Power") only repeated the group
  and is gone; a long name takes two lines instead of being cut short.
- **Smooth crest and gear** (2026-10-24). The crest and the settings gear are made at the
  size they are drawn, area-averaged; shrunk at run time they came out hard-edged.
- **Galaxy Map and Pazaak** (2026-10-24; [save-editor.md](docs/save-editor.md)). The
  places the Ebon Hawk can fly to are drawn as the game's own planets, globes made from the
  game's planet textures at run time and preloaded right after start: a click opens one
  (green ring) or closes it (grey). Places the game's travel script cannot fly to (the
  Endar Spire, the Ebon Hawk, the Leviathan, the Star Forge) are left out. The Pazaak side
  cards are shown large with the game's own card faces, each with how many are carried.
- **Quests drop down into their stages** (2026-10-24). A quest opens into its stages as an
  indented list; a stage picked shows in full with Start the Quest, Start Quest at This
  Point or Finish the Quest, by where it stands.
- **Save as a New Save** (2026-10-24). An arrow on Save Changes writes the changes into a
  new save in the next free slot under a new name and leaves the original byte for byte as
  it was (`tests/save_as.rs`).
- **Advanced tab** (2026-10-24). Story Values (now with the save's name, time played and
  cheat mark), Area Objects and Creatures moved from Story to a tab of their own; Story
  opens on the timeline.
- **Faster, reproducible builds** (2026-10-24). The tests build beside the program in their
  own profile, so `build.ps1` takes about 225 seconds after a change instead of about 505;
  the program is built exactly as before, and the linker's `/Brepro` makes two builds of the
  same sources byte for byte the same. The newest build is always in `dist`.
- **Lighter** (2026-10-24). The quest data is parsed once instead of on every refresh (13 MB
  each time), the other saves for the timeline and the Map tab's area lists are read in the
  background.
- **The Save Editor's story is a timeline** (2026-10-23;
  [save-editor.md](docs/save-editor.md), section 3b). Story's sections are tabs
  along the top; the Timeline tab shows the main chapters on one line from start to end on a
  starfield, the open save in gold, every other save of the game in green (blue in KotOR II)
  with its picture, character and how far its story is on pointing, and the Ebon Hawk and
  Yavin Station as ships for their side chapters. A chapter opens a pop-up from its
  beginning to its end with its quests and stages, each with Skip to Here or Go Back to
  Here. Fixed with it: a save past Taris read the Endar Spire as never played (the game
  takes its quest out of the journal on Taris), so Skip Ahead could have played it again; a
  chapter now also counts as played when a chapter the save has played removed its quest.
  A typed Health or Force maximum the level cannot reach now asks to raise or lower the
  level, with a chart of both levels' ranges, instead of stopping short; the Health and
  Force notes say what the game computes them from (Wis alone for KotOR II's Force), and a
  save whose level records another editor left out of step is flagged. KotOR II's
  companions trained by Skip Ahead now get what the game's AddMultiClass gives (the Jedi
  class at level 0 and the XP for their next level, the level-up waiting in the game)
  instead of a finished level 1 with no feats or powers. **Not yet seen in the games.**
- **The Save Editor keeps Health and Force exactly as the game would through level and
  class changes** (2026-10-23; issue 70, after a Discord report of an editor breaking
  them). Both maxima are now computed by the engines' own rules (KotOR 1 and KotOR 2,
  read from the executables and matched against every save on the maintainer's PC)
  instead of read from `MaxHitPoints` and `MaxForcePoints`, which the games never read.
  Fixed: a party member's current Health and Force were read and stored by the player's
  rule (the engine adds level x modifier for them), so Heal and a set value came out
  short by any Toughness or Force Sensitive bonus; added levels and classes now heal as
  the game's level-up does and lower ones keep what was missing; the stored maxima are
  written as the game writes them (Skip Ahead's saves had a Jedi player at Force 0 there);
  an edit to a party member no longer reaches an unrelated area creature with the same
  tag (the Endar Spire's Carth). Rules in
  `docs/save-editor-format.md` section 3.4; tests in
  `tests/levels.rs`. Not yet checked in the games.

