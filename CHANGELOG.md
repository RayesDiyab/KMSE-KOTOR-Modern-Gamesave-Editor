# Changelog

The KOTOR Modern Gamesave Editor (KMSE) for KotOR 1 and 2, until 2026-10-24 the KMRP Save Editor. Until 2026-10-24 it lived in the KMRP repository
(`save-editor/`); its history came along, and its earlier changes are in KMRP's own
CHANGELOG.

## [Unreleased]

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
  as does a save made with Save as a New Save. A pen at the name's top right turns the name into a text box: Enter or the save
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

