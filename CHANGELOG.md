# Changelog

The KOTOR Modern Gamesave Editor (KMSE) for KotOR 1 and 2, until 2026-10-24 the KMRP Save Editor. Until 2026-10-24 it lived in the KMRP repository
(`save-editor/`); its history came along, and its earlier changes are in KMRP's own
CHANGELOG.

## [Unreleased]

- **Character and Map tidied** (2026-11-08, the maintainer). An attribute's modifier has room
  for three characters at every text size ("+22" showed as "+2."). The level records' warning
  under Heal Everyone, one orange line cut off, is an orange "i" in a circle that shows the
  whole text in a box on hover. The Map's frame takes the map's own shape, so the picture
  fills it with no black bands beside it.

- **A save without a picture gets one** (2026-11-08, the maintainer: saves made from KMRP's
  test saves showed an empty picture). When the editor writes a save that has no
  `Screen.tga` (Save Changes, Save as a New Save, Copy, rename), it adds the game's own load
  screen of the module the save loads in, at the game's size and format. A save with a
  picture keeps its own.
- **macOS port** (2026-11-08, the audit in
  [docs/audits/MACOS_PORT_AUDIT.md](docs/audits/MACOS_PORT_AUDIT.md)).
  - KotOR and KotOR II are found inside their Mac apps (Steam's and the Mac App Store's),
    in `/Applications`, `~/Applications` and wherever Spotlight knows of them. Settings' game
    folder takes the app itself, the folder Steam keeps it in, or the folder of game files.
    A folder without the game's files (a KEY, a TLK and `modules`) is refused when it is
    picked, not later.
  - Which game a folder holds is told by its program, its ini, its app's bundle id, or its
    first module. The size of `chitin.key` decided before and took the Mac App Store's
    KotOR II for KotOR.
  - KotOR II Steam's `cloudsaves` beside `KOTOR2.app` are listed. The Mac App Store's
    KotOR II, which keeps a `dialog.tlk` per language, is read in English, and a renamed
    companion's name is written in English.
  - When macOS will not let the editor read a copy's saves (the Mac App Store's KotOR keeps
    them in its own container), a pop-up lists every copy found: a green check where its
    saves are read (with how many), a grey question mark where there are no saves yet, a red
    cross where they cannot be read. Choose the Folder opens macOS's folder panel there, and
    a folder picked that can then be read is added; Open Settings opens Privacy & Security,
    Full Disk Access; Not Now stops the question for those folders (kept as `quiet=`).
  - The game is seen running on macOS by its app (`AppBundleExe`, `KOTOR_Exe`, `KOTOR2`),
    and the Windows game under Wine or CrossOver by its arguments. When the editor cannot
    tell, it does not write.
  - Finder's `.DS_Store` and `._` files in a save folder no longer stop Save Changes as "the
    save changed on disk", and are not copied into backups.
  - Command-Q and the Dock's Quit go through the window's check for changes not saved. They
    ended the program at once before.
  - Folders are flushed after a write's renames, and backups before the save is written
    over (macOS and Linux).
  - `build-macos.sh` makes `dist/KMSE.app` (Apple silicon and Intel), a DMG and a ZIP;
    `build.sh` runs it on macOS.
  - Go Back no longer changes the party table before refusing a save whose level history is
    damaged (both games; the window works on a copy, so no save took it).
  - KotOR II's Override subfolders are read in the order Windows lists them on every disk.
