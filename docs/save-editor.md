# KOTOR Modern Gamesave Editor: what it reads, what it writes, field by field

Reference, 2026-10-22, following the [documentation standard](documentation-standard.md).
This covers the KOTOR Modern Gamesave Editor (KMSE, this repository; [README](../README.md)): a
separate program, not part of the KMRP installer or patch, that opens KotOR 1 and KotOR 2
saves and changes them. Until 2026-10-24 it was the KMRP Save Editor in KMRP's
`save-editor/` folder; the paths below are this repository's since it moved here that day. The save format itself (every file, struct and field, with where
each fact was measured) is in [save-editor-format.md](save-editor-format.md); this document
says which of those fields the editor changes, under which rules, and how that was checked.

Provenance: the format facts come from real saves read on the maintainer's PC (ten KotOR 1
saves, four KotOR 2 saves), the installed games' own tables, and the maintainer's own
(private) reverse engineering of the KotOR 1 executable for what the engine reads and
recomputes when a save loads. Nothing comes from the code of KSE (KotOR Savegame Editor)
or its forks: KSE was used only as a list of what players expect an editor to do and of the
bugs they reported. Untested parts are marked **untested** where they are described.

## The build this describes

| | |
| --- | --- |
| Source | this repository (Rust 2021, Slint 1.18 with its software renderer and the winit backend) |
| Toolchain | rustc 1.99.1 (2026-10-13), cargo 1.99.1 |
| Windows build | `dist\KOTOR Modern Gamesave Editor.exe` from `build.ps1` (size and SHA-256 printed by the script) |
| Linux and macOS builds | `build.sh`; **not built or run yet** (no Linux or Mac machine here) |
| Fonts | the KMRP Original HD fonts (KMRP's [fonts/](https://github.com/RayesDiyab/KMRP-KOTOR-Modern-Restoration-Patch/blob/master/fonts/README.md), SIL OFL 1.1, [licence](../fonts/OFL.txt)): Menu, MenuMedium, MenuSmallB and Descriptions, committed in `fonts/` and embedded at build time ([build.rs](../build.rs)). *Corrected 2026-10-22:* until then they were the traced OTFs from `tools/build_hd_original_otf.py`, made from the game's atlases and never committed |
| KotOR 1 tested | Steam, `swkotor`, with KMRP and K1CP 1.10.0; and a KOTOR Patch Manager test copy of the game in KMRP's build folder |
| KotOR 2 tested | Steam, German release, TSLRCM 1.8.6, saves in `cloudsaves\<account>` |

## 1. What the engine does with a save

The parts that decide what an editor may change (measured; sources in
[save-editor-format.md](save-editor-format.md), sections 3.4 and 8):

| Field | Read when the save loads? | So the editor |
| --- | --- | --- |
| `Str`..`Cha`, skills `Rank`, `FeatList`, `KnownList0`, `Experience`, `GoodEvil`, `HitPoints`, `ForcePoints` | read | changes them |
| `MaxHitPoints`, `MaxForcePoints`, `PregameCurrent`, saving throws, `ArmorClass`, item `Cost` | **not read; recomputed and written back** | computes both maxima by the engine's rules and shows those; after a Health, Force, level or class edit writes the three Health and Force fields as the game would at a save |
| `CurrentHitPoints`, `CurrentForce` | read, the player's **relative to the base** (`stored + max - base`), anyone else's **plus level x modifier** ([format, 3.4](save-editor-format.md#34-hit-points-and-force-points-what-is-stored-what-is-recomputed-refile)) | shows and stores them by the same two rules, so the game loads exactly the value set |
| The player's level | recomputed from `ClassList` and one `LvlStatList` record per level | a level is added only through a class, with its record (section 4) |

The player exists in `Mod_PlayerList` of the last module's `Module.ifo` (both games) and, in
KotOR 2, also as `pc.utc` in `SAVEGAME.sav`. A party member exists as `AVAILNPCn.utc`, and a
second time in the area's creature list when they stand there (the Ebon Hawk in the
GalaxyMap save holds all seven recruited members twice, with identical stats).

## 2. Where saves are found

Nothing is configured. On every start the editor looks in every place below and lists what
is there; Add a Save Folder in Settings adds any other folder (a save, a folder of saves, or a game
folder) and remembers it.

| Where | Path | Tested here? |
| --- | --- | --- |
| Steam libraries | every `path` in `<Steam>/steamapps/libraryfolders.vdf`, for the Steam root in the registry (`HKCU\Software\Valve\Steam\SteamPath`), Program Files, `~/.steam/steam`, `~/.local/share/Steam`, the Flatpak Steam and `~/Library/Application Support/Steam` | yes, Windows: KotOR 1 on C:, KotOR 2 on a second library on D: |
| Game folder saves | `<game>/saves` (any case) | yes, KotOR 1 |
| KotOR 2 Steam Cloud | `<game>/cloudsaves/<account>/`, marked CLOUD | yes |
| KotOR 1 Steam Cloud | Steam syncs `<game>/saves` itself; nothing separate | yes (no separate folder exists) |
| GOG and the discs | registry: GOG ids 1207666283 and 1421404581; `BioWare\SW\KOTOR`, `LucasArts\KotOR2` | **untested** |
| Windows VirtualStore | `%LOCALAPPDATA%\VirtualStore\<path without C:\>\saves` for a game under Program Files | **untested** |
| Aspyr ports | KotOR 1 Mac: `~/Library/Application Support/Knights of the Old Republic/saves` and the App Store container; KotOR 2 Linux: `~/.local/share/aspyr-media/kotor2/saves`; KotOR 2 Mac: two Application Support paths | **untested**; paths from Aspyr's FAQ ([save-editor-format.md](save-editor-format.md), section 6) |
| Phone, Switch, another PC | Add a Save Folder | **untested** (no such saves here) |
| Original Xbox | recognised by `SaveMeta.xbx` or `SaveImage.xbx`; opened to look at, never written (section 6) | **untested** |

Which game a folder holds: its executable or ini (`swkotor2.*` is KotOR 2), else the size of
`chitin.key`; which game a save belongs to: KotOR 2's `savenfo.res` has `PCNAME`,
`STORYHINT0` or `TIMESTAMP`, KotOR 1's never does. When both games are installed the list
opens on the one used last, and one button switches; KotOR 2 then gets its own colours.

The pen beside the preview's name opens a rename field. Enter or its save icon writes
`savenfo.res`'s `SAVEGAMENAME`, through `Save::write` with a backup first. Escape or a
click outside the field and its save icon cancels the draft. Switching games or selecting
another save cancels it too (`ui/saves.slint`, `ui/app.slint`, `src/main.rs`, 2026-10-24).
Other text fields still apply changed text when focus leaves them. Before this correction,
an unchanged rename field stayed open across game switches, and changed text could be
applied when focus left it. The shared `Field` treated focus loss as Enter.

**Long-name correction, 2026-10-29:** the shared `Field` clipped a fixed-width
`TextInput`, so the hidden part of a long name could not be reached visibly. It now
uses a clipped viewport with an input wide enough for the text. Cursor-position
changes adjust the horizontal offset, following keyboard movement and dragged
selections while retaining the field's padding. Rename still applies only through
Enter or the save icon, and outside clicks cancel it.
Headless input checks on 2026-10-29 used a 195-character name: Home and repeated
Right moved between its beginning and end, edits to both ends reached the rename
callback intact, and dragging beyond the right edge selected hidden characters.
Rendered beginning, end and selection states were inspected. The earlier seven
rename interaction checks passed too. These checks used synthetic rows and wrote
no saves; desktop mouse interaction remains for the user to check.

To check: press the pen, type a different name, then click an empty part of the window.
The field must close and the original name remain. Repeat by switching games and by
selecting another save. Clicking inside the field must keep it open; Escape cancels.
Check Enter and the save icon only on a copy of a save. On 2026-10-25, a headless
interaction check of the generated Slint UI passed seven cases: empty-space click,
game switch, click inside the field, Enter, save icon, Escape and save selection. Only
Enter and the icon invoked the rename callback. It used synthetic rows and wrote no saves.
The 33 existing tests passed too, including round trips of 23 saves and edits read back
from copies of all 23. Desktop click checks were blocked by the automation approval
check; in-game checks for this correction are **untested**.

Copy and Save as a New Save make a new folder beside the save, numbered one past the
highest number in the saves folder and named in the game's own pattern, `<number> -
Game<number - 1>` (`save::next_save_folder` in `src/save.rs`). The name the player typed goes
only into `savenfo.res`'s `SAVEGAMENAME`. The game reads a folder's number and accepts any
name after it, but when it overwrites a save it writes to `<number> - Game<number - 1>`.
From 2026-10-24 to 2026-10-29 the folder carried the typed name instead ("000913 - Game26 -
Copy"). Overwriting such a save in the game left it in place and made a second save of the
same name: "000920 - KMRPPazaak" gained a twin "000920 - Game919" (reported 2026-10-29).
Folders made in that time keep their names; renaming one by hand to the pattern, with the
game closed, ends the problem for it. Checked by `a_new_save_folder_is_named_as_the_game_names_it`
in `tests/save_as.rs`; overwriting a new copy in the game is **untested**.

## 3. Every value the editor writes

"Copies" means every copy of the creature the save holds (section 1). All names in lists come
from the installed game (its `dialog.tlk` and tables, with Override, KotOR 2's Override
subfolders and its Steam Workshop folder first), so a German game shows German names and a
modded game shows its mods.

| Screen | Value | File and field | Rule |
| --- | --- | --- | --- |
| Character | Name (player) | `FirstName` and `Mod_FirstName` (the text, StrRef -1); `LastName`, `Mod_LastName` cleared; KotOR 2 also `PT_PCNAME` and savenfo `PCNAME` | Windows-1252; other characters become `?` |
| Character | Experience | `Experience` | at least 0; buttons set the next level's or the highest level's XP from `exptable.2da` |
| Character | Alignment | `GoodEvil` | 0 to 100 |
| Character | Attributes | `Str` `Dex` `Con` `Int` `Wis` `Cha` | 1 to 100 |
| Character | Add a Class | a second `ClassList` entry at level 1, its hit die added to `HitPoints`, its Force die to `ForcePoints`, for the player one more `LvlStatList` record, XP raised to the new level if short; then Health full and Force up by what its maximum rose, as the game's level-up leaves them | only when there is one class; droids get droid classes only |
| Character | Look | `PortraitId`, `Appearance_Type` (the portrait's body in the current build: `appearance_s`, `appearancenumber` or `appearance_l` of `portraits.2da`), `Gender` | portraits with `forpc` 1 |
| Character | Position (player) | `XPosition`, `YPosition`, `ZPosition` | numbers only |
| Character | Heal Everyone Fully | `CurrentHitPoints` and `CurrentForce` such that the game loads both at their maxima, for every creature (the player: equal to `HitPoints` and `ForcePoints`; a party member: above them by the bonuses the load leaves out) | |
| Abilities | Skills | `SkillList[i].Rank` | 0 to 99 |
| Abilities | (how they are shown) | every feat and Force power the game has is listed, each with its icon (feat.2da `icon`, spells.2da `iconresref`), in groups: feats by `toolscategories` (Jedi, Combat, Weapons and armour, Class abilities, Skills and general, Droid and other), powers by `goodevil` (Universal, Light side, Dark side), a chain's links together in order; what the creature has is lit | names, icons and groups come from the game and Override, so modded feats and powers appear with their own icons |
| Abilities | Feats | `FeatList`: add one entry, or remove one and the same feat from every `LvlStatList[].FeatList` | a new entry is a copy of an entry already there |
| Abilities | Force powers | `ClassList[j].KnownList0` of the class that has powers (else the first Force class): add one, or remove one and the same power from every `LvlStatList[].KnownList0` | only a creature with a Force class; powers are `spells.2da` rows with `usertype` 1 |
| Inventory | Credits | `PT_GOLD` | at least 0 |
| Inventory | Components, Chemicals (KotOR 2) | `PT_ITEM_COMPONEN`, `PT_ITEM_CHEMICAL` | |
| Inventory | Stack | `StackSize` | 1 to `baseitems.2da` `stacking` |
| Inventory | Remove | the `ItemList` entry of `INVENTORY` | |
| Inventory | Add an Item | a new `ItemList` entry built from the item's `.uti` (template-only fields dropped, save-only fields shaped like an item already in the save) | every `.uti` in the game and Override |
| Inventory | (icons, kinds, Add Item) | an item's icon is `i` + its base item's `itemclass` + `_` + its `ModelVariation` in three digits (iw_lghtsbr_001), else the base item's `defaulticon`; the inventory and Add an Item have a tab per kind (two rows of tabs when the window is too narrow for one), and Add Item is a full-width bar under the list, chosen by the maintainer from four rendered placements so it never covers a row; (Weapons, Grenades, Armour, Gear, Usable, Upgrades, Other) from baseitems.2da `equipableslots`, `itemclass` and `defaulticon`; creature-only items are left out | the rule matched all 40 items checked in the GalaxyMap save; Override textures (TPC or TGA) come first, so modded icons show |
| Party | Recruited | `PT_AVAIL_NPCS[n].PT_NPC_AVAIL` | only when the save holds `AVAILNPCn` |
| Party | Can be picked | `PT_AVAIL_NPCS[n].PT_NPC_SELECT` | |
| Party | Influence (KotOR 2) | `PT_INFLUENCE[n].PT_NPC_INFLUENCE` | 0 to 100; hidden for someone not met (-1) |
| Party | Bring or Replace from Another Save | `AVAILNPCn.utc` copied from another save of the same game, recruited and selectable | the other save must hold that member |
| Party | Party experience | `PT_XP_POOL` | members who join later catch up to their share of it |
| Advanced | Global variables | `ValBoolean` bit `7 - i % 8` of byte `i / 8`; `ValNumber` byte `i` (shown -128 to 127); `ValString[i]` | the catalogue lists are never reordered |
| Advanced | Compare with Another Save | read only; "Take" copies the other save's value of one variable, "Take All Theirs" every differing one | matched by name and kind |
| Advanced | Doors | `Locked` (unlocking also clears `KeyRequired`), `OpenState` (opening also unlocks) in the area's `Door List` | |
| Advanced | Containers and stores | `ItemList` of a placeable (new items get an `ObjectId` above every id in the area) or a store (items numbered by position, `Infinite` kept when the store uses it) | |
| Story | Journal | removes an entry of `JNL_Entries` | the story switches stay as they are |
| Story | Galaxy map | `GlxyMap.GlxyMapPlntMsk`: bit *i* and bit *i*+16 together | rows of `planetary.2da` |
| Story | Pazaak side cards | `PT_PAZAAKCARDS[i].PT_PAZAAKCOUNT` | 0 to 9 |
| Story | Marked as cheated | `PT_CHEAT_USED` and savenfo `CHEATUSED` | |
| Story | Save name | savenfo `SAVEGAMENAME` | |

When `PARTYTABLE.res` changes, savenfo's `TIMEPLAYED`, `CHEATUSED` and (KotOR 2) `PCNAME`
are written from it again, so the Load screen agrees with the save.

## 3a. What KSE edits, and where it is here

Taken from KSE 3.3.7's readme (its list of editable fields), not from its code.

| KSE | Here | Rule |
| --- | --- | --- |
| Equipment (player and party) | Inventory, Equipped by | an item taken off goes to the party inventory and the effects whose `CreatorId` is its `ObjectId` go with it; an item put on gets `Mod_NextObjId0` of the module's `Module.ifo` as its object id, which then moves on; only items whose `equipableslots` has the slot and whose `droidorhuman` (1 people, 2 droids) fits |
| Current Party | Party, With you | `PT_MEMBERS`, `PT_NUM_MEMBERS`, savenfo `PORTRAIT1`/`PORTRAIT2`; the area is left alone |
| Levels, Class add/remove/change | Character, Classes, Edit | `set_class_level`, `change_class`, `remove_class` in `src/creature.rs`; one level record per level for the player |
| Hit Points, Force Points (current and max) | Character | shown and stored by the engine's own rules; the maximum moves what the engine computes it from (the player's level-record dice, newest first; anyone else's base); what was missing stays missing ([format, 3.4](save-editor-format.md#34-hit-points-and-force-points-what-is-stored-what-is-recomputed-refile)) |
| Min1HP | Character, Cannot die | `Min1HP` |
| Appearance, Gender, Soundset, Portrait, NPC name | Character, Look and voice | `Appearance_Type`, `Gender`, `SoundSetFile`, `PortraitId`, `FirstName` |
| Time played | Story, More | `PT_PLAYEDSECONDS` (savenfo follows) |
| Quests | Story, Quests | `JNL_Entries`: a quest at a stage of global.jrl |
| Copy inventory between saves | Inventory, Copy from Another Save | the other save's `INVENTORY` item list |
| Xbox .sig files | not done | see section 6 |

Area creatures (in the fork the maintainer showed) are listed in Story, Creatures, with a
remove cross.

## 3b. The map, moving the player, Finish Quest

| What | How |
| --- | --- |
| The picture | `lbl_map<area>` from the GUI texture pack or Override (KotOR 1 also without the planet prefix), its left 440 of 512 pixels |
| World to map | the .are `Map` struct: scale per axis from `WorldPt1/2` and `MapPt1/2`, axes swapped for `NorthAxis` 2 and 3 (`Calibration` in `src/map.rs`); checked against the game: markers in their rooms on Tatooine, the Ebon Hawk, Citadel Station (KotOR 2) and the Jedi Enclave (`NorthAxis` 3) |
| The arrow | `mm_barrow`, turned by the angle the player's `XOrientation`/`YOrientation` makes on the map; matched the game's arrow at 90 and 180 degrees |
| Markers | round badges drawn in the editor (so they stay round whatever the renderer clips): a person's portrait (portraits.2da `baseresref`), else the game's icon: `i_opendoor` door, `ig_areatrans` way out (KotOR 2 only; KotOR 1 shipped no walking figure, so `gui_mp_doorU`), `i_openplace` container, `i_useplace` thing to use, `i_dialog` person, `whitetarget` map note; the ring's colour says the kind |
| Zoom | 1x to 6x with + and - beside the map or the wheel (about the pointer); zoomed in, a drag moves the view; markers keep their size |
| Floor | every room of the area's .lyt, its .wok faces whose material has `walk` 1 in surfacemat.2da; a spot must have floor under it and under eight points 0.45 m around it, within 0.6 m of its height |
| Spots | things to use and containers: in front (their `Bearing`), then round the sides, 1.1 to 2.4 m out; people who talk: 1.3 m out; doors: 1.2 to 2.6 m out on each side; ways out without a door: the trigger's middle; a clicked point: the nearest spot within 1.5 m |
| Moving within the area | `XPosition`/`YPosition`/`ZPosition` of every player copy, facing the object |
| Moving to another area | only areas in SAVEGAME.sav; the player goes into that module's `Mod_PlayerList` with that module's player `ObjectId` and `AreaId`, its items new ids from `Mod_NextObjId0`, effects re-pointed; savenfo `LASTMODULE` and `AREANAME` |
| Spawn a Creature Here | on a picked floor spot in the area the save resumes in: a copy of a creature standing there, or any creature template the game has (its templates and Override, and the area's own module templates), in tabs In This Area, Hostile, Friendly, Other (by `FactionID` in repute.2da); it faces the player; a new one runs its own spawn script when the save loads ([save-editor-format.md](save-editor-format.md), section 5.2) |
| Finish Quest | the quest's stages with `End` 1 in global.jrl; XP = plot.2da `xp` x the stage's `XP_Percentage`, added to `Experience` and `PT_XP_POOL` when the stage is new |

**Travel and Skip Ahead** (2026-10-23; played by the maintainer in KotOR 1):

| What | How |
| --- | --- |
| Any area | The Map tab's Areas list holds every area of the game, by planet (cutscene stages, `stunt_` modules, and nameless modules left out; of two areas with one name the visited one, else the one a door leads into). An area not visited shows the game's own .git with each object's template fields filled in (names, portraits) |
| Travel Here | The ways the game's doors and transitions lead into the area (each module's shipped .git, `LinkedToModule` and `LinkedTo`). Picking one queues an area-transition event (`EVENT_AREA_TRANSITION`, 26) first in the current module's `EventQueue`, due at the saved moment: the game makes the trip right after the save loads, loads an unvisited area fresh from its files with its arrival scripts, and brings the party (`Save::queue_transition`; the event's layout from the maintainer's own reverse engineering, handler 0x0050D6C0). *Corrected 2026-10-23:* queued last and a second later, it waited behind the save's own delayed events until the player did something |
| Skip Ahead | Story, Timeline (Skip to Here in a chapter's pop-up): a chapter as if played (`questdb::skip_to`). The chapter's milestone (`data/quests/*-milestone.json`): journal, the story values the rest of the game reads, items; its rewards (`*-rewards.json`, `tools/questdb/rewards.py`): the XP of a full run (kills by the engine's rule, plot XP, script XP, SetXP floors, `tools/xp/calc.py`), the enemies' Dropable items and the containers' loot (class gear by the game's own tests); then the trip by the game's own first hop with its movies, so the story plays on (from the Endar Spire: escape pod movie 01c, the vision, then the Taris apartment) |
| Kill XP | The engine on every death (`ApplyDeathExperience` 0x004FB1E0): xptable.2da row (player level from XP) column trunc(CR), for a creature whose faction's reputation toward the player is 10 or below; the player all of it, party members 80%, the pool all of it. Checked against two Endar Spire saves (150 and 450 XP at the same points) |

**The whole of KotOR 1** (2026-10-23, mapped by `tools/questdb`, `checked: false`): 12
chapters (Endar Spire, Taris, Dantooine, Tatooine, Kashyyyk, Manaan, Korriban, Leviathan,
Unknown World, Star Forge, and the Ebon Hawk and Yavin Station as side chapters), each with
its quest file, milestone, rewards and XP scenario, in `data/quests` and `data/xp`, IDs only
(no game text: names and journal text are looked up from the player's own game; the readable
review pages go to the ignored `data/quests/local`). Skip to Here applies every main chapter
before the picked one on the save's route (the galaxy map's order, below) that the save has not
played, in that order, then the picked one:

| Applied per chapter | How |
| --- | --- |
| Journal | the chapter's stages, then the quests the game itself removes (`journal_remove`) |
| Story values | the milestone's globals |
| Star maps | `K_STAR_MAP` counted the game's way (10 and 10 a planet whose map is found) and what reaching a count sets (the Leviathan capture after the third, the Unknown World after the fourth). Only the chapter's own `K_STAR_MAP_*` flag is set. Until 2026-11-02 closing a chapter's quests replayed every branch of the game's star map script, so a skip that included Tatooine or Korriban found all four maps, opened the Unknown World and left `K_STAR_MAP` at 10, and the capture never played. Checked by `tests/star_maps.rs`; in the game, **untested** |
| Which planets | the order on the galaxy map's route (`questdb::story_route`): the free planets the save has done, in the order it went to them, then those it has begun, then the rest in the order the player picks with the arrows under the map (the timeline's order, Tatooine, Kashyyyk, Manaan, Korriban, until the player picks another). A skip to a free planet plays the planets before it on the route that the save has not played; a planet picked first is played alone. The count follows the planets in any order. The Leviathan goes in after the third free planet, as in the game (`story_route`; `leviathan_in_place` in `src/questdb.rs` keeps it there when a skip runs), and a skip to the Leviathan leaves the fourth planet open. Until 2026-11-02 the Leviathan came after all four; until 2026-11-06 the order was always the timeline's, with no choice. Checked by `tests/galaxy_route.rs` |
| Items, loot | held items, the enemies' Dropable items, the containers' loot (class gear by the game's tests); credit chips (base item 57) add their stack size to the party's credits |
| Credits, alignment | what the chapter's conversations and scripts pay and take, for a light-side and for a dark-side run (`tools/questdb/credits.py`, in the rewards file's `credits`), plus the payments the main path cannot avoid (`credits_fixed`: the droid on Taris, the assassin droid on Tatooine); the alignment moves by the side's points, never toward the other side |
| Light or dark | asked in the confirmation when it changes anything (`side_matters`), the player's current leaning first; the dark side applies the chapter's `dark` block where the story branches: on Dantooine slot 5 is killed, not recruited; at the temple summit slot 0 rejoins and slots 4 and 5 are lost, then slots 2 and 6 leave the ship; the Star Forge plays the dark-side ending |
| Companions | recruited from their templates into their slots (AddAvailableNPCByTemplate), removed where the story removes them |
| Jedi class | the player's pick in the confirmation: `DAN_PATH_STATE`, a level of the class, its lightsaber |
| XP | the cumulative full-run total at the end of the picked chapter, never lowered |
| Areas | every area of every chapter the player can return to is stored as played through (`src/stored.rs`): each object a full struct as the game stores a left area, one-time enemies omitted, doors on the path open and unlocked, looted containers empty, the area's, module's and objects' local flags as a run leaves them; never over an area the save has been to; only for save groups the trip on keeps (modulesave.2da) |
| The trip | the last chapter's own route on, with its movies |

Checked without the game (2026-10-23; the audit of 2026-11-02 found the test needs only 50
modules to pass and no script here repeats the `danm14ad` comparison, so read this as a record
of that day): a played-through copy of every one of the 117 modules
builds and reads back (`played_through_copies_of_every_module`; since 2026-11-02 the door and
placeable counts leave out objects with no template anywhere, which a mod can ship and the
editor cannot build), and a synthesized `danm14ad`
matches the copy the game itself stored in a real save list for list and field for field (but
for fields the game recomputes on load); companions from templates write and read back
(`recruit_from_template_survives_a_write`); a new save skipped to the end of Dantooine, of
Korriban and of the Star Forge reads back with the XP (53,690; 156,320; 207,440), the
companions, the class and lightsaber, the stored areas and the trip; a save on the Endar Spire
skipped through the whole game on each side reads back with that side's ending, party,
alignment and more credits (`skip_whole_game_on_each_side`). **Not yet seen in the game:** any
chapter after the Endar Spire; a return to a stored area; companions from templates; the
credits and the dark side.

How credits.py models a run, per conversation: a node's own effect is what its scripts pay,
take and shift; where the player picks a reply, one with a condition (a bounty handed in) is a
situation the run meets and counts, unless it costs credits or leans to the other side, and of
the free replies the run takes one that costs nothing if it can, then the side's way, then the
most credits; where the game picks a line, the best one (a persuade check passed); every
starting line counts (later visits), each node once. Gambling is left out. Per KotOR 1 chapter
(light / dark): Taris 2,980 / 4,005; Dantooine 2,900 / 3,000; Tatooine 2,470 / 2,590; Kashyyyk
1,900 / 2,150; Manaan 7,600 / 9,300; Korriban 1,500 / 1,500.

**KotOR 2** (2026-10-23) is mapped the same way, from the game's own files (Steam 1.0.2), in
eleven chapters: Peragus, Telos, Nar Shaddaa, Dxun, Onderon, Dantooine, Korriban, The Jedi
Masters (the enclave meeting and the return to Telos), Ravager, Malachor V, and the Ebon Hawk as
a side chapter (`tools/questdb/chapters_k2/`). What differs from KotOR 1:

| | How |
| --- | --- |
| Conversation scripts | KotOR 2 nodes run two scripts with five numbers and a string each (`a_global_set`, `a_givecredits`); the tools run them with those values bound (`tools/questdb/nodescript.py`, `ncsvm` params), so the values become constants |
| The masters found | `000_Jedi_Found`, a number each free planet raises by one (`increments`); at 4 the game itself calls the party to the enclave |
| Companions | by the player's gender (slot 4 for a male player, slot 11 for a female one) and side (slot 7 light, slot 10 dark); a template only in an area's own files is read from that area |
| Items | many loot and quest items exist only in an area's own files; Skip Ahead finds them in the chapter's areas |
| Kill XP | read from swkotor2.exe 1.0.2 (`CalculateDeathExperience` at 0x00575B20): KotOR 2's tables, the row and column capped at 31, and an autobalanced creature (`MultiplierSet`, 316 of the 333 hostile kinds) counted at the player's level when it spawned plus its `autobalance.2da` crmod (`tools/xp/calc.py`) |
| The trip | the save's event queue loads event 26 with the same layout as KotOR 1 (swkotor2.exe 0x005205F0) |

A full KotOR 2 run reaches 240,360 XP (this page said 249,610 until 2026-11-02; the data has never held that); light / dark credits by chapter: Telos 8,545 / 9,545,
Nar Shaddaa 4,550 / 8,953, Onderon 37,475 / 42,475 (each side's ruler pays the war's reward,
`credits_side_only`), Dantooine 12,000 / 13,575. Checked without the game: a save on Peragus
skipped through the whole game on each side reads back with the masters found, the XP, the
companions by gender and side, and more credits (`skip_whole_k2_game_on_each_side`). **Before
it ships:** nothing of KotOR 2 has been seen in the game. Quest XP takes one route per quest
(of the stages in the same group of ten, the best paying), and autobalanced enemies are counted
at the player's level on entering their area.

**As a real run leaves it, nothing twice** (2026-10-23, both games). Skip Ahead closes each
skipped chapter's own quests at the ending a run reaches (journal, story values, the NPCs' own
flags, doors, people who leave; `close_quests`), so their people do not offer them again; it
brings the areas the save has already been to to the played state too (one-time enemies gone,
doors open, looted containers emptied, flags set; `reconcile_visited`); and it leaves out what
the save already had: containers it emptied, drops of enemies it killed, conversations whose
quests it finished (their credits, alignment and influence), purchases whose companion it has
(T3-M4, HK-47, KotOR 2's HK chassis), story items and the Jedi lightsaber it holds, a KotOR 2
master who already taught (`master_met`). Checked: a chapter applied twice pays little the
second time and gives no item twice (`a_chapter_does_not_pay_twice`). KotOR 2's per-object
flag table holds 160 flags and 32 numbers (KotOR 1: 96 and 8); played-through areas use the
game's own size.

**KotOR 2 party and powers** (2026-10-23): influence per companion from the conversations of
each chapter (`ModifyInfluence`, the run's replies preferring what a companion likes), added to
`PT_INFLUENCE`; the masters' teachings by the game's own rule (`a_give_form`: class, prestige
class, `G_PC_FORM`, side, forms known; spells.2da 261 to 268); Force Chain on Peragus, Force
Enlightenment or Force Crush at the enclave; the prestige class Kreia offers from level 15,
picked in the confirmation (`a_pc_jedi`: the class added, `000_Prestige` 2); and, if the
player picks it, the companions trained as Jedi without their conversations (the class
`a_makejedi` gives and the values their conversation chains end with).

**The galaxy map** (the Timeline tab; 2026-11-06, asked for by the maintainer: "the timeline
needs to be changed into a galaxymap with blue bands of light connecting the planets visited in
order a player visited them ... aesthetically pleasing in the style of kotor but also
intuitive"). It replaced the timeline (below). It was first a section of Story, beside Quests
and Planets, Pazaak; the same day the maintainer asked for it as a tab of its own beside Story
("Make Timeline a seperate tab next to story and make the actual galaxy map bigger"). With no
section tabs above it the map is 453 px tall at 1600 by 900, where it was 380 px. Story keeps Quests and Planets, Pazaak and opens on
Quests. The map is a dark
display in light mode too, on the generated starfield (`ui/editor.slint`, `GalaxySection`;
`fill_skip` in `src/main.rs`):

| What | How |
| --- | --- |
| Stops | each main chapter where the game's own galaxy map puts its planet (`MAP_STOPS` in `src/main.rs`; the planets as the editor's Planets page spreads them), drawn as the game's own planet texture wrapped on a globe (`globe_pixels`, read from the player's install); the ships (Endar Spire, Leviathan, Ravager) and stations (Star Forge, the masters' meeting) drawn by the editor, near the planet the story meets them at. Each carries its number on the route |
| The route | the main chapters in the order the save plays them (`questdb::story_route`): outside the free planets the story is one line; the free planets (KotOR: Tatooine, Kashyyyk, Manaan, Korriban; KotOR II: Nar Shaddaa, Dxun with Onderon, Dantooine, Korriban) come done first, then begun, then not visited; KotOR's Leviathan after the third. Done: the chapter played (`chapters_played_in`, where KotOR's `K_STAR_MAP_*` TRUE counts as played). Begun: the save has been to one of the chapter's areas (`questdb::been_to`: the areas the save keeps, and the one it is in), or KotOR II's master marker for it is above 0 (`351NAR_Ebon_Released`, `506OND_End`, `602DAN_End`, `702KOR_Vash`; it can be set before the planet's end). **Corrected 2026-11-06:** the first version took a quest of the planet's in the journal for a visit, and KotOR II hands out a Nar Shaddaa quest on Telos, so a save on Telos showed Nar Shaddaa begun; and it took KotOR II's markers for done, which Skip Ahead did not, so a planet could read done on the map and be played again by a skip |
| The order of what is done | the save does not record the order planets were played in. The route takes the order in which each planet's own quests first appear in the journal (`JNL_Entries` keeps new entries at its end). Read from four of the maintainer's KotOR saves on 2026-11-06 (`examples/journal_order.rs`): the entries mostly follow the order of play, but not strictly (one KotOR save lists an Ebon Hawk quest before Taris's `tar_escape`), and `JNL_Date` is a day count of 0 or 1 with `JNL_Time` inside a day, so neither orders the planets. Where the journal guessed wrong, the arrows reorder what is done among itself. The begun and the not visited move among themselves too, so a planet just landed on can wait while another is skipped to |
| Bands of light | lit blue lanes join the stops the save has played or begun, in route order; faint lanes run on along the route ahead. Each lane is a quadratic curve bending a little to one side (`lane` in `src/main.rs`) |
| You are here | the stop whose areas hold the area the save will be in (`Save::place`: a trip waiting in the queue, which Skip Ahead, Go Back and Travel Here set and the game makes as the save loads, else the area it is in; `questdb::chapter_of_module`, by each chapter's `cleared` areas): a gold ring and gold name, and a gold "You are here" pill above the map naming it. At Yavin Station, "At Yavin Station". On the Ebon Hawk (or an area no chapter lists) it is the last stop played, "Aboard the Ebon Hawk, last at ...", and the Ebon Hawk is drawn beside it. While no trip waits, the chapters before the stop the save stands in, on the story's line, read played (`chapters_played_in`); with a trip waiting, the journal alone decides. **Corrected twice on 2026-11-06:** the first version used the area the save stood in even with a trip waiting, which after Go Back is still the area gone back from: a return from the Unknown World to Kashyyyk read every planet played again, and a later Skip Ahead would not have played them. The second used the earlier of that area and the trip's, but Go Back's trip leads to the Ebon Hawk or a cutscene area, not to the chapter gone back to, so the same happened (`tests/audit_round2.rs`) |
| States | played (blue number, blue ring), the one the save is in (gold), begun and not finished (blue outlined number), the next stop (brighter), ahead (dim). Pointing at a stop shows a card with its number, state and chapter title |
| Planet order | under the map, the free planets in route order, each with arrows to move it one place earlier or later among those of its kind (done, begun, not visited); Undo order changes goes back to the save's own order; its room under the map is kept while it is hidden, so the map keeps its size when a planet is moved (until 2026-11-06 the line grew when the button came and the map shrank by the difference; measured since by the map's corners, at the same rows before and after a move at 1600 by 900 and 1280 by 780). The order changes nothing in the save; Skip Ahead and Go Back follow it. It is kept until another save opens |
| Played | a chapter's journal entries reached, or its journal quests taken out of the journal by a chapter the save has played (`chapters_played`); since 2026-11-06 also every chapter before a played one on the story's line, and all the free planets before a chapter after them (KotOR's Leviathan excepted, which follows the third). A KotOR save on the Ebon Hawk after the Leviathan had no Dantooine entry left and read Dantooine as never played, so the map put the save in Dantooine |
| Other saves | every other save of the game, any character's, as a tag with their number high on the right of the furthest stop each has played on its own route; green (blue in KotOR II); pointing at a tag of several opens a ring of them (kept inside the map and never taller than it: sixteen saves at KotOR's Endar Spire ran off the map's top until 2026-11-06), pointing at one shows its picture, name, character, level and XP, how far its story is and where it loads. A click asks first (since 2026-11-06, the maintainer: "there needs to be a warning where you can cancel"): a warning names the save, its character, level and story, says the save open now closes and that nothing is written, and offers Cancel (or Escape) and Open This Save. Changes not saved still stop it, as before |
| Zoom and move | the wheel zooms about the pointer, from all of the map (1) to four times (`map.zoom`; each step 1.25); the planets' distances grow with it and the globes by its square root, so they spread apart. Dragging a part of the map with nothing on it moves it, never past the map's own edges at that zoom; a press on a planet still opens it. A double-click, or Fit, shows all of the map again, and so does opening another save. Out, in (1.5 a press) and Fit sit in the bottom right corner, with "Scroll to zoom, drag to move" beside them where the map is at least 1300 px wide (at 1280 by 780 the words covered Tatooine's name). Since 2026-11-06, the maintainer: "make the timeline zoomable and movable with mouse"; checked in the program in both games at 1600 by 900 and 1280 by 780 |
| Loading | filled when a save opens (`open_selected` calls `fill_skip`), and the other saves are read for it then, away from the window, so the tab opens without a wait. The page is made once with the editor and hidden on the other tabs. Measured on 2026-11-06 on a KotOR save: the tab's fill took about 600 ms, nearly all in `questdb::story_route` (145 ms a call, four calls), because `chapter_quests` copied every chapter's milestone file on each call; borrowed instead, the route takes 0.08 ms and the fill 26 ms |
| The pop-up | a click on a stop opens it, as the timeline's did: the chapter's beginning, expandable quests and stages, and its end on the left; the selected point's description, choices and measured changes on the right. The beginning of a stop ahead is skipped to by playing the stop before it on the route, and is not offered when that stop is played (the save is at its beginning already). A begun stop is treated as the one the save is in: its beginning and the stages reached are gone back to |
| Side chapters | the Ebon Hawk's own quests (the ship beside the save's stop) and Yavin Station (KotOR, at Yavin, the station from the game's movie where the build made it); off the route, a click opens their pop-ups |

Measured on 2026-11-06: `tests/galaxy_route.rs` (the default route, a picked order in both
games, done before begun before not visited, played chapters in journal order, the line
before a played chapter, Skip Ahead's path following the route, and a whole route with the
Leviathan after the third planet for each of the 32 saves of both games on the PC). The map
was looked at in the program at 1600x900 with Medium text on a KotOR save aboard the Ebon
Hawk after the Leviathan: the lanes, the hover card, the order arrows and the pop-up. Slint's
software renderer draws a radial gradient as a square and fits a path's view into its element
keeping the view's shape, so the glows are a picture (`glow_image`) and each lane is drawn in
the box of its curve. In the game, nothing here changes what is written: Skip Ahead and Go Back
write as before, in the route's order.

**The timeline, replaced 2026-11-06** (Story, Timeline; 2026-10-23, designed by the maintainer
from mockups in both games' colours). Kept as the record of what the galaxy map replaced. On a
starfield (dark mode; light mode kept the plain panel, whose text is dark):

| What | How |
| --- | --- |
| The line | START to END; each main chapter a point at even steps, the point being the chapter's end, numbered I to X; played chapters filled, the rest hollow; the stretch behind the open save bright |
| You are here | a gold point just past the furthest main chapter the open save has played; the planets that can be played in any order (KotOR's four star maps, KotOR II's four free planets) may still be open behind it, and show hollow |
| Other saves | every other save of the game, any character's, just past the furthest chapter each has played |
| Chapters | pointing at one shows it above the line; a click opens its pop-up |
| Side chapters | their ships at the bottom middle open their pop-ups |

The timeline had no order for the free planets: it always drew and skipped them in the
timeline's order, and put the Leviathan after all four, though the game plays it after the
third.

**The selected point's preview** (2026-10-25, `src/story_preview.rs`,
`src/main.rs`, `ui/app.slint`). The editor clones the current in-memory save, including
pending edits, and runs the same story operation on that independent copy. Preparation
runs away from the window. Apply stays disabled until it succeeds. Changing a story
choice prepares a new result; an older selection's result cannot replace it. Apply uses
that prepared save in memory. Cancel discards it. Neither action writes files; Save
Changes still goes through the existing game-running check, Xbox refusal and backup.
After applying Skip Ahead or Go Back, the popup closes directly to the editor with its
pending change count. Map travel also stays in the editor. The extra "Gone back",
"Skipped to" and travel success dialogs were removed on 2026-10-25; changes still wait
for Save Changes. This covers whole chapters and individual quest stages through their
shared Apply callbacks. Error messages and reports of quest steps left unapplied remain:
the working tree dropped them for a while (audit finding 27), and since 2026-11-02 anything
that could not be applied is shown after Apply (`story_problems`), success reports not.
The earlier removal covered only Go Back; the other success paths are now covered too.

Experience, credits, level and recruited companions always appear. A total that does not
change is shown once, with no difference label. Other changes appear when present:
alignment, party XP, components, chemicals, classes, Health and Force, attributes, skills,
influence, feats, Force powers and companions recruited or lost. The preview reports what
is stored after the operation; XP that has not been spent on levels still lets the game
offer its normal level-up. There is no quest-journal row in the totals table. The selected
stage's full journal text remains in its description.

Inventory changes have their own scroller with two columns, real icons read from the
installed game, and small signed quantities: green for added items, red for removed
items. The count includes shared inventory and equipment held by the player and recruited
companions. Stack splits, row order, object IDs and inventory positions do not count as
changes. Different upgrades and charges remain distinct. If the game supplies no icon,
the item's name and quantity remain visible.

Going back always uses the story's items and credits (`keep_items = false` in the existing
story engine). It removes mapped rewards still held and restores what the story operation
restores. This is not an earlier snapshot of everything the player bought, sold or used;
that limit still applies. **Correction:** the earlier interface offered a keep-items
choice, and this reference described a separate confirmation with a button on each row.
The choice has been removed; the selected row's prepared result is reviewed and applied
in this one popup.

Go Back restores classes from the chronological `LvlStatList` records that remain at
the target level. It removes the recorded gains from later levels, restores XP and party
XP, and updates Health and Force maxima. It caps retained levels before a class granted
by an undone chapter. Later edits to `ClassList` do not prevent restoring usable earlier
records. Missing or unusable retained records stop the operation before story changes.
This restores recorded level gains; unrelated manual stat edits are not an earlier
snapshot. A removed companion is also removed from `PT_MEMBERS`; `PT_NUM_MEMBERS` and
`PT_CONTROLLED_NP` are updated. A side-quest return includes a companion whose recruitment
module is an earlier chapter's recorded arrival module.

Go Back defaults to alignment 50, Neutral. Its preview offers a red-to-cyan slider, a
numeric field from 0 to 100 and Reset to Neutral. Opening a chapter starts a new choice
at 50; changing stages in the same popup keeps the chosen value. The slider supports
dragging, arrow keys and Home/End. Releasing the slider prepares the new preview. The
chosen alignment is part of the prepared result and is applied after any quest-stage
replay, so that replay cannot change it. The stored field is the player's `GoodEvil`
value, edited in the same player copies as other character changes. Cancel discards the
prepared result; writing still requires Save Changes and its backup.

**Alignment correction, 2026-10-25:** Go Back previously subtracted estimated chapter
light-side or dark-side gains. In the reported KotOR 1 save this subtracted 69 from 79
and wrote 10. Chapter totals do not identify the player's actual choices. That
subtraction has been removed. The neutral default and explicit choice replace it;
they do not claim to reconstruct historical alignment. Forward skips retain their
existing alignment behavior. `tests/story_safety.rs` checks neutral defaults, chosen
values including both endpoints, quest-stage returns, invalid inputs without mutation,
and neutral alignment after writing and reopening a temporary copy.
Automated checks on 2026-10-25 exercised those cases with copies from both installed
games. Headless window input checks also covered hover without edits, dragging to both
endpoints, preview refresh on release, arrow keys, Home/End and Reset to Neutral. Renders
were inspected at 1600x900 Medium and 1280x780 Large, plus KotOR II light mode. These are
software-rendered checks; desktop interaction and loading this alignment change in the
game remain **untested**.

**Correction, 2026-10-25:** the former level-removal loop could stop when the current
class list disagreed with the recorded classes, then still reduce XP. In the reported
KotOR 1 Steam Taris failure, a copied written save had level 20 with 3085 XP and active
party members who were no longer recruited. Replaying the selected Taris stage
(`tar_selvenbounty`, 1) on a separate copy of its backup now writes and reads back Scout
level 3, 3085 XP, three matching level records, an empty active party and Carth recruited.
`tests/story_safety.rs` checks active-party removal, refusal before mutation for an
unusable retained record, and reconstruction despite a changed current class list, on
copies from both installed games. Two creature unit tests cover later synthetic levels
and the retained-record check. The original crash's native cause has not been established;
loading the corrected copy in the game remains **untested**.

To check by hand, use a copy of a save. Open a stop on the galaxy map, expand a quest,
and select a stage. Check the totals and both columns of item changes. Try a different
story choice and wait for its new preview. Cancel, then verify that the pending edit
count and values have not changed. Apply a prepared point, check the values in the editor,
then Save Changes and inspect the backup. Load that copy in the game to check its arrival
and normal level-up. Desktop and in-game checks of this popup are **untested** here.

Automated checks on 2026-10-25: `tests/story_preview.rs` prepared forward and backward
changes on copies of saves from both installed games, including pending edits. It compared
the result with the story engine and checked that the source in-memory save and every
file of the test copy remained unchanged. KotOR 1 quest-stage skips and returns were also
compared. The three unit tests in `src/story_preview.rs` check summed stacks, runtime IDs,
inventory positions, field order and distinct upgrades. Headless renders of the generated
Slint window were inspected at 1600x900 with Medium text, 1280x780 with Large text, and
1600x900 in KotOR II light mode. These used sample rows and item icons from the installed
game; they are layout checks, not desktop interaction or in-game checks.
The selected row's divider previously painted over the bottom of its yellow border.
It is hidden while the row is selected or hovered. In the 1600x900 headless sample, the
top and bottom border pixels both read `#f4d03f` after the correction.
The final Windows `build.ps1` run passed all 37 tests on 2026-10-25 and copied the
release executable to `dist`.

The Ebon Hawk is a free-to-use render from Deadly Stream (`assets/timeline/ebon-hawk.png`,
its background removed with rembg's BiRefNet model). Yavin Station is a frame of KotOR's own
movie, so it is not in the repository: `build.rs` makes it from the player's game with ffmpeg
(Live_1c.bik, frame 484, cut out by the committed outline `yavin-station-mask.png`); without
the game or ffmpeg the button shows its name alone. The starfield (`stars.png`) is generated. Sources in
[assets/timeline/README.md](../assets/timeline/README.md).

**A maximum out of reach** (Character, Health and Force; 2026-10-23, the maintainer: "there
should be a notification saying this will lower the level or this will raise the level"). The
game computes both maxima from the level (section 3.4 of the format notes), so at a given level
each has a lowest and a highest value: the player's level dice are 0 to 255 each, and anyone's
Health gains at least 1 a level. The editor finds both ends by setting them on a copy and
reading back what the engine's rules give (`pool_reach`). A typed maximum past either end
opens a pop-up instead of stopping short: "Max Force 500 needs a higher level. At level 2, Max
Force is 256 at most. At level 4 it fits.", a chart with a bar for what the level allows now,
one for the level that reaches the value and a gold mark at the value, and Cancel, Set to 256
and Raise to Level 4. Raising adds levels to the newest class; lowering takes them from the
newest class first, then the one before, as the class level control does. A creature the game
gives no Force (no Jedi class) is told so. Tested: the ends set exactly and a value past one
stops there, for every creature in every save (`the_ends_of_a_level_set_exactly`).

The data pipeline caches each script's analysis on disk (`ncsvm.run_script`, in
`build/questdb-cache`, keyed on the script, its parameters, the interpreter's
settings and its code), so a chapter rebuild takes minutes, and `rewards.py` works out the
chapters' credits in parallel.

Seen in the game: the Endar Spire skip from a fresh save gave 3,085 XP, the level-ups, the
loot, the escape pod movie and the vision, and the Taris apartment with Carth's opening.
**Not known:** random treasure rolled by scripts at death (left out); chapters after the
Endar Spire (Taris has had a milestone for some time; this line said otherwise until
2026-11-02); KotOR 2.

**The quest tracker** (Story, Quests; 2026-10-22, the maintainer: "what a person wants when
he enters the story mode ... is a quest tracker"): Story opened on Quests then (it opened on the timeline from 2026-10-24, and on Quests again since 2026-11-06, when the map became the Timeline tab), listed first before
the modder sections, and the party list steps aside. Left: the quests in three tabs, Active,
Completed (the journal entry is one of the quest's endings) and Not Started, each with where it
stands. Right: the picked quest, its current journal entry in full, and every stage as a
timeline (earlier stages filled, the current one a gold ring, later ones empty, endings
tagged, each with its XP); clicking a stage puts the quest there. Finish Quest opens a pop-up
listing the endings with their XP; the chosen one is set and its XP given to the player and
the party pool, once. The journal keeps only a quest's current stage, so "earlier" means a
lower stage number, not a stage the player is known to have seen.

**Removing what was spawned** (2026-10-22): a creature the editor spawned, picked on the map,
offers Remove This Creature; the area's own creatures do not (Story, Creatures still lists
every creature for modders). The editor records each spawn in `spawned.txt` beside its
settings as game, module, `ObjectId` and `Tag`; the game keeps a creature's id when it saves,
so the record finds it in later saves of the same playthrough. Creatures spawned before this
record existed (the first test Gamorrean) are not in it. Checked in the editor: a spawned copy
offered Remove, the area's own creatures did not.

**Spawning, tested** (2026-10-22): a template creature and a copy went into copies of four
KotOR and four KotOR II saves (`tests/edits.rs`), were written and read back: both there, the
new one with `CreatnScrptFird` 0, every creature and item id owned once. **Seen in the game**
(KotOR 1, 2026-10-22, by the maintainer): a Gamorrean spawned on the Ebon Hawk stood there
when the save loaded. **Untested in the game**: KotOR 2, whether the spawn script visibly runs, a template made for another module (its
conversation and scripts may be missing there), copies of story characters (scripts find
creatures by tag).

**The layout** (2026-10-22, the maintainer: "make the map the biggest thing"): on the Map tab
the party list steps aside and the map's frame fills the panel, the picture as large as fits;
a narrow rail on the right holds the area's name and a switch between Picked, Areas and Stuck?.
Picking anything on the map shows Picked; Spawn a Creature is always there, greyed until a
floor spot is picked. Over the map the pointer is a crosshair the editor draws in the accent
colour with a dark edge (the system crosshair is drawn inverted, so it looked see-through).

**Equipped** (Inventory tab, 2026-10-22): the game's equip screen, its slots three across in
the order of equip.gui (Implant, Head, Hands; Left arm, Body, Right arm; Left weapon, Belt,
Right weapon; KotOR II's second weapon set as a fourth row, equip_p.gui), each drawn as the
shape of the game's slot frames (lbl_top, lbl_mid, lbl_bot) so the rows interlock as in the
game. An empty slot shows the game's own picture of what goes there (`iimplant`, `ihead`,
`ihands`, `iforearm_l`, `iarmor`, `iforearm_r`, `iweap_l`, `ibelt`, `iweap_r` from the GUI
texture pack) on a faint accent fill; a worn item sits on the plain frame. Clicking a slot turns the list beside it into the party's items that fit (baseitems.2da
`equipableslots` and `droidorhuman`), each with Equip, and shows what is worn there with Take
It Off. In the editor, Revert and Save Changes sit at the top
right beside the settings button, with a line under them saying what is changed or what the
last save did; the backups folder, the version and the credit moved into Settings, so the tabs
reach the bottom of the window.

Not done, on purpose: walking through walls. A save only holds where things are; letting the
player pass through doors and walls would be a change to the running game (a KMRP patch),
not to the save. Open Every Door Here unlocks and opens the area's doors instead.

## 4. The bugs other editors had, and what this one does

From KSE's issue tracker and the forums ([sources](save-editor-format.md#6-save-locations-by-platform)):

| Reported against KSE | Here |
| --- | --- |
| Removing one Force power removed all of them (KotOR 2) | removes exactly one entry, from the class list and the level records; test `edits_survive_a_write` removes one of a save's powers and checks one fewer remain, in all 14 saves that have two or more |
| No way to add or remove Force powers | both, searched by name |
| Raising the level added the wrong Vitality (KotOR 1) | no level box: XP is raised and the game's own level-up runs, with its own hit dice and choices; a class added here adds exactly its hit die and, for the player, the level record the game sums Health from |
| Max HP and FP edits thrown away | the maximum is changed where the engine computes it from (level-record dice or base), never in the ignored `MaxHitPoints` field |
| Health and Force broken by level changes (a Discord report against another editor, issue 70) | levels, classes, Health and Force follow the engine's level-up and load rules; `tests/levels.rs` checks every save on the PC (section 8) |
| Inventory edits failed on commit | items are built from the game's templates and shaped like items already in the save; tested by adding two items to a save and loading it in the game |
| Saves not found: Steam Cloud, `kse.ini` paths, registry, VirtualStore, Linux | section 2, no configuration |
| Android KotOR 2 saves | Add a Save Folder; **untested** |
| Party member cannot be copied between saves | Bring from Another Save |
| No comparison of two saves' globals | Compare with Another Save |
| Override subfolders ignored (KotOR 2) | read, with the Workshop folder |
| Save pictures, player position, doors, containers, stores (nadrino's fork) | all here |

## 5. How a write stays safe

1. **Nothing is written until Save Changes.** Every change is held in memory; the bottom bar
   counts them, and Revert reads the save from disk again. Leaving a save with changes not
   saved is refused with a message, by Back and by a click on the timeline (the timeline had
   no check until 2026-11-02). Closing the window asks once; closing again with the same
   changes leaves without saving them.
2. **The game must be closed.** Save Changes checks for `swkotor.exe` / `swkotor2.exe`
   (`tasklist`; `pgrep` elsewhere) and refuses while it runs. A check that itself fails counts
   as running.
   The save's files must also be the ones read: their sizes and times are noted on open, and a
   difference refuses the write (`CHANGED_ON_DISK`), so a slot the game overwrote is not mixed
   with older edits.
3. **A backup first.** The whole save folder is copied to
   `Save Editor Backup/<save folder>/<date time>/` in the game's folder, beside the saves,
   before anything is written. A save outside the game keeps its backups in the folder that
   holds it.
   *Corrected 2026-11-02:* backups were in the folder that holds the save until then, and for a
   KotOR 2 Steam Cloud save in the game's `saves` folder. Steam Cloud syncs all of KotOR 1's
   `Saves`, backups too (`remotecache.vdf` listed them), so they moved out. Backups made before
   that date stay where they are and are not moved.
   After a save the editor says "Saved! Backup of the unedited save is in" and the folder.
   *Corrected 2026-10-22:* backups were kept in `<data>/KMRP/Save Editor/backups/` until the
   maintainer asked for them beside the saves. **Untested**: whether the game's Load screen
   ignores the `Save Editor Backup` folder (it holds no `savenfo.res` of its own).
4. **Only the changed files**, each written in full to `<name>.kmrp-new` first; only when
   every new file is written are they moved over the old ones.
5. **Lossless files.** GFF and ERF are read into a tree that keeps every field, its type,
   its order and every struct id, and written back from it. Test `every_save_round_trips`
   reads, writes and reads again every GFF and archive (nested module saves included) of
   all 14 saves on this machine and compares the trees: all equal. Byte identity is not
   claimed: the engine orders its struct table differently (1 or 2 of 9 to 74 GFFs per save
   came back byte for byte), which the game does not depend on.

## 6. Limits

| Limit | Why |
| --- | --- |
| Saves with external player state are read only | `pifo.ifo` holds a player copy the editor does not write. The measured KotOR 1 arrival autosave has no archived player, so the reader falls back to this file; the measured KotOR II autosave has both external and archived copies. Both are read only. Save Changes, Save as a New Save and rename are refused; Copy changes only the new copy's Load-screen name. Saving to a normal slot in the game gives an editable save when it has no external player file. Until 2026-10-29 the KotOR 1 arrival save failed to open. `a_save_with_its_player_in_pifo_opens_read_only` checks refusals and unchanged player files on temporary copies from both games. |
| Original Xbox saves are read only | the Xbox checks a signature on each save; re-signing needs the console's and the title's keys, and no verified procedure for KotOR was found. The maintainer chose (2026-10-22) to re-sign with keys read from the player's own files; **not built**: there is no Xbox save or game here to test it with |
| Nintendo Switch saves | open if they are plain PC-format folders; a compressed wrapper, if the Switch uses one, is not documented and not handled |
| Global variable list shows 200 rows at a time | drawing every row of 1,185 slowed the tab by over a second; typing part of a name narrows it |
| Names typed into the game | Windows-1252 only, as the Western releases store them |
| Two classes | the engine knows two |
| A quest stage's way | a save does not record how a stage was reached. `one_way` takes one way per stage, the one that suits the side played, the first listed on a tie. It can differ from what a player chose |
| Go Back and story values with no end value | a value a kept chapter also writes, with no known end value, stays as it is, and the report names it |
| The fourth planet on a return to the Leviathan | the save does not say which planet came last; it is the last free planet on the route, which the player can reorder on the galaxy map (until 2026-11-06: the last on the timeline) |
| The order free planets were played in | the save does not record it; the route takes the order each planet's own quests entered the journal, which mostly follows play but not strictly, and the player can reorder it |
| Load-screen portrait of the leader | savenfo `PORTRAIT1` and `2` follow the party, by hand and after a skip, a return or an import (since 2026-11-02); `PORTRAIT0` is the leader's and is left to the game's next save |
| Attributes shown with effects | an item equipped in the editor counts towards them only once the game has saved, because its effects are not in the save until then; the engine's caps and its rule for effects of one source are not modelled |

## 7. What is deliberately not changed

* `MaxHitPoints`, `MaxForcePoints`, saving throws, `ArmorClass`, item `Cost`, `CombatInfo`:
  recomputed by the game (section 1).
* `EffectList`, beyond equipping: equipment bonuses are effects created by the items, so the
  editor only removes the effects of an item it takes off (section 3a) and never edits an
  effect on its own. An equipped item needs no stored effect: the game applies an item's
  properties when it loads a creature and drops stored item effects (save-editor-format.md,
  "Effects left by removed items"; read in the executables, not yet seen in the game).
* `ObjectId`s of existing objects, struct ids, field order, labels. `Mod_NextObjId0` moves
  on only when the editor gives a new object an id (section 3a).

*Corrected 2026-10-24:* this list also said that equipment, the active party
(`PT_MEMBERS`, `PT_NUM_MEMBERS`), removing a class and lowering a level are not changed, and
that `Mod_NextObjId` is never touched. That was true when the section was written on
2026-10-22; the editor has done all of them since (section 3a: Equipped, With you, Classes;
`equip`, `unequip` and the party list in `src/save.rs`, `remove_class` and
`set_class_level` in `src/creature.rs`; the level rules in `tests/levels.rs`).
* Stale player copies in the other modules' `Module.ifo`: the game reads the last module's.
* Steam Cloud: nothing in Steam is touched; Steam sends the changed files itself.

Rejected along the way, and why:

| Tried or proposed | Why not |
| --- | --- |
| A tile dashboard (the first mockup) | the maintainer: "I dont want it to be tiles"; replaced by the game's in-game-menu shape, tabs and the party down the left |
| C++ with SDL3, .NET like the Font Converter, Tauri | the maintainer chose Slint: native on all three systems, no web view, runs under Wine |
| Arimo and system fonts | the maintainer: use KMRP's own fonts, the HD Original OTFs |
| Reading the GUI texture pack whole for the portraits | 300 MB of memory; the archive's index is read and each portrait read on its own (59 MB with a save open) |
| A level box | the KSE Vitality bug; see section 4 |

## 8. Verifying by hand, and what was verified

Automated (`cargo test --release` in KMRP's `save-editor/`, all passing 2026-10-22; since 2026-10-24 `build.ps1` runs the same tests as `cargo test --profile tests`, and `build.sh` and the GitHub workflow as `cargo test --release`):

| Test | What it checks |
| --- | --- |
| `formats::gff`, `text`, `globals` unit tests | GFF writer and reader agree; Windows-1252 both ways; booleans most significant bit first |
| `every_save_round_trips` | every GFF and archive of every save found (10 KotOR 1, 4 KotOR 2) survives write and read unchanged, and the save model opens each |
| `edits_survive_a_write` | on a copy of each of the 14 saves: Strength, a skill, XP, alignment, a feat added, one power removed, credits, an item added, influence (KotOR 2), a global switch flipped and an NPC's Constitution in all copies; written, read back, each change present, Dexterity and time played unchanged, the backup holding the old Strength |
| `a_new_class_brings_its_level_record` | a second class adds one level record, its hit die and a power list |

Levels, classes, Health and Force (`tests/levels.rs`, issue 70; 8 tests, all passing
2026-10-23 on the 19 KotOR 1 and 4 KotOR 2 saves on the maintainer's PC, each scenario on a
fresh copy, written and read back):

| Test | What it checks |
| --- | --- |
| `computed_maxima_match_what_the_game_wrote` | the editor's Health and Force maxima equal the `MaxHitPoints` and `MaxForcePoints` the game wrote, for all 62 creatures (player and party) in the saves the game wrote; storing the shown current value gives back the stored one |
| `health_and_force_set_directly` | maximum and current Health and Force set together on everyone; read back as set, classes unchanged, Force left at 0 where the engine gives none |
| `levels_up_then_down` | half-spent Health and Force, first class two levels up: Health full, Force up by its maximum's rise, base up by the dice, two records with the class's dice; down again: records, bases, maxima as before, Force as before; written two levels up and half spent, read back so |
| `classes_added_removed_and_changed` | a Jedi class added to a creature without one and an ordinary class to a Jedi, then removed; the first class changed to another and back; everything returns, what was missing stays missing; the player's Force follows the newest class |
| `levels_taken_away_newest_first` | the player two levels up, then levels taken from the newest record back, as Go Back does: the records and maxima as before |
| `healing_fills_both` | half spent, then healed: both at their maxima as the game will load them, party members with Toughness included |
| `rules_on_made_up_creatures` | the rules' corners no save here reaches: Toughness feats, feat 95, feat 116, an ability effect, the newest-class rule, a party member whose base is too low for the Con rule, droids |
| `recruited_companions_keep_the_templates_values` | a companion recruited from the game's template (Carth in KotOR 1, Atton in KotOR 2) loads with the template's own Health and Force, the stored maxima as the game writes them |

Every edit test also checks: the stored maxima are what the engine computes, the player has
one record per level, `PregameCurrent` is the current Health, and every other copy of the
character (the player's `pc.utc`, a party member's copy in the area) says the same. A save
whose records do not match its classes (`000023 - Game22`, changed by another editor: twenty
records with a hit die of 0 under a 20/20 multiclass) is left out of the change-and-back
check, as its records of two classes cannot be told apart.

**Untested in the games** (issue 70's acceptance list asks for it): each of the following,
on KotOR 1 and KotOR 2, loaded, the character sheet read, saved in the game, reopened in the
editor:

* a level raised on the player and on a party member (Health full, Force as shown);
* a class added (a Jedi class to a non-Jedi, and the reverse for the player, whose Force
  then reads 0) and removed;
* Health and Force set below the maximum on a party member with Toughness: the game should
  show the set value, and after a save and load it shows less by the Toughness bonus (the
  engine's own quirk, section 3.4 of the format document);
* then a normal level-up in the game, checking Health and Force move as on an unedited
  character.

In the games (the test copy of KotOR 1 and the Steam KotOR 2; the real saves were copied,
never written):

* KotOR 1: a copy of GalaxyMap with the name "Kmrp Tester", Strength 24, Charisma 16,
  Computer Use 14, a feat, a power removed and one added, credits 424,242, two items and an
  NPC's Constitution changed. It loaded; the character screen showed the name, Strength 26
  (24 plus the worn gauntlets' 2), Charisma 16 and Force 112/112 (the game's own
  recalculation from the higher Charisma); the skills screen Computer Use 14; the inventory
  424,242 credits.
* KotOR 2: the same changes on a copy of Game3, in the local `saves` folder (so Steam Cloud
  was not involved; removed afterwards). It loaded; the character screen showed "Kmrp
  Tester", Strength 24, Charisma 16.

The interface was checked at 1600x900 in dark and light mode, KotOR and KotOR II colours,
every tab and the pop-ups, by screenshots.

**Settings** (2026-10-22): the gear at the top right, on the list of saves and in the editor,
opens one pop-up, kept in `settings.txt` in `<data>/KOTOR Modern Gamesave Editor` (`%LOCALAPPDATA%` on
Windows, `~/Library/Application Support` on macOS, `$XDG_DATA_HOME` or `~/.local/share` on Linux).
*Corrected 2026-10-24:* until that day the folder was `<data>/KMRP/Save Editor`; the editor
moves it to the new name on its first start (`games::data_dir`).

| Setting | What it does |
| --- | --- |
| Text Size | Small (the first release's sizes), Medium (1.2 times, the default), Large (1.32 times); every text grows, the boxes around names and labels with it. *Corrected 2026-10-24:* Medium was 1.1 and Large 1.2 until that day (`ui/theme.slint`) |
| Appearance | Dark or Light; it replaced the round moon and sun button at the bottom right, which took a footer's height |
| Game Folders | the KotOR and KotOR II folders in use, each with Change (as KSE's `kse.ini` keeps `K1_Path` and `K2_Path`) |
| Save Folders | the folders added by hand, each with Remove, and Add a Save Folder (KSE's `K1_SavePath`, `K2_SavePath`, `K2_SavePathCloud`; the editor finds the usual ones itself) |

Folders change only from the list of saves: a new game folder can switch the game under an
open save. Edges of buttons and fields were raised to about 3:1 against the field they sit on
(they were about 1.8:1 and hard to see). Checked by screenshots at all three sizes, every tab
at Large. *Corrected 2026-10-22:* Large was first 1.3 times; at that size the Character
tab's labels, the attribute names and the Inventory headings ran into each other, so the
steps became 1.0, 1.1 and 1.2 (since 2026-10-24 they are 1.0, 1.2 and 1.32, with the
kerned fonts; Large at 1.32 has not been checked by screenshot on every tab). **Untested**: other window sizes than 1600x900
(the minimum is 1280x780), Linux and macOS, every place in section 2 marked untested,
importing a party member and comparing globals in the game itself, containers and stores in
the game.

To check a save by hand: open it in the editor, change one value, Save Changes; the line
under Save Changes names the backup folder (it was a bottom bar until 2026-10-22). Open the save in the game. To undo, copy the backup's files over
the save folder.

Verification on 2026-10-29: the original feat-record check assumed every owned feat
was listed in a level record. The edited Trial copy already owned feat 101 outside
that history, so adding Jedi Guardian correctly left it out of the new level record.
The check now carries unrecorded grants as already owned when replaying recorded
grants; their original award time cannot be recovered. It still requires every new
class grant to be owned and, in KotOR 1, recorded when first acquired. The explicit
`adding_a_class_does_not_claim_a_feat_the_player_already_owned` regression checks
that adding and removing the class preserves this earlier feat in both games. No
player save was repaired or excluded by folder number to make this check pass.

The 2026-10-29 `build.ps1` run passed all 46 tests and produced the executable in
`dist`. The focused Copy/Save As checks used temporary saves from both installed
games; the external-player-state refusal check used one autosave from each. The
editor was launched from `dist`. In-game overwrite of a newly copied save and
in-game loading after these changes remain **untested**.
