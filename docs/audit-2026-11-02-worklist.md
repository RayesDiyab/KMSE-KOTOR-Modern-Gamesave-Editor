# Audit worklist, 2026-11-02

> **Documentation standard.** This document follows
> [`documentation-standard.md`](documentation-standard.md). Read it before editing this
> file, and check the result still meets it: measured claims only, rejected alternatives
> and corrections kept visible, and anything untested labelled as untested.

All 55 findings of [audit-2026-11-02.md](audit-2026-11-02.md), in the order to work them
off. This page holds only the order and the state of each finding. The audit keeps the
trigger, the evidence and the numbers, so a finding is explained in one place only.

The order is the maintainer's working order, not a measurement. Parts 1 and 2 are what was
judged to block a release on 2026-11-02. The issue numbers are the audit's, so they do not
run in sequence here. Marks: **M** measured, **S** checked against the source, **R**
reported by one review and not checked again.

## How to work it

1. Pick the first open box and read its row in the audit.
2. Reproduce it where the audit marks it **R** or calls it a trace, before changing code.
3. Fix it, add a test where a test can reach it, and update `CHANGELOG.md` and the documents
   the change touches.
4. Tick the box and add the date and how it was checked, for example
   `2026-11-03, tests/star_maps.rs; in the game, untested`.

A finding that turns out to be wrong stays on the list, ticked, with a line saying what was
true (documentation standard, rule 11). A finding left alone on purpose does the same, with
the reason.

Progress: 55 of 55 ticked on 2026-11-02; 78 tests pass; build 13,759,488 bytes, SHA-256 BF4285A9F4060D6125B3F68B24FBF1E4356A6E2D11480AA679C11E2BE4F8B1D9. Open: none. Nothing here has been tried in the game; "traced" means read through the code, with no test and no click.

## Part 1: lost work or a damaged save (11)

- [x] **1** (S) A click on a save on the timeline drops unsaved changes. 2026-11-02: refused with the same message as Back; traced, not clicked
- [x] **2** (S) Closing the window drops unsaved changes. 2026-11-02: the first close asks, a second with the same changes leaves; traced, not clicked
- [x] **3** (S, traced) Cancel in Copy or Save as a New Save performs the action. 2026-11-02: `accept-on-blur: false`, as the rename field; traced, not clicked
- [x] **4** (S) Save Changes can write over a newer save made by the game. 2026-11-02: size and time of every file compared before a write; tests/audit_fixes.rs
- [x] **5** (S) A locked file leaves a half-written save and says "Nothing was changed". 2026-11-02: old files kept until all are in, put back on failure; a locked file, tests/audit_fixes.rs
- [x] **6** (M) Unequipping a template item loses it and adds a blank inventory entry. 2026-11-02: a template entry is made a whole item first, or left in place; tests/audit_fixes.rs
- [x] **7** (S) A map click on a zero calibration writes NaN as the position. 2026-11-02: `Floor::height`, `move_player` and `move_to_module` refuse it; tests/audit_fixes.rs
- [x] **8** (S) An item added to a container can reuse an object id. 2026-11-02: the id now comes from the module's counter; traced
- [x] **9** (S) Travel Here stacks events in the module's queue. 2026-11-02: one trip at a time, and a move clears it; tests/audit_fixes.rs
- [x] **10** (S) A failed Save as leaves the session on the new folder. 2026-11-02: restored and the new folder removed; tests/audit_fixes.rs
- [x] **34** (S) Any panic ends the program with no message and loses unsaved work. 2026-11-02: a message box and `crash.txt`; the ERF count, tests/audit_fixes.rs; the box itself never seen

## Part 2: story changes that break a run (7)

