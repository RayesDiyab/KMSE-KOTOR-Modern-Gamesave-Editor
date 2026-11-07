# KMSE Windows audit, 2026-11-07

> A **lab record**, held to the [documentation standard](../documentation-standard.md). It says
> what was inspected, what was found, what was fixed and how each claim was checked. What the
> editor writes, field by field, stays in [save-editor.md](../save-editor.md); the save format in
> [save-editor-format.md](../save-editor-format.md). The earlier audits are
> [audit-2026-11-02.md](../audit-2026-11-02.md) and [audit-2026-11-06.md](../audit-2026-11-06.md).

## A. Scope

| | |
| --- | --- |
| Audited | branch `master`, commit `cac2d4b` (clean tree), plus the fixes below |
| Environment | Windows 11 Pro 10.0.26300; Rust 1.99.1, cargo 1.99.1, clippy 0.1.99, rustfmt 1.10.0; cargo-audit 0.22.2 |
| Games on the PC | KotOR 1 (Steam, `C:\Program Files (x86)\Steam\steamapps\common\swkotor`, English); KotOR 2 (Steam, `D:\SteamLibrary\steamapps\common\Knights of the Old Republic II`, German) |
| Save fixtures | the PC's own saves, read only: 28 KotOR saves (Endar Spire to Star Forge, the editor's earlier output among them), 4 KotOR II saves (Peragus and Telos). Every test that changes a save works on a copy in the system's temporary folder |
| Source | 16,020 lines of Rust in `src/` (22 files), 6,088 lines of Slint in `ui/` (6 files), `build.rs`, `build.ps1`, `.github/workflows/build.yml`, 4,005 lines of tests before this audit |

**Method.** Five reviewers read their parts line by line in parallel, read only:
- the binary formats (`src/formats/*`, `text.rs`);
- the quest and story engine (`questdb.rs`, `story_preview.rs`, `quests.rs`, `globals.rs`, the story handlers in `main.rs`);
- character, inventory and party editing (`creature.rs`, `catalog.rs`, the edit API of `save.rs`, their handlers);
- world and filesystem (`map.rs`, `area.rs`, `stored.rs`, `games.rs`, `gamedata.rs`, settings and folder handling);
- the UI and its wiring (every `.slint` file and every `on!` handler).

The save pipeline (`save.rs`: open, write, backup, restore, and every caller in `main.rs`) was audited by the lead. Every finding a reviewer reported was checked against the code, and where possible reproduced, before it was fixed. A sixth reviewer then read the finished changes as unfamiliar code (section F).