- **Windows audit fixes** (2026-11-07, the full audit in
  [docs/audits/WINDOWS_FULL_AUDIT.md](docs/audits/WINDOWS_FULL_AUDIT.md)).
  - Finish Quest takes one path through each set of alternatives of a conversation or script,
    as the game does. The quest data lists every path of a line as its own step, and all were
    applied together: KotOR II's DroidRetrieval took 46,400 credits, other quests gave rewards
    several times. Steps at different points of one line (the node's script, the line's own
    test, the line after a reply) are all run. Conditions on story values are read from the
    save as the run goes.
  - Finish Quest on a KotOR star map quest finds that planet's map only. It found all four
    and opened the Unknown World.
  - Finish Quest recruits and lets go the companions its steps do (Mandalore in KotOR II's
    escape_merchant). They were only listed as left to the game.
  - A quest at an ending already, or at the stage asked, is not finished again: that gave the
    ending's rewards twice, and moved the journal back.
  - Each backup records which save it was made from, by its place in the game folder, so a
    moved game keeps its records. Two saves with one folder name (two Steam accounts' cloud
    saves, a copy kept in the game folder) no longer see each other's backups in History, and
    Revert refuses the other's. A backup that cannot be made whole is removed.
  - A half-written save is told in a pop-up that names its backup, not only in the status line.
  - Save Changes, Save As and Revert refuse an original Xbox save themselves, as well as in the
    window.
  - A link (junction) inside a save folder is never followed by a backup, a copy or a write.
  - Made-up save files that asked for gigabytes of memory are refused (a GFF field named many
    times, archive entries covering the whole archive); the save list read every savenfo.res,
    so one such file stopped the editor each time.
  - Lowering Constitution (or Wisdom, Charisma, Toughness) keeps a hurt character's Health at
    1 or more; it went below 0.
  - Bring from Another Save takes someone else into a slot only from a save at least as far
    along the story: from an earlier one it put the Endar Spire's Trask in Bastila's place.
  - After Skip Ahead, Go Back and Finish Quest the member shown is the one picked; Finish
    Quest reads the party and the map again. The feat and power search is cleared with the
    box. An old "Saved" line gives way to the change count after the next edit. Closing the
    window warns again after edits made since the last warning, in this save or another.
  - The settings file is written beside itself and moved into place, and is never written
    over when it could not be read. tasklist and explorer are run from the Windows folder by
    their full path.
  - pulldown-cmark 0.13.5 to 0.13.6 (RUSTSEC-2026-0433; the editor never parses wiki links).
  - Add a Class offers a Jedi player only Jedi classes, and Change Class of a player's newest
    class after a Jedi class only Jedi classes: the game reads Force from the newest class,
    so Soldier added to a Jedi set Force to 0.
  - Revert keeps the open save, changes and all, when the save cannot be read again (no save
    was left open). Advanced, Creatures does not remove a party member's own copy in the area.
  - `examples/ingame_fixture.rs` makes one edited save in a new slot of a game's saves folder,
    for checks in the game.
  - Skip Ahead and Go Back leave the player leading: no party member stays marked as the one
    controlled (`PT_IS_LEADER` 0, `PT_CONTROLLED_NP` -1). A save skipped from the Endar Spire
    kept Trask's slot 0 marked; at the end of Taris that slot is Bastila, the game queued a
    switch to her on every area load, and after the turret game it crashed
    (`CrownFutureLeader`). Found and confirmed in the game by KMRP's agent.
  - Travel to a KotOR planet sets the current planet (`K_CURRENT_PLANET`): after a trip to
    Dantooine, Juhani still spoke of Manaan. Values as the game writes them: Endar Spire 5,
    Taris 10, Dantooine 15, Kashyyyk 20, Manaan 25, Korriban 30, Tatooine 35, Leviathan 40,
    Unknown World 45, Star Forge 50 (the Ebon Hawk's arrival scripts); the Ebon Hawk keeps the
    planet it left. KotOR II never sets it and is left alone.
  - A spot clicked on the map keeps out of the area's ways out, as the offered exit spots do:
    clicked at the middle of a way out, 17 of 17 tested landed inside the trigger.
  - With two installs of one game, a save reads the game's data from the install that holds it.
  - Stored copies of areas keep each door's and trigger's own tag.
  - The Go Back pop-up says what it does: the chapters' items are taken back up to the number
    they give, bought ones of the same kind too, and companions' XP is not rewound.
  - On KotOR's Endar Spire the Character tab says that the game sets XP and level itself there
    (the maintainer's untouched start save, 150 XP in the file, shows 3,080 in the game).
  - Finish Quest and Skip to stage take a quest's stages in the story's order, as the Quests
    tab lists them: Carth's quest at 3 (his revenge, on the Leviathan) no longer replays his
    talks 10 to 70, nor pays their journal XP again. Only stages set in a later chapter than
    every stage numbered above them count as out of order (Carth's 3, Bastila's 1 and 2), so
    the Quests tab no longer shows KotOR II's RebuildHK47 stages 20 to 71 as passed at 10.
  - The Finish Quest pop-up lists what the editor does apart from the steps only the game can
    run ("Left to the game", once "Happens in the game"), and says the game may skip those
    now that the quest is further on.
  - The tabs are spread over the whole bar.
  - Go Back can keep your items and credits: a switch under "Inventory changes", off when the
    pop-up opens. On, nothing is taken back, and what a companion who leaves has on goes to
    the party inventory. A companion's starting gear the game has not made into items yet is
    no longer listed as an inventory change.
  - Add a Class and Change Class offer only the classes the game gives: no Minion (the class
    of ordinary enemies); KotOR II's Tech Specialist, Bao-Dur's class, to party members only;
    a prestige class only to a player with one Jedi class at level 15 or above, as Kreia
    offers it, with its side in the note. Party members get no prestige class.
  - A window smaller than 1280 by 780 is drawn smaller as a whole: text, pictures and tables
    shrink together and keep their places. At 150% scaling on a 1920x1080 laptop the window
    ran off the bottom of the screen.
  - A spot clicked on the map keeps out of the area's objects too (each placeable's walkmesh):
    clicked on 2,186 placeables, 2,125 would have put the player inside one.
  - Advanced, Creatures asks before removing a creature. Editing a party member no longer
    changes a copy of them the editor spawned.
  - The audit's smaller findings (2026-11-08):
    - Revert asks first. Skip Ahead and Go Back show what they cannot do before Apply, and a
      fault while working one out is shown instead of "Calculating" forever. A story move is
      worked out again when the save was edited after it was asked for.
    - KotOR's star map quest names the map found last, as the game's script sets it, and a
      quest stage the game may lower is lowered.
    - The party: a member who stays keeps the leader flag, and a member the story keeps out
      is not put in the party. Change Class swaps the classes' own feats. Removing a class
      KotOR II added at level 0 no longer takes feats. Unequip refuses when the inventory
      did not load, where the item was lost.
    - Said, not refused: a feat added without its prerequisites, XP below what the level
      needs, and a portrait that changes the gender.
    - A renamed companion's name is written for the game's own language too.
    - A move to a stored area keeps the game's clock going on. Planets show on only when the
      galaxy map has both of their bits; turning off the selected planet clears the
      selection. Another area's map picks the floor by its own markers' height.
    - A game folder the editor cannot use is refused with the reason. Backups go to the
      editor's own folder when the game folder cannot be written.
    - Long pop-up messages scroll, Settings scrolls with Done in view, and KotOR II's Party
      row is tighter.
    - Game files: a damaged table no longer makes the editor set aside gigabytes, labels
      and names are written back byte for byte, a name with more than 64 translations
      keeps them all, and a damaged Screen.tga is bounded. A module name from a save must be
      a plain name, so it cannot lead out of the game folder.
    - Saves and installs: a network install opens in Explorer, older Steam library lists
      and Steam's install path in the registry are read, a save is listed once however its
      folder is spelled, and a KotOR II save in KotOR's folder is no longer listed there.
  - Every row of the story pop-ups' change table has an icon. The new ones are the game's own,
    read from the open game: Health the Cure power's, Force KotOR II's gain and loss icons
    (KotOR's Force Sensitivity icon), influence KotOR II's gain and loss icons, and class,
    attributes, skills, components and chemicals the game's Character, Abilities and
    Inventory menu icons.
  - Checked in KotOR II: quest XP Finish Quest puts in the party's pool reaches the
    companions, as in KotOR. The game raises them to 80% of the pool when the save loads.
- **Shorter, plainer text everywhere** (2026-11-07, the maintainer: "short, clear, simple
  language"). About 75 descriptions, notes and messages are rewritten to say the same in
  fewer, plainer words: the History and pop-up notes, the save and folder messages, the
  pickers' hints, the timeline's travel text, the crash and error messages. Settings now
  says the save is backed up when it is opened.
- **Revert to a backup from History** (2026-11-07, the maintainer: "a safe feature ...
  impossible to break"). Pick a backup in History and press Revert to This Backup, then
  confirm. The save as it is is backed up first, so a revert can itself be undone. The
  backup's files are staged, flushed and read back before any file is replaced, and the swap
  puts every old file back if one step fails. The result is compared with the backup byte for
  byte. A folder that is not one of this save's backups, a damaged backup, one of the other
  game, a running game and an Xbox save are refused with nothing changed. Tested on copies of
  saves from both games (`tests/restore.rs`), including a file held open part way through.
- **The backup is made when a save is opened, and checked before saving** (2026-11-07, the
  maintainer). Opening a save copies it to its backup folder and reads the copy back; an
  unchanged save keeps its newest backup instead of piling up copies. Save Changes first
  checks that backup against the save on disk, byte for byte and without writing, and makes
  a new one where it is missing, damaged or out of date, so every write still has a checked
  copy of exactly what it replaces. The pop-ups say "your save was backed up when you
  opened it". A History button between Copy and Settings lists the save's backups oldest first.
- **The story pop-ups grow with the window, and a skip's description is short** (2026-11-07,
  the maintainer). Finish Quest, travelling on the timeline and Go Back are drawn for a window
  of 1600 by 900 and now grow in step with a larger one instead of sitting small in its middle.
  A skip's description says in a line where it ends and which chapters are played first; the
  XP, credits and loot it repeated are in the change table under it, and the line saying it was
  worked out from the game's files is gone. In the travel pop-up the inventory changes follow
  the change table in the same scroll, with no gap. The change table's icons
  are the games' own notification icons traced into vector outlines, so they match the
  game's and stay sharp at any size, in each game's colours.
- **The story pop-ups look like the game's own menus, and filled buttons have round corners**
  (2026-11-07, the maintainer). Finish Quest, travelling on the timeline and Go Back sit in a
  framed dialog with a gold title, framed panels and lit section headings; the changes are a
  table with an icon on each row (XP, credits, level, party, alignment, the journal, items),
  drawn by the program in each game's own colours. "What only the game does" reads "Happens in
  the Game". Every button, switch and bar filled with a gradient had square corners inside its
  rounded edge (the renderer does not round a gradient); now they are round, and the selected
  half of a switch no longer shows a fuzzy rim where its fill met the outline. Looked at in the
  program in both games at 1600 by 900, and in KotOR at 1280 by 780.
- **Light and dark endings are marked, and changes have one colour language** (2026-11-06,
  the maintainer). Finish Quest tags an ending LIGHT SIDE or DARK SIDE where its own stage
  moves alignment that way. Wherever a change is shown before it happens (Finish Quest, Skip
  Ahead, Go Back), what goes up is green, what goes down red and the value after gold;
  "Added" and "Removed" follow. The Quests tab's right side is restyled in the editor's own
  grammar (section labels over plain text, outline buttons, actions along the bottom), and a
  stage's XP is green. Looked at in the program at 1600 by 900 and 1280 by 780.
- **Finish Quest plays the quest to its end, and shows what you receive first** (2026-11-06,
  the maintainer). It is always as if played from where the save stands: one way through each
  stage left, the one that suits the side, with its items, credits, XP, alignment, story values,
  doors, the journal quests it removes and the counters it raises, and the journal's own XP for
  each new stage. The pop-up shows the endings in full, asks the side where it changes
  anything, and on the right lists what the save receives, measured on a copy (values, items,
  companions, feats, powers, journal changes) and what only the game does; Finish takes exactly
  that. The "As if played" switch is gone. Quest stages the game sets out of number order
  (Carth's "revenge is complete", set on the Leviathan) are listed where they happen instead of
  first, and are no longer offered as the quest's beginning. `tests/finish_quest.rs`: 136
  finishes on the saves of both games. In the game, **untested**.
- **Pazaak & Destinations is a tab of its own, and Story is Quests** (2026-11-06, the
  maintainer). The planets the Ebon Hawk can fly to and the Pazaak cards have their own tab
  beside Quests, with its own mark for changes; Quests is the quest tracker alone. On a narrow
  window the new tab reads "Pazaak & Dest." (its whole name is shown wherever it fits,
  measured; "Pazaak" where neither fits, as with the largest text on a small window) and
  the cards go five a row so the planets keep their whole title; the cards' counts are never
  cut. The tabs are a little smaller so all nine fit. Hints that sent players to "Story,
  Quests" and "Story, Creatures" name the right places. Looked at in the program at 1600 by
  900 and 1280 by 780.
- **KotOR II's feats and powers can be added and removed again** (2026-11-06, the
  maintainer: "abilities in kotor 2 dont work to add or remove"). A long name in the lists
  (in German, "Geschwindigkeitsschub") made every row wider than the list, so the + and x
  column was off its right edge; the list now keeps its own width and a long name wraps.
  Measured in the program on a KotOR II save: a feat and a power added and removed, the
  counts 25, 26, 25 and 8, 9, 8. A count could stay on screen after it changed (adding a
  feat and a power and removing both left "FORCE POWERS 9" with 8 in the save until the
  window was resized; Slint's software renderer did not redraw it), so the whole window is
  drawn again after every edit.
- **KotOR II's Equipped is KotOR's 3 by 3 grid** (2026-11-06, the maintainer). The second
  weapon set takes the weapons' places when "Weapons 2" is picked under the grid, as the
  game's swap button does; it was a fourth row that made every slot small. Credits,
  components and chemicals sit side by side to give the slots room. In both games the worn
  items fill their frames and the grid is centred in its space; KotOR II's empty-slot
  pictures stay inside their frames. Looked at in the program in both games at 1600 by 900
  and 1280 by 780.
- **The Endar Spire, the Leviathan and the Ravager are drawn as themselves** on the
  Timeline (2026-11-06, the maintainer's pictures): each in place of the outline of a ship, a
  little larger than its ring; the Leviathan made greyer than its picture. In
  `assets/timeline`, cut from their backgrounds and credited in the README. Looked at in the
  program at 1600 by 900 and 1280 by 780.
- **Every switch and button shows its edge** (2026-11-06, the maintainer: "the edges of the
  button seem invisible"). The two- and three-way switches (the game, the quest list's
  filter, Settings) draw their outline over their choices, where the picked one's fill hid it,
  and round only their outer corners, so a choice's fill is the square a click reaches. Revert
  and a greyed-out Save Changes, with its arrow, have an edge at rest.
- **The Timeline map is the page** (2026-11-06, the maintainer's review). No panel around it
  and nothing above it: the map is 624 px tall at 1600 by 900 (it was 380 before the Timeline
  had its own tab). Where the save is and how to use the map are centred small print along
  its top; the zoom is in its top right corner. The planet order is inside it along the
  bottom, centred and only as wide as it needs, the buttons on the map with no bar behind
  them; Undo order changes takes the place of "Skip Ahead follows it" under its name (on a
  narrow map it floats just above), so nothing moves when it comes. The numbers in the
  planets' and the order's circles are centred: the title font set its digits 2 to 2.5 px low,
  now within half a pixel (measured). Stops whose names and tags ran into each other at 1280
  by 780 moved a little: KotOR's Endar Spire, Taris, Leviathan and Tatooine, KotOR II's
  Ravager and Malachor V. Looked at in the program in both games at 1600 by 900 and 1280 by
  780, before and after a reorder; in the game, **untested** (nothing here changes what is
  written).
- **The Timeline zooms and moves** (2026-11-06, the maintainer: "make the timeline zoomable
  and movable with mouse"). The wheel zooms in and out about the pointer, from the whole map
  to four times its size; dragging an empty part of it moves it, never past its edges; a
  double-click, or Fit, shows all of it again, as does opening another save. Buttons for
  out, in and Fit sit in its bottom right corner. A planet clicked while zoomed opens as
  before. The ring a tag of many saves opens into stays inside the map (sixteen saves at the
  Endar Spire ran off its top). The rename button's disk is centred: an empty label beside
  it had pushed it 6 px left. The map keeps its size when a planet is moved: Undo order
  changes, appearing under it, made its line taller and the map gave up the difference (at
  short windows the whole line appeared); the line now keeps the button's room hidden.
  Looked at in the program at 1600 by 900 and 1280 by 780 in both games, the map's corners
  measured at the same rows before and after a move; in the game, **untested** (nothing here
  changes what is written).
- **Timeline is its own tab** (2026-11-06, the maintainer's review). The galaxy map moved out
  of Story into a Timeline tab beside it, with no section tabs above it, so the map is about a
  fifth taller (380 px to 453 px at 1600 by 900). Story now holds Quests and Planets, Pazaak, and opens on Quests. The
  Timeline opens at once: it is filled when the save opens and the other saves are read for
  it in the background then, and its route is worked out in 0.08 ms instead of 145 ms
  (`questdb::chapter_quests` copied every chapter's file on each call); the tab's fill fell
  from about 600 ms to 26 ms, measured in the program on a KotOR save. A save clicked on the
  map no longer opens straight away: a warning names it, its character, level and story, says
  the save open now closes, and offers Cancel. Feat and power rows are as tall as their names
  (a name on three lines was cut). The rename button's disk icon is redrawn larger. Health and
  Force boxes hold four digits at Large text (944 and 1056 were cut). The quest filter fits at
  1280 px. Looked at in the program at 1600 by 900 and 1280 by 780 with Large text; in the
  game, **untested** (nothing here changes what is written).
- **The audit of 2026-11-06** (two rounds, each finding checked in the code, measured where a
  test reaches it: `tests/audit_round2.rs`, `tests/exits.rs`, `tests/galaxy_route.rs`,
  `tests/level_grants.rs`, `tests/source_text.rs`, `tests/audit_fixes.rs`). Writing: new files reach
  the disk before old ones are replaced, a failed swap that cannot put the old file back says
  so, a save left part written is refused with the way back from its backup (a finished write
  whose clean-up failed still opens), and backups never land in a `saves` or `cloudsaves`
  folder. Go Back keeps the star map values the game would keep, handles several begun
  planets, and no longer reads undone planets as played from the area the save still stands
  in. A found star map counts as played, so Skip Ahead never plays a planet twice; KotOR II's
  Ebon Hawk scenes with G0-T0 wait until he has joined. A quest stage's XP is given once.
  Taking a companion's second class away keeps what the first gives. Typed positions refuse
  `nan` and `inf`; Health and Force boxes ignore a non-number. A way out's spot is outside its
  trigger. Pazaak shows the game's cards only. Pop-ups close with Escape. A background result
  arriving while a folder dialog is open waits instead of ending the program. The saves list
  keeps the save picked when it is re-sorted. Every page fits the 1280 by 780 window. **In
  the game, untested.**
- **The timeline is a galaxy map** (2026-11-06). Story opens on a map of the galaxy: each
  chapter at its planet, as the game's own map places them, with the game's planet pictures;
  bands of blue light join the planets in the order the save played them, faint lanes run on
  along the route ahead, and the save's stop has a gold ring with the Ebon Hawk beside it. A
  click on a planet opens its quests as before. The other saves are tags at the furthest stop
  each has played. **The order of the free planets can be picked**: arrows under the map move
  Tatooine, Kashyyyk, Manaan and Korriban (KotOR II: Nar Shaddaa, Dxun and Onderon, Dantooine,
  Korriban) earlier or later, and Skip Ahead and Go Back follow that order; a planet picked
  first is skipped to alone. The Leviathan comes after the third free planet, as in the game.
  A chapter before a played one on the story's line now reads played, though the game took
  its quests out of the journal (a save after the Leviathan read Dantooine as never played).
  The Galaxy Map page that opens and closes planets is now Planets. `tests/galaxy_route.rs`;
  looked at in the program; in the game, **untested**.
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