- [x] **27** (S) Skip and Go Back drop their notes (regression in the working tree). 2026-11-02: failures shown after Apply, success reports stay hidden; traced
- [x] **11** (M) Go Back leaves star maps and capture state behind. 2026-11-02: `restore_star_maps`; tests/audit_fixes.rs. A KotOR 2 value with no end value stays and is now reported
- [x] **12** (M) Go Back to the Leviathan leaves a dead end. 2026-11-02: the fourth planet is undone too, the last on the timeline, which is a guess; tests/audit_fixes.rs
- [x] **13** (M) A skip to a stage applies every way of that stage at once. 2026-11-02: `one_way`, side picks the way; tests/audit_fixes.rs. Which way a player took is not knowable
- [x] **14** (S) A skip to a side chapter plays every unplayed main chapter. 2026-11-02: `skip_path`; tests/audit_fixes.rs
- [x] **15** (S) Any Go Back also undoes played side chapters. 2026-11-02: `return_to`; tests/audit_fixes.rs
- [x] **17** (M) A dark run reads as unplayed. 2026-11-02: either side's journal counts; tests/audit_fixes.rs

## Part 3: the rest of the story system (10)

- [x] **16** (S) Go Back counts the current chapter as fully played. 2026-11-02: `unfinished` keeps what only a finished chapter gave; tests/audit_fixes.rs, also on KotOR 2 with its real tables
- [x] **18** (M) `close_quests` undoes `journal_remove`. 2026-11-02: tests/audit_fixes.rs
- [x] **19** (S) A stage skip inside KotOR 1 Dantooine ignores the Jedi class asked for. 2026-11-02: `apply_jedi` runs when the route passes `k_pdan_makejedi`; traced
- [x] **20** (R) `k_starforge` can make an earlier planet read unplayed. 2026-11-02: `k_starforge` left out on map planets; tests/audit_fixes.rs
- [x] **21** (R) Go Back can raise XP. 2026-11-02: never raised; tests/audit_fixes.rs
- [x] **22** (R) XP totals follow timeline order, not the order played. 2026-11-02: `xp_of` counts each chapter's share; tests/audit_fixes.rs
- [x] **23** (R) Go Back picks light or dark by current alignment. 2026-11-02: `played_dark` reads the journal, alignment only where it cannot tell; traced
- [x] **24** (R) Go Back leaves the Jedi lightsaber, `000_Prestige` and `TRAINABLE`. 2026-11-02: lightsaber, `000_Prestige` and the training values go back; tests/audit_fixes.rs for the class, `000_Prestige` and `000_Jedi_Found` on KotOR 2
- [x] **25** (R) KotOR 2 chapters need side quests to read played (unmeasured). 2026-11-02: KotOR 2 counts quests its critical path names. tests/audit_fixes.rs: a run made by skipping reads played. **No real late run**: the four KotOR 2 saves are early
- [x] **26** (R) Small quest data errors, for the data pass. 2026-11-02: a boolean write skips a number, the Jedi path stays the pick (tests/audit_fixes.rs), and a route stops at the milestone's stage

## Part 4: safety rules and refusals (6)

- [x] **28** (R) `certain: false` is honoured only for milestone globals. 2026-11-02: the skip popup says the data is unchecked, and uncertain values stay in the notes
- [x] **29** (S) A rename from the saves page backs up to the wrong folder. 2026-11-02: uses `backup_root` as Save Changes does; traced
- [x] **30** (S) `game_running` misses a failed `tasklist` and Proton. 2026-11-02: a failed check counts as running; `swkotor.exe` is looked for off Windows; traced
- [x] **31** (S) Copy has no read-only check for Xbox saves. 2026-11-02: Copy refuses an Xbox save; no Xbox save here to try
- [x] **32** (R) Read-only shows on one label; edits are refused only at Save Changes. 2026-11-02: Skip, Go Back and the other game-bound edits refuse at once, any other edit is undone with a message; traced
- [x] **33** (R) KotOR 1 backups sit inside `Saves`, which Steam Cloud syncs. 2026-11-02: moved at the maintainer's word: backups go to `<game>\Save Editor Backup`, outside what Steam syncs; tests/audit_fixes.rs. Backups already in `Saves` stay there

## Part 5: character, inventory and world (21)