**Excluded.**
- `tools/` (Python that builds `data/quests` and `data/xp` on the maintainer's machine; not shipped).
- `examples/` (development aids).
- The macOS and Linux paths beyond noting portability issues (section G).
- The games themselves: nothing was loaded in KotOR or KotOR II (section E).

## B. Findings

Severity:
- **P0:** save corruption, data loss, damage to files.
- **P1:** wrong story state, an unplayable save, a broken core feature.
- **P2:** a functional defect.
- **P3:** minor.

Game: K1, K2, or both.

### Fixed

| ID | Sev | Game | Where | What was wrong | Fix | Checked by |
| --- | --- | --- | --- | --- | --- | --- |
| S-1 | P1 | both | `questdb::one_way` | The data lists each path through one conversation line or script as its own step; all were applied together. K2 DroidRetrieval stage 20: 63 paths paid Kodin, 46,400 credits. 24 quests gave rewards several times | `one_way_in`: one step per set of alternatives (same way, same line, same kind of last condition); conditions on story values read as the run goes; else the first that does something. Corrected after the second review (section F) | `finish_quest_audit.rs`: every quest of both games, at most one step per set of alternatives; Kodin paid under 1,500; the XP of four stages equal to values worked out by hand from the data |
| S-2 | P1 | K1 | `questdb::finish_quest` | Finishing a star map quest set all four maps, opened the Unknown World, queued the Leviathan capture | Star map branches dropped as Skip does; maps counted the game's way | `finishing_a_star_map_quest_finds_one_map`, 24 finishes; fails without the fix ("4 maps found, 0 before") |
| S-3 | P1 | both | `finish_quest` | Recruits (`AddAvailableNPCBy...`) and `RemoveAvailableNPC` were only listed as "left to the game" | `recruits_in` as Skip does; `RemoveAvailableNPC` lets the member go | `finishing_escape_merchant_recruits_mandalore`; fails without the fix |
| S-5 | P2 | both | `finish_quest` | A quest at an ending already could be finished again: the ending's items, credits and step XP twice, and a lower ending moved the journal back | Refused when the quest is at an ending or at the stage asked | `a_finished_quest_is_not_finished_again`, 91 quests |
| B-1 | P1 | both | `save::backup_list`, `restore` | Backups were matched to a save by folder name alone. Two saves with one name (two Steam accounts' cloud saves, a copy inside the game folder) shared History, and Revert accepted the other's backup | `<backup>.source` record names the save by its place in the game folder; list and restore check it; a backup that cannot be made whole, record included, is removed; legacy backups without one are still listed | `a_save_of_the_same_folder_name_never_gets_the_others_backups`; `backups_stay_the_saves_own_after_the_game_folder_moves` |
| U-1 | P1 | both | `on_save_changes` | A half-written save was told only in the status line, cut to one line, without the backup's place, and cleared at the next edit | Pop-up naming the checked backup | code read; the failure itself is covered by `write_faults.rs` (no half write happens in any injected case) |
| X-1 | P2 | both | `Save::write`, `write_as`, `restore` | The Xbox refusal lived only in the window's handlers | Refused in `save.rs` too | `an_xbox_save_is_never_written` |
| F-1 | P2 | both | `gff.rs` `Reader::field_data` | A GFF naming one large field many times copied it each time (2 MB asking for 256 GB); the save list reads every `savenfo.res`, so the editor stopped each time | Copy budget: four times the file plus 1 MB | `made_up_files_asking_for_gigabytes_are_refused`; `roundtrip.rs` shows no real file refused |
| F-2 | P2 | both | `erf.rs` `Erf::read` | Archive entries covering the whole archive copied it once each (3 MB asking for 298 GB) | Entry sizes together may not exceed the archive | same tests |
| F-6 | P3 | both | `Erf::read` | A description block outside the file was dropped but its count kept, so the archive was written back inconsistent | Empty block accepted; a non-empty one that does not fit is an error | `roundtrip.rs` (no real archive refused) |
| W-17 | P2 | both | `save::copy_dir`, `same_files` | A junction or link inside a save folder was followed by backup, Copy and Save As; a loop recursed until the program fell over | Links refused; never taken as "the same" | `a_link_inside_a_save_folder_is_never_followed` (a real junction) |
| W-13 | P2 | both | `Settings::store` | Written in place; if reading had failed, the defaults replaced the player's added folders at exit | Written beside and moved over; never written when it could not be read | code read |
| W-16 | P3 | Windows | `game_running`, `open_path` | `tasklist` and `explorer` run by bare name; Windows looks in the program's folder first | Full paths under `%SystemRoot%` | code read; the game check still ran in every Save Changes test of this audit |
| E-1 | P2 | both | `on_set_attribute`, `on_remove_feat` | Lowering Constitution (Wisdom, Charisma, Toughness) left a hurt character at 0 Health or below | `creature::keep_in_range` | `lowering_constitution_never_leaves_a_hurt_character_dead`: without it -218 (K1) and -6 (K2) |
| E-2 | P2 | K1 | `Save::import_npc` | Bring from Another Save put whoever held the slot there: Trask into Bastila's slot, recruited | Another person only from a save at least as far along the story (chapters played) | `the_endar_spire_stand_in_never_replaces_bastila` (real saves); `a_companion_from_another_save_must_be_the_same_person` |
| U-2 | P2 | both | close handler | After one warning, later edits to the same values closed without asking | Warned again after any edit, keyed on the opening and its edit count (`Save::session`, `Save::edits`) | code read |
| U-3 | P2 | both | `refresh_after_edit` | An old "Saved" or error line hid "N changes, not saved yet" | Cleared when the save's edit count moved, not on a tab switch | code read |
| U-4 | P2 | both | `on_do_skip`, `on_do_back` | The old member row was put back after the list was sorted again: one member's tabs under another's name | Only the tab is kept | code read |
| U-5 | P2 | both | `on_select_tab`, `open_selected` | The feat and power search stayed under an empty box | Cleared with the box | code read |
| U-7 | P3 | both | `on_finish_quest` | After a finish the member list and the map were not read again | Read again, as after Skip | code read |
| D-1 | P3 | build | `Cargo.lock` | `pulldown-cmark` 0.13.5, RUSTSEC-2026-0433 (quadratic parse of nested wiki links). Reached through Slint; the editor never enables wiki links | 0.13.6 | `cargo audit` |

### Found in the game, after the report (2026-11-07 afternoon)

| ID | Sev | Game | Where | What was wrong | Fix | Checked by |
| --- | --- | --- | --- | --- | --- | --- |
| G-1 | P1 | K1 (both by the code) | `questdb::skip_core`, `go_back_part_played` | The party table's leader flag was carried through a story move. "KMSE back from Dantooine", made from the Endar Spire "test" save, kept slot 0 marked as the controlled member (Trask there, Bastila after the move). The game queued a switch to that member on every area load, and after the Taris turret game it crashed in `CrownFutureLeader` (`0x005FB803`, a null creature). **First reported here as a crash in the rumble code: wrong.** KMRP's agent found the real cause and confirmed it in the game: the same save with the flag cleared played on to Dantooine | `Save::release_controlled_npc` on Skip Ahead and Go Back | `a_story_move_leaves_the_player_leading` (8 Endar Spire saves; fails without the fix) |
| G-2 | P2 | K1 | `Save::queue_transition`, `move_to_module` | Travel leaves `K_CURRENT_PLANET` as it was: after travel from Manaan to Dantooine, Juhani's "what do you know about this world" spoke of Manaan (the maintainer, in the game). The values the game writes, measured in its saves: Endar Spire 5, Taris 10, Dantooine 15, Manaan 25, Tatooine 35, Unknown World 45 (the quest data agrees) | `Save::set_planet_for`; Kashyyyk, Korriban and KotOR 2 left as they are | `travel_sets_the_current_planet` |
| G-3 | P3 | K1 | Character tab | On the Endar Spire the game sets the player's XP and level itself as the save loads: the untouched "test" save, 150 XP in the file, shows 3,080 and level 3 in the game (the maintainer). XP and level edits there do not show | a note under Experience on the Endar Spire | the maintainer, in the game |

### Measured, not a defect

| | |
| --- | --- |
| GFF layout | Every GFF of the 32 saves (1,692 files with the archives and their contents) reads back as the same structure after writing, and writing twice gives the same bytes. 975 GFF files are not byte-identical to the game's: the writer numbers structs in another order. Every section keeps its size and count (measured on `GLOBALVARS.res`: first difference at byte 72, inside the struct table). The game reads structs through offsets. Asserted by `tests/roundtrip.rs` |
| Archives | Every `SAVEGAME.sav`, and every area save inside it, is written back byte for byte. The one exception is the description-strings offset when there are no strings: one save had 0 there, the editor writes 160. Nothing reads that field |
| Transactions | See section D |

## C. Test matrix

Statuses:
- **VERIFIED:** a test or measurement with evidence.
- **PARTIAL:** some aspects tested.
- **BLOCKED:** needs what this PC or session cannot provide.
- **FAILED:** a known defect remains.
- **NOT TESTED:** nothing checked.

No row is VERIFIED for gameplay: nothing was loaded in a game (section E).

| Feature | KotOR 1 | KotOR 2 | Evidence |
| --- | --- | --- | --- |
| Reading saves (all 32 here) | VERIFIED | VERIFIED | `roundtrip.rs`; `parser_fuzz.rs` (16 files damaged 1,500 to 3,000 times each, no panic) |
| Save Changes, writing | VERIFIED | VERIFIED | `write_faults.rs` (locked file mid swap, staging blocked, no backup possible, changed on disk, Xbox, read-only file, no change, 8 writes in a row); `edits.rs` |
| Backup on opening, check before saving | VERIFIED | VERIFIED | `backup_on_open.rs`, `write_faults.rs` |
| History list, Revert to This Backup | VERIFIED (backend) / PARTIAL (window) | VERIFIED (backend) / PARTIAL (window) | `restore.rs`, 9 tests incl. a held file and same-name saves; the pop-up seen in the program, a real revert not run on a real save |
| Save As, Copy, rename | PARTIAL | PARTIAL | `save_as.rs` |
| Attributes, skills, Health, Force | PARTIAL | PARTIAL | `edits.rs`, `audit_2026_11_07.rs`; E-3 remains |
| Classes, levels | PARTIAL | PARTIAL | `levels.rs`, `level_grants.rs`, `audit_2026_11_07.rs`; E-3, E-4 and E-5 to E-15 fixed |
| Feats, powers | PARTIAL | PARTIAL | `abilities.rs`; E-10 answered (prerequisites said, not enforced) |
| Inventory, stacks, credits | PARTIAL | PARTIAL | `edits.rs` |
| Equipment | PARTIAL | PARTIAL | `audit_fixes.rs`; E-8, E-9 remain |
| Party, recruit, import | PARTIAL | PARTIAL | `audit_fixes.rs`, `audit_2026_11_07.rs`; E-6, E-7 remain |
| Influence, components | NOT TESTED | PARTIAL | `edits.rs` (influence) |
| Finish Quest | PARTIAL | PARTIAL | `finish_quest.rs` (84 + K2 finishes prepared), `finish_quest_audit.rs`; S-4 and S-7 fixed later; S-6 answered in both games |
| Skip Ahead, Go Back | PARTIAL | PARTIAL | `story_safety.rs`, `star_maps.rs`, `galaxy_route.rs`, `audit_fixes.rs`; S-8 remains |
| Map, travel, spawn | PARTIAL | PARTIAL | `audit_round2.rs`, `exits.rs`, `audit_2026_11_07.rs`; W-1 to W-5 fixed |
| Pazaak, destinations | PARTIAL | PARTIAL | code read; W-8 remains |
| Advanced (globals, objects, creatures) | PARTIAL | PARTIAL | `edits.rs` (a global flipped); W-2 remains |
| Window at 1600x900 and 1280x780, 100% | PARTIAL | PARTIAL | seen in the program this session (Finish Quest, travel, History) |
| Window at 150% scaling on 1920x1080 | VERIFIED (shrinking) | VERIFIED (shrinking) | U-6 fixed: a window smaller than 1280 x 780 is drawn smaller as a whole (`fit_to_window`); run at 1100 x 680 and 860 x 530 on this PC (100% scaling), everything in place. Not run on a 150% screen |
| 1920x1080, 2560x1440, 3840x2160, 125%, 200% | NOT TESTED | NOT TESTED | not run this session |
| Loading edited saves in the game | PARTIAL | PARTIAL | section E: two Finish Quest saves loaded and played in KotOR; nine more made for the maintainer; in KotOR II one Finish Quest save loaded by the maintainer (S-6) |

## D. Save integrity

**What a write does** (`Save::write`, `write_files`, `replace_all`):
1. It refuses a `pifo.ifo` save, an Xbox save, and a save changed on disk since it was opened.
2. It checks the backup made on opening against the save, byte for byte, read only. If the backup is missing, damaged or out of date, it makes a new one and reads it back.
3. Each changed file is written to `<stem>.kmrp-new` and flushed (`sync_all`).
4. Each old file is renamed to `<stem>.kmrp-old`, and its new file renamed into place.
5. The old files are removed.

A failure in step 4 puts every old file back. A file that cannot be put back is named in the error, which then starts "The save may be half written" and opens a pop-up naming the backup.

**Measured under injected failures** (`write_faults.rs`, both games):
- a save file held open, so it can't be renamed, first and last in the swap order;
- a folder in place of the staged file;
- a file where the backups folder goes;
- the save changed on disk;
- an Xbox save;
- a read-only file.

In every case the save is byte for byte as it was, nothing of the write is left behind, the changes stay in memory, and the same write goes through once the cause is gone.

**Revert to a backup** (`save::restore`) has the same steps. It backs up the current save first and checks the result against the backup byte for byte. `restore.rs` adds a held file part way through, nine kinds of refusal, same-name saves, and six reverts back and forth.

**What Windows does not guarantee:**
- A rename is atomic for one file. A write of several files is not: a power cut between two renames leaves some new files and some old. The next open sees the `.kmrp-new` or orphaned `.kmrp-old` files and refuses the save, naming them and the backup to copy back. It does not repair the save itself.
- The renames are not flushed (Windows has no directory fsync for this API). The data of each new file is flushed before any rename.
- All files of a save are in one folder, so no rename crosses volumes. Backups are copies, so their volume does not matter.
- A virus scanner or indexer that holds a file open makes the write fail cleanly (the held-file tests). It cannot make it half.

**Backups:**
- Made on opening, reused when the newest one is identical, and checked before every write.
- Never removed by the editor, and never inside a folder Steam Cloud syncs (`backup_root`).
- Since this audit, each one records its save.
- They sit in the game folder, so uninstalling the game through Steam removes them (W-18).
- Backups grow without limit.

**Remaining risk:**
- The multi-file non-atomicity above.
- Settings and `spawned.txt` written by two copies of the editor at once (last writer wins).
- Backups made before this audit have no record, so two same-name saves still share those.

## E. Gameplay verification

**First pass (no game run).** Every gameplay claim rested on the save data, the game's own
files, the quest database and the tests.

**Second pass, 2026-11-07 afternoon, at the maintainer's request ("run the game yourself").**
KotOR (Steam, with KMRP installed) was driven with KMRP's own live harness
(`testing/controller/live_harness.py`, `live_session.py`): a virtual pad only, the open
screens read from the game's memory, frames taken with ffmpeg's `ddagrab`.

| Save, made by `examples/ingame_fixture.rs` | Loaded | Result |
| --- | --- | --- |
| BOS_SAVE + Finish Quest, Genoharadan: Ithorak ending 99 | 4.4 s | the HUD in play on the Ebon Hawk, Carth there, the game still running 15 s later; the frame read |
| BOS_SAVE + Finish Quest, Star Map: Manaan ending 40 | 4.6 s | the same; one map added (the fixture's read-back), not four |

Three things were learnt about running the game this way:
- KMRP's test copy (`build/verify/steam`) does not start: Steam refuses it ("Application load
  error 5:0000065434"). The real install starts.
- Steam starts `swkotor.exe` again under a new process id. The harness kept the first id and
  sent nothing; a wrapper in the session's scratch folder follows the new one.
- The game's Load Game list is in slot order with the Auto Save first, not newest first.
  KMRP's `load_newest_save` therefore loaded the Auto Save. That is in `sle_m30aa` of the
  Sleheyron mod, which the game left at once (no crash report).

Seven more loads failed before reaching the list: the pad presses landed on the intro
movies, or the game was not in front. They are not results.

At the maintainer's request ("I will test myself"), nine test saves were then made from two
saves the game wrote: "test" (Game23, the Endar Spire, level 1) and "Trial succcess" (Game22,
Manaan; game-written, but level 40 with 99,919,498 credits). They are in the real `Saves` as
"KMSE ..." slots: Finish Quest (Attack on the Endar Spire; Missing Selkath), Skip Ahead to
Dantooine, Go Back from it, level 10, a lightsaber added, travel to the Dantooine Enclave,
Carth from a Taris save, and a revert. Each was read back by the editor's code: values as
intended. Loading them in the game is the maintainer's.

For S-6 two KotOR II saves were made the same way from the maintainer's Game3 (Telos, level
8), in `cloudsaves` as "KMSE S-6 copy" (unchanged) and "KMSE S-6 fuel" (the fuel quest
finished), their backups in the game folder. The maintainer loaded the fuel save: Atton and
Kreia at 28,993, 80% of the pool rounded up, against 28,258 and 28,257 in the file; the
player at 34,738, as written. The copy, for a companion outside the party (T3-M4, 23,781 in
the file), was not loaded.

These remain unverified in the game:
- whether the path Finish Quest takes, where the save cannot decide, is one a player can reach;
- what the engine does with the state Skip Ahead and Go Back write in KotOR II (the maintainer's notes say KotOR II's story was never played through);
- whether the game reads the values Finish Quest sets when it first builds an area.

## F. Second review

The finished changes were read again as unfamiliar code by a separate reviewer.

It found that two of the fixes were themselves wrong, and some tests too weak. Each finding was
checked against the code and the data, fixed, and tested again:

| ID | Sev | What the review found | What was done | Checked by |
| --- | --- | --- | --- | --- |
| R-1 | P2 | The first S-1 fix kept one conditioned step per way. One way's steps are often different points of one run (the node's script, the line's own test, the line after a reply), so real rewards were dropped: K2 tutorial_3CFD stage 90 went from 220 XP (doubled) to 0, not 110; MedBayMur, dan_murder, kor35_doublecross2 lost XP; 47 stages lost local flags | Steps grouped into sets of alternatives by way, line and kind of last condition; the first that does something taken where the save cannot decide; `not (1 && GetIsObjectValid(...))` read as false | `steps_at_different_points_of_a_line_are_all_run`: 110, 50, 270 and 610 XP, each worked out by hand from the data |
| R-2 | P2 | Conditions were read against the story values before the run, not as earlier stages change them | Values carried through the run; Skip's `close_quests`, where they already stand at the chapter's end, reads none | the story suites |
| (R-2 follow-up) | P2 | Reading a step's `needs` as well refused Mandalore's recruit: its needs hold `502OND_End_First == 2`, which the run to the ending sets | `needs` not read; only the branch condition | `finishing_escape_merchant_recruits_mandalore` |
| R-3 | P2 | The close warning's count started at 0 again for each newly opened save, so a later save's edits could match an old warning | Keyed on (`session`, `edits`) | code read |
| R-4 | P3 | Records held absolute paths (a moved game hid every backup); a failed or half-made backup was left without a record | Path inside the game folder; half-made backups removed; a record that cannot be written fails the backup | `backups_stay_the_saves_own_after_the_game_folder_moves` |
| R-5 | P3 | Clearing the status on every `refresh_after_edit` also cleared it on a tab switch | Cleared only when the edit count moved | code read |
| R-6 | P3 | A K2 `AddAvailableNPCByObject` without a template for its slot was dropped silently | Noted as left to the game | code read |
| R-7 | P3 | The import rule refused bringing a companion forward within one chapter | Refused only from a save strictly earlier | `a_companion_from_another_save_must_be_the_same_person` |
| R-8 | P3 | Weak tests: the Xbox test passed for another reason; the round trip did not fail on unreadable files; two story tests could pass with nothing checked; the Health test did not show the case was real | Each tightened | the tests themselves |

Checked correct by the review: the order of the checks in `write`, `write_as` and `restore`, that links are never followed while cloud placeholders are not refused, the GFF and archive limits against real files, `condition_holds`, the settings guard, `windows_tool`, `keep_in_range`, and the refusals of Finish Quest.

## G. Portability (for the macOS port; nothing changed)

**Finding the install and the game:**
- `games::is_install` expects a `swkotor*` executable or a `.app`. The Mac bundle layout is unverified, and a case-sensitive disk would miss `CHITIN.KEY`.
- `game_running` uses `pgrep -x` with names unverified on the Mac. If `pgrep` is missing, saving is never allowed.

**Writing while the game runs:** on macOS a file another program holds open can still be renamed. The Windows sharing errors that make a write fail cleanly here do not happen there. The game check carries all of it, and the held-file tests are Windows only.

**Paths and text:**
- Paths are written as text (`display()`), which loses bytes of non-UTF-8 Unix paths.
- `backup_root` compares lower-cased paths, which joins two folders differing only in case on a case-sensitive disk.
- `source_key` lower-cases on Windows only.

**Other:**
- The registry and Program Files lookups are `cfg(windows)`. Local time comes from `date +%z`.
- The folder picker (rfd) must run on the main thread on macOS. It does today.
- The KotOR II Workshop lookup assumes Steam's layout.

## H. Remaining issues

Not fixed. Each was checked against the code, and none is P0 or P1.

| ID | Sev | Game | What | Why left |
| --- | --- | --- | --- | --- |
| S-4 | P2 | both | ~~Stage order is taken as number order in Finish Quest and Skip to stage~~ | **fixed**: `questdb::story_order`, shared with the Quests tab. Only stages set in a later chapter than every stage numbered above them are out of order (K1: Carth's 3, Bastila's 1 and 2; the flag alone also marked all of RebuildHK47, which the tab then showed as passed). `finishing_carths_quest_from_the_leviathan_skips_his_talks`, `only_stages_set_in_a_later_chapter_are_out_of_order` |
| S-6 | P2 | both | Journal and plot XP go to the player and `PT_XP_POOL` only; the game hands them out (`DistributeExperience`) to the party, scaled. Other quests' stages reached also give journal XP | **the party half answered in both games: not a defect.** Each game raises a companion's XP to 80% of `PT_XP_POOL` when the save loads, so what goes to the pool reaches the companions. KotOR: Carth brought from Taris, 69,389 of a pool of 86,735. KotOR II (the maintainer, 2026-11-07): "KMSE S-6 fuel", Game3 with the fuel quest finished (player and pool +1,100, companions left at 28,258 and 28,257 in the file), showed Atton and Kreia at 28,993 of a pool of 36,240 and the player at 34,738. **The other half answered too (2026-11-08): not a defect.** KotOR's AddJournalQuestEntry (0x005483F0, in the maintainer's reconstruction at C:\KoTor RE, matched to the x86 and checked on 250 fuzzed cases) moves a quest's stage only when the new stage is higher (or the script allows a lower one), then pays that stage's journal XP to the party (DistributeExperience). It does so for any quest a script names, so another quest moved by a finish pays its XP in the game too, and only when it moves up: Finish Quest does the same (another quest's stage only when above where the save has it). Playing it was not practical: the only finishes in the saves here that move another quest with XP are Manaan's star map (ends in the Hrakert Rift) and the Star Forge. KotOR II's copy of the function was not read |
| S-7 | P2 | both | ~~"Happens in the game" lists events the game may never run once the journal is past them~~ | **fixed**: the pop-up lists what the editor did ("Also done") apart from the steps "Left to the game", and says the game may skip those |
| S-8 | P2 | both | Go Back takes back the chapters' items up to the number they give, bought ones of the same kind among them; companions' XP is not rewound | the save cannot tell a bought item from a found one; **the pop-up now says so, and offers "Keep my items and credits"** (off by default; on, nothing is taken back and what departing companions wear goes to the party inventory) (it claimed bought items were not undone, and that XP went back for all). The Endar Spire save's leftover AVAILNPCn of a companion not yet met is left: the game keeps such files itself (BOS_SAVE keeps Bastila's while she is not recruited) |
| S-9, S-10, S-11 | P3 | both | ~~Skip/back previews show no notes; a panic in the preview worker leaves "Calculating"; script line clamp 0..255 against globals' -128..127~~ | **fixed** (2026-11-08): the Skip Ahead and Go Back previews list what they cannot do under NOT DONE before Apply (`failed_notes`); a panic while working one out is caught and shown as an error (`prepare_guarded`); a script line's global number is kept to the game's signed byte as every other write; a journal step to a stage the game may lower lowers it (`lowered_stages`; Carth's 3), and the star map quest takes the planet's own stage as the map script sets it (10 Korriban, 20 Tatooine, 30 Kashyyyk, 40 Manaan, then 50 with the fourth); XP above 2^31 shows capped, not negative. `audit_p3.rs` |
| E-3 | P2 | both | ~~Add a Class lets a Jedi player take a non-Jedi newest class, which sets Force to 0~~ | **fixed** in the second pass: only Jedi classes are offered then, in Add a Class and in Change Class of the newest class |
| E-4 | P2 | both | ~~Classes no player can get (Minion; K2 Tech Specialist, Bounty Hunter; prestige classes at any level) are offered~~ | **fixed** by a hand-kept rule, since no game data tells them apart (`classes.2da` `playerclass` is 1 for all of them): no Minion; Tech Specialist for party members only; a prestige class only for a player with one Jedi class at level 15 or above, party members never; the Bounty Hunter row has no name and was already hidden (`questdb::class_offered`, `audit_2026_11_07.rs`, rows checked against both games' tables). What the game does with a class it never gives was not tried |
| E-5..E-15 | P3 | both | ~~`MClassLevUpIn` left after removing a class; party leader flag; story lock on "With you"; equip checks in the backend; feat prerequisites; class change keeps granted feats; three classes from a damaged rewind; companion rename language; portrait changes gender; XP display above 2^31~~ | **fixed** (2026-11-08): E-5 a level-0 class is removed without being raised first, and `MClassLevUpIn` is reset; E-6 a member who stays keeps their own party entry (the leader flag); E-7 a member the story keeps out is refused, and the With you switch is off for them; E-8 Unequip refuses when the inventory is not loaded (the slot rules stay where items are picked, the one caller, which has the game's tables); E-9 a template weapon's base item is read from its template; E-10 adding a feat without its prerequisite feats says which (not refused: a save editor adds what it is asked); E-11 Change Class swaps the classes' own feats (`swap_class_feats`); E-12 a history naming more than two classes is not rewound; E-13 a renamed companion's name is written for the game's own language too (dialog.tlk's); E-14 a portrait that changes the gender says so; E-15 XP below the level's need says so, stack sizes are kept to the item's maximum in the handlers, XP shows capped. `audit_p3.rs` covers E-6, E-7, E-12 |
| W-1 | P2 | both | ~~Stored area copies drop each door's and trigger's own `Tag`~~ | **fixed**: measured in 12 KotOR areas, every one of 107 doors carries its own Tag (2 differ from the template), 8 of 160 triggers (1 differs); now kept |
| W-2 | P2 | both | ~~Advanced, Creatures removes any creature with one click, party copies included~~ | **fixed**: a recruited member's own copy is refused, with the reason; any other creature is removed only after a confirmation naming it (the maintainer chose the confirmation) |
| W-3 | P2 | both | ~~A spawned copy of a party member is edited along with them~~ | **fixed**: `Save::edit` leaves alone the creatures the editor spawned (`Save::spawned`: its own spawns and the ids `spawned.txt` records for the area). Checked on a real save with companions in the area: the member's own copy takes the edit, a spawned copy of them does not (`audit_2026_11_07.rs`) |
| W-4 | P2 | both | ~~A clicked floor spot is not checked against transition triggers or placeables~~ | **fixed**: `Area::clear_spot` keeps out of the ways out (17 of 17 tested landed inside before) and of every placeable's walkmesh (its `.pwk`, turned to its bearing; 2,125 of 2,186 clicks on placeables landed inside one before), `exits.rs` |
| W-5 | P2 | both | ~~Moving to another module leaves the current-planet values as they were~~ | **fixed** for KotOR (G-2), now with every planet: the Ebon Hawk's arrival scripts (`k_ebn12_<planet>`) set Kashyyyk 20, Korriban 30, Leviathan 40 and Star Forge 50 beside the measured values; the Ebon Hawk keeps the planet it left. KotOR II has `K_CURRENT_PLANET` but never sets it (0 in its four saves here, Peragus and Telos): nothing to set |
| W-14 | P2 | both | ~~Two installs of one game: the first found supplies the data~~ | **fixed**: `State::ensure_game_for` reads the install that holds the save; not run here (one install of each game) |
| U-6 | P2 | both | ~~Minimum window height 780 does not fit 150% scaling on 1920x1080~~ | **fixed** (the maintainer: everything smaller, pictures, fonts and tables alike, anchored as they are): the window's scale factor is lowered as far as the layout's 1280 x 780 needs to fit the window's real pixels, so the whole window shrinks evenly; Windows' own factor at the design size or larger, never below half of it. The window's minimum is 640 x 390 |
| U-8..U-11 | P3 | both | ~~Revert asks nothing; a story result applied over edits made after it was asked for; fixed widths at 1280; an old install's item list after changing the game folder at start~~ | **fixed** (2026-11-08): U-8 Revert asks first, keeps the backup mark and the route, and a History revert whose reread fails says so; U-9 a story result is worked out again when the save was edited since it was asked for (`story_save_unchanged`); U-10 message texts scroll inside their pop-ups, Settings scrolls with Done in view, and the Party row's columns are tighter for KotOR II at 1280 (not seen in the program); U-11 the preloaded item list is used only when it is of the install in use |
| W-6..W-12, W-15, W-18..W-21 | P3 | | ~~counters on module moves, `spawned.txt` matching, galaxy bits, module names with `:`, map indexes, work on the window's thread, backups inside the game folder, UNC paths, old Steam library format, save list details~~ | **fixed** (2026-11-08) except as said: W-6 a move to a stored area carries the clock and counters on from the module left; W-7 `spawned.txt` records the creature's template too (old lines still match); W-8 a planet shows on only with both halves of the mask (the halves differ in the saves here) and turning off the selected one clears the selection; W-9 see F-8; W-10 map picks are bounds-checked; W-11 the spawn list's templates are read once per area (the first map open and `arrivals` still read on the window's thread); W-12 another area's map takes the height of the marker nearest the click; W-15 a picked folder the install list would leave out is refused with the reason; W-18 a game folder the editor cannot write in keeps the backups in the editor's own folder (backups are still never removed by the editor, and a Steam uninstall still removes those in the game folder: chosen, to keep them out of Steam Cloud); W-19 a network path is shown and opened as \\server\share (`strip_verbatim`); W-20 the pre-2021 libraryfolders.vdf lines and HKLM's Steam InstallPath are read (the registry read not run here); W-21 a save listed once however its folder is spelled (`folder_key`), and a save of the other game left out of an install's folders (`add_save`); reading added folders on the window's thread stays |
| F-3..F-5, F-7, F-8 | P3 | | ~~game files sized before checking (BIF, RIM), Latin-1 labels written as UTF-8, more than 64 localized substrings cut, a Screen.tga up to about 768 MB, `:` in a module name~~ | **fixed** (2026-11-08): F-3 a table's offset and size are checked against the file before memory is set aside (`read_at`, `Key::bif`); F-4 GFF labels and ERF names are written a byte to a character, as read (`latin1`), and LASTMODULE and the transition's strings in the save's own encoding; F-5 every substring of a localized string is kept, a count its size cannot hold is refused; F-7 a TGA's RLE bound is a byte and a pixel per 128 pixels, and a packet stops at the picture's end; F-8 and W-9 a module name becomes a path only when it is 1 to 16 letters, digits and underscores (`map::is_module_name`). `audit_p3_formats.rs`, 15 tests that failed on the old code (two of them by asking for 4 GB) |
| Clippy | P3 | | Strict lints (unwrap, indexing, arithmetic) give 26,000 warnings, mostly in tests and checked code; the default lints show only style and performance notes | not defects; kept as a list |
| Dependencies | P3 | | unmaintained: bincode, pin-utils, ttf-parser, zbus_names, zvariant (through Slint and the Linux file dialog); yanked: async-executor, clipboard-win | no safe upgrade without upgrading Slint or rfd |