- [x] **35** (R) An item equipped in the editor brings no effects (untested in the game). 2026-11-02: no code change is right. Read in both executables: the game applies an item's properties when it loads a creature and drops stored item effects; seven saves agree. Not yet seen in the game. The attributes the editor shows leave such an item out until the game saves
- [x] **36** (R) Import and recruit miss the copy standing in the area. 2026-11-02: new ids, and the standing copy follows; tests/audit_fixes.rs for the ids
- [x] **37** (R) Total level is not capped. 2026-11-02: class maximum and Add a Class respect the total; traced
- [x] **38** (S) Stealth Field Generators are missing from Add an Item. 2026-11-02: base item 88; tests/audit_fixes.rs. The first fix let claws through, caught by measuring the table
- [x] **39** (R) KotOR 2's second weapon set offers nothing to equip (unmeasured). 2026-11-02: the second set offers what the first does. Measured in KotOR 2's baseitems.2da: no row lists 0x40000 or 0x80000, so the second set has to borrow the first's
- [x] **40** (R) Equip ignores `weaponwield`. 2026-11-02: measured in both games' baseitems.2da, by `weaponwield` too: KotOR 1's Gamorrean Battleaxe lists the right hand alone yet takes one hand, which the first fix got wrong
- [x] **41** (R) The attribute floor of 3 also applies to KotOR 1; one source is wrong. 2026-11-02: the code was right and the document wrong: read in both executables, both floor at 3. KotOR 2's Dexterity getter was not read. Found on the way: type 37 decreases were ignored, now counted; the engine's caps and stacking are not modelled
- [x] **42** (R) Raising a level with pending XP lowers XP without a message. 2026-11-02: tests/audit_fixes.rs
- [x] **43** (R) Moving to another area keeps `PerceptionList` and `ActionList`. 2026-11-02: both lists emptied on the move; traced
- [x] **44** (S) Map notes never appear. 2026-11-02: `HasMapNote` and `MapNoteEnabled`; the field names are from the format, no note seen on the map
- [x] **45** (R) Open Every Door Here edits the current area, not the one on the map. 2026-11-02: the map's area, also Unlock; traced
- [x] **46** (R) The map goes stale after a Story removal or Revert. 2026-11-02: the map is read again after Revert, Skip and Go Back; traced
- [x] **47** (R) A reverted spawn stays in `spawned.txt`. 2026-11-02: a reverted spawn's line leaves `spawned.txt`; traced
- [x] **48** (R) Boxes stop following the value after typing. 2026-11-02: Stepper follows its value, and the other boxes read theirs on each fill; not clicked
- [x] **49** (R) A non-number in XP or Credits becomes 0. 2026-11-02: a non-number leaves the value as it was; not clicked
- [x] **50** (R) A deleted save shifts the other-saves list. 2026-11-02: gone saves leave the list before the rows are made; traced
- [x] **51** (R) The party list is stale after Skip or Go Back. 2026-11-02: everything is filled again after Skip and Go Back; traced
- [x] **52** (R) A Copy of a save added on its own is not listed. 2026-11-02: a copy of a save added on its own is added too; traced
- [x] **53** (R) One invalid byte in the settings file resets all settings. 2026-11-02: read as bytes; traced
- [x] **54** (R) `LASTMODULE` with `..\` can read a file outside the game (read only). 2026-11-02: `ModuleFiles::open` refuses a name with a path in it; traced
- [x] **55** (R) Small items: see the audit's row for the list. 2026-11-02: picture rows, `stage_pick`, `equip_slot`, the box stepper, `AreaId` and three dead callbacks done. `PORTRAIT1` and `2` follow the party after a skip, a return or an import; traced. `PORTRAIT0`, the leader's, is left to the game

## Not numbered, also in the audit

The audit's sections on tests and on documents untrue of the code have no issue numbers, so
they are not among the 55. Work them from there, alongside the fixes they touch.

## What is deliberately not on this list

Mod content (`slem*`, `liv_m99*`) counts as known. Trying the changes in the game is a
release step, not a finding, and it has not been done: no finding has been tried in the
game. (This line said KotOR 2 is not installed; that was wrong, see the audit's correction.)

## Checking by hand

Count the boxes: `^- \[[ x]\] \*\*[0-9]+\*\*` matches 55 lines, one for each number from 1
to 55, and the number of ticked boxes matches the Progress line.