## I. Release recommendation

**READY FOR LIMITED BETA TESTING.**

What supports it:
- No P0 is known.
- Every P1 found is fixed and has a test that fails without the fix (S-1, S-2, S-3, B-1), or is a message path read in the code (U-1).
- Writing, backing up and reverting are tested under injected failures in both games.
- Every save on the PC reads and writes back without loss.
- The full test suite, 126 tests, passes (section J).

What stops a release candidate:
- Few edited saves were loaded in the game: Finish Quest, Skip Ahead, travel and companion saves in KotOR (section E), one Finish Quest save in KotOR II.
- No P2 story defect is left unanswered: S-4, S-7, E-3 and E-4 are fixed, S-6 is not a defect (both games raise companions on load), and S-8 is answered by a switch that keeps the items. Story moves (Skip Ahead, Go Back) are still unplayed in KotOR II.
- The window at 150% scaling on a 1920x1080 laptop (U-6) is fixed by shrinking, but not yet seen on such a screen.

A beta should load edited saves in both games, above all Finish Quest, Skip Ahead, Go Back, companion import and travel, and report back.

## J. Tests run

| Run | Result |
| --- | --- |
| Baseline, `build.ps1` at `cac2d4b` | release build OK; 104 tests passed |
| Final, `build.ps1` with every fix | release build OK; **126 tests passed, 0 failed** (22 new); `dist\KOTOR Modern Gamesave Editor.exe`, 15,215,616 bytes, SHA-256 `2B8A969169FD34C0D2517AF78D2B194B65C2B4254F8EE86086E26858B7E54DBC` |
| The release program | started from `dist`, listed the saves, opened BOS_SAVE (KotOR) and showed it; nothing written |
| Tests shown to fail without their fix | the star map finish ("4 maps found, 0 before"), Kodin's payment (-46,400, then -3,800), Mandalore's recruit, the restore rollback and own-backup check (broken on purpose, caught, restored), Health after lowering Constitution (-218 and -6) |
| `cargo clippy --all-targets` with strict lints | run once; no defect among the default lints (style and performance notes only) |
| `cargo audit` | one advisory, fixed (D-1); unmaintained and yanked crates listed in section H |
| Fuzzing | `parser_fuzz.rs`: 16 files of real saves, 1,500 to 3,000 damaged copies each, read: no panic; two made-up files asking for gigabytes, refused |

The new test files: `roundtrip.rs`, `write_faults.rs`, `parser_fuzz.rs`, `finish_quest_audit.rs`, `audit_2026_11_07.rs`, and three more cases in `restore.rs`. Every test that needs the games or saves passes without checking anything on a PC without them (GitHub's runners); there it checks the made-up files, the quest data and the one-path rule only.
