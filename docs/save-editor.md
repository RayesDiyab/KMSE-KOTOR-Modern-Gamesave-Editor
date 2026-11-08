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
| macOS build | `dist/KMSE.app` from `build-macos.sh` (Universal 2); built and run on an Apple M5 Mac, macOS 27.0.1, 2026-11-08 ([macOS audit](audits/MACOS_PORT_AUDIT.md)). *Corrected 2026-11-08:* until then this row said the Linux and macOS builds were not built or run |
| Linux build | `build.sh`; **not built or run yet** (no Linux machine here) |
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
| Aspyr ports | KotOR 1 Mac: `~/Library/Application Support/Knights of the Old Republic/saves` and the App Store container; KotOR 2 Linux: `~/.local/share/aspyr-media/kotor2/saves`; KotOR 2 Mac: two Application Support paths | KotOR 1 Mac (Steam): **yes**, 2026-11-08. The rest **untested**; paths from Aspyr's FAQ ([save-editor-format.md](save-editor-format.md), section 6) |
| Mac App Store containers | `~/Library/Containers/<id>/Data/Library/Application Support/<any>/saves`, ids `com.aspyr.kotor`, `com.aspyr.kotor2.appstore`, `com.aspyr.kotor2` | **blocked**: macOS closes another app's container; the editor names such a folder in a pop-up ([macOS audit](audits/MACOS_PORT_AUDIT.md), F) |
| Beside a Mac app | `saves` and `cloudsaves/<account>/` in the folder that holds the game's app (Steam's KotOR II keeps `cloudsaves` beside `KOTOR2.app`) | **yes**, KotOR 2 Steam, 6 saves, 2026-11-08 |
| Mac installs | Steam libraries as above; `/Applications`, `~/Applications`, and Spotlight by bundle id `com.aspyr.kotor*`. An app stands for `Contents/Assets`, `Contents/GameData` or `Contents/KOTOR Data` | **yes**: all four Mac copies (KotOR and KotOR 2, Steam and App Store), 2026-11-08 |
| Phone, Switch, another PC | Add a Save Folder | **untested** (no such saves here) |
| Original Xbox | recognised by `SaveMeta.xbx` or `SaveImage.xbx`; opened to look at, never written (section 6) | **untested** |

Which game a folder holds: its executable or ini (`swkotor2.*` is KotOR 2), else the bundle
id of the Mac app it is in (`com.aspyr.kotor2*` is KotOR 2), else its first module
(`end_m01aa` KotOR 1, `001ebo` KotOR 2); a folder counts as a game only with a KEY
`chitin.key`, a TLK `dialog.tlk` (the App Store KotOR 2's is `Localized/English/dialog.tlk`)
and a `modules` folder. *Corrected 2026-11-08:* the last step was the size of `chitin.key`
(KotOR 2 above 700,000 bytes), which took the Mac App Store's KotOR 2 (406,035 bytes) for
KotOR 1. Which game a save belongs to: KotOR 2's `savenfo.res` has `PCNAME`,
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
| Character | Add a Class | a second `ClassList` entry at level 1, its hit die added to `HitPoints`, its Force die to `ForcePoints`, for the player one more `LvlStatList` record, XP raised to the new level if short; then Health full and Force up by what its maximum rose, as the game's level-up leaves them | only when there is one class; droids get droid classes only; never Minion; KotOR II's Tech Specialist for party members only, and a prestige class only for a player with one Jedi class at level 15 or above (by class level or XP), as Kreia offers one; Change Class follows the same rules |
| Character | Look | `PortraitId`, `Appearance_Type` (the portrait's body in the current build: `appearance_s`, `appearancenumber` or `appearance_l` of `portraits.2da`), `Gender` | portraits with `forpc` 1 |
| Character | Position (player) | `XPosition`, `YPosition`, `ZPosition` | numbers only |
| Character | Heal Everyone Fully | `CurrentHitPoints` and `CurrentForce` such that the game loads both at their maxima, for every creature (the player: equal to `HitPoints` and `ForcePoints`; a party member: above them by the bonuses the load leaves out) | |
| Abilities | Skills | `SkillList[i].Rank` | 0 to 99 |
| Abilities | (how they are shown) | every feat and Force power the game has is listed, each with its icon (feat.2da `icon`, spells.2da `iconresref`), in groups: feats by `toolscategories` (Jedi, Combat, Weapons and armour, Class abilities, Skills and general, Droid and other), powers by `goodevil` (Universal, Light side, Dark side), a chain's links together in order; what the creature has is lit. A row is as tall as its name, and the list keeps its own width whatever the names: a long name (KotOR II's German "Geschwindigkeitsschub") widened every row until 2026-11-06, which pushed the add and remove column off the right edge, so in KotOR II nothing could be added or removed | names, icons and groups come from the game and Override, so modded feats and powers appear with their own icons |
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

**A save without a picture** (since 2026-11-08, the maintainer). Save Changes, Save as a New
Save, Copy and the saves page's rename give a save that has no `Screen.tga` one: the load
screen of the module it will load in. That is the module of a trip a story move or Travel
Here queued, else `LASTMODULE`. The picture is the game's texture `load_<module>`, else
`load_default` (Override, the GUI texture pack, then the archives). It is made 256 x 256 with
a box filter and written as the game writes the file
([format, 1.6](save-editor-format.md#16-screentga-save-thumbnail-file)): the same 18 header
bytes as the game's, 196,626 bytes. A save that has a picture keeps its own; the editor never
replaces one.

It is written only after the write went through: staged as `Screen.kmrp-new`, flushed, moved
in, no file replaced. If that step fails, the save is as the write left it. A write that is
refused adds nothing. A revert to a backup without a picture takes it away again.

Measured: every KotOR and KotOR 2 area read has `LoadScreenID` 0 (`LOAD_DEFAULT`), so the
`.are` field is not used. Each module's own texture exists in both games' GUI pack
(`load_end_m01aa`, `load_203TEL`). Checked by `tests/save_picture.rs` on copies of real saves
of both games, and by looking at the pictures made for `end_m01aa` and `203tel`. Not yet
seen on the game's Load screen.

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
| Time played | Advanced, Story Values (above the list) | `PT_PLAYEDSECONDS` (savenfo follows) |
| Quests | Quests | `JNL_Entries`: a quest at a stage of global.jrl |
| Copy inventory between saves | Inventory, Copy from Another Save | the other save's `INVENTORY` item list |
| Xbox .sig files | not done | see section 6 |

Area creatures (in the fork the maintainer showed) are listed in Advanced, Creatures, with a
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
| Finish Quest | the quest's stages with `End` 1 in global.jrl; as if played since 2026-11-06 (the quest tracker, below): the steps on one way there, and for each quest stage reached that is new, XP = plot.2da `xp` x the stage's `XP_Percentage`, added to `Experience` and `PT_XP_POOL` |

**Travel and Skip Ahead** (2026-10-23; played by the maintainer in KotOR 1):

| What | How |
| --- | --- |
| Any area | The Map tab's Areas list holds every area of the game, by planet (cutscene stages, `stunt_` modules, and nameless modules left out; of two areas with one name the visited one, else the one a door leads into). An area not visited shows the game's own .git with each object's template fields filled in (names, portraits) |
| Travel Here | The ways the game's doors and transitions lead into the area (each module's shipped .git, `LinkedToModule` and `LinkedTo`). Picking one queues an area-transition event (`EVENT_AREA_TRANSITION`, 26) first in the current module's `EventQueue`, due at the saved moment: the game makes the trip right after the save loads, loads an unvisited area fresh from its files with its arrival scripts, and brings the party (`Save::queue_transition`; the event's layout from the maintainer's own reverse engineering, handler 0x0050D6C0). *Corrected 2026-10-23:* queued last and a second later, it waited behind the save's own delayed events until the player did something |
| Skip Ahead | Timeline (Skip to Here in a chapter's pop-up): a chapter as if played (`questdb::skip_to`). The chapter's milestone (`data/quests/*-milestone.json`): journal, the story values the rest of the game reads, items; its rewards (`*-rewards.json`, `tools/questdb/rewards.py`): the XP of a full run (kills by the engine's rule, plot XP, script XP, SetXP floors, `tools/xp/calc.py`), the enemies' Dropable items and the containers' loot (class gear by the game's own tests); then the trip by the game's own first hop with its movies, so the story plays on (from the Endar Spire: escape pod movie 01c, the vision, then the Taris apartment) |
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
section tabs above it the map is 453 px tall at 1600 by 900, where it was 380 px. Story kept Quests and Planets, Pazaak and opened on
Quests; later that day the maintainer made each a tab of its own ("put Pazaak & Destinations
as a seperate menu tab / make Story called Quests"): **Quests**, then **Pazaak &
Destinations** (planets and Pazaak, numbered 8 so the other tabs keep their numbers). Where the
whole name does not fit beside the other tabs (measured unseen, the tab row with that name
against the room; at 1280 by 780 it does not) the tab is "Pazaak & Dest." (the maintainer's
short name), measured the same way, and "Pazaak" only where even that does not fit. On a window
under 1500 px the tabs' words are 16 px times the text size with 16 px after each. Its changes, `party.planet`
and `party.pazaak`, put the orange mark on that tab. Its cards are five a row where the section
is narrower than 1250 px, so the planets keep the room for their whole title, and a card's
column is never narrower than 76 px times the text size (at 1280 by 780 a 74 px column cut the
count between its buttons). Later the same day the maintainer asked for everything around the map to be in it
("Include the planet order in the map itself / Meaning expand map size"; "Remove the top text,
and incorporate it into the map itself ... like small print"; "Remove the outer border and make
the map the main piece on this timeline page"): the map is now the page, with no panel around
it, 624 px tall at 1600 by 900 (from 240 to 864, measured by its corner marks on a screenshot). Where the save is
and how to use the map are centred small print along its top, the zoom is in its top right
corner, and the planet order is along its bottom. The map is a dark
display in light mode too, on the generated starfield (`ui/editor.slint`, `GalaxySection`;
`fill_skip` in `src/main.rs`):

| What | How |
| --- | --- |
| Stops | each main chapter where the game's own galaxy map puts its planet (`MAP_STOPS` in `src/main.rs`; moved on 2026-11-06 where names and tags ran into each other at 1280 by 780: KotOR's Endar Spire up, Taris down, the Leviathan up and Tatooine right; KotOR II's Ravager clear of the Ebon Hawk beside Telos, and Malachor V right; the planets as the editor's Planets page spreads them), drawn as the game's own planet texture wrapped on a globe (`globe_pixels`, read from the player's install); the ships (Endar Spire, Leviathan, Ravager) and stations (Star Forge, the masters' meeting) drawn by the editor (since 2026-11-06 the Endar Spire, the Leviathan and the Ravager as their pictures in `assets/timeline`, a little larger than their rings, the long ones wider; the Ebon Hawk waits 1.3 of a stop's size beside the save's stop, 1.15 before, where it touched the Leviathan's picture), near the planet the story meets them at. Each carries its number on the route |
| The route | the main chapters in the order the save plays them (`questdb::story_route`): outside the free planets the story is one line; the free planets (KotOR: Tatooine, Kashyyyk, Manaan, Korriban; KotOR II: Nar Shaddaa, Dxun with Onderon, Dantooine, Korriban) come done first, then begun, then not visited; KotOR's Leviathan after the third. Done: the chapter played (`chapters_played_in`, where KotOR's `K_STAR_MAP_*` TRUE counts as played). Begun: the save has been to one of the chapter's areas (`questdb::been_to`: the areas the save keeps, and the one it is in), or KotOR II's master marker for it is above 0 (`351NAR_Ebon_Released`, `506OND_End`, `602DAN_End`, `702KOR_Vash`; it can be set before the planet's end). **Corrected 2026-11-06:** the first version took a quest of the planet's in the journal for a visit, and KotOR II hands out a Nar Shaddaa quest on Telos, so a save on Telos showed Nar Shaddaa begun; and it took KotOR II's markers for done, which Skip Ahead did not, so a planet could read done on the map and be played again by a skip |
| The order of what is done | the save does not record the order planets were played in. The route takes the order in which each planet's own quests first appear in the journal (`JNL_Entries` keeps new entries at its end). Read from four of the maintainer's KotOR saves on 2026-11-06 (`examples/journal_order.rs`): the entries mostly follow the order of play, but not strictly (one KotOR save lists an Ebon Hawk quest before Taris's `tar_escape`), and `JNL_Date` is a day count of 0 or 1 with `JNL_Time` inside a day, so neither orders the planets. Where the journal guessed wrong, the arrows reorder what is done among itself. The begun and the not visited move among themselves too, so a planet just landed on can wait while another is skipped to |
| Bands of light | lit blue lanes join the stops the save has played or begun, in route order; faint lanes run on along the route ahead. Each lane is a quadratic curve bending a little to one side (`lane` in `src/main.rs`) |
| You are here | the stop whose areas hold the area the save will be in (`Save::place`: a trip waiting in the queue, which Skip Ahead, Go Back and Travel Here set and the game makes as the save loads, else the area it is in; `questdb::chapter_of_module`, by each chapter's `cleared` areas): a gold ring and gold name, and "YOU ARE HERE" with its name in gold small print centred along the top of the map (a pill above the map until 2026-11-06). At Yavin Station, "At Yavin Station". On the Ebon Hawk (or an area no chapter lists) it is the last stop played, "Aboard the Ebon Hawk, last at ...", and the Ebon Hawk is drawn beside it. While no trip waits, the chapters before the stop the save stands in, on the story's line, read played (`chapters_played_in`); with a trip waiting, the journal alone decides. **Corrected twice on 2026-11-06:** the first version used the area the save stood in even with a trip waiting, which after Go Back is still the area gone back from: a return from the Unknown World to Kashyyyk read every planet played again, and a later Skip Ahead would not have played them. The second used the earlier of that area and the trip's, but Go Back's trip leads to the Ebon Hawk or a cutscene area, not to the chapter gone back to, so the same happened (`tests/audit_round2.rs`) |
| States | played (blue number, blue ring), the one the save is in (gold), begun and not finished (blue outlined number), the next stop (brighter), ahead (dim). Pointing at a stop shows a card with its number, state and chapter title |
| Planet order | along the bottom inside the map, centred and only as wide as it needs, with nothing behind it but the map (the buttons have their own backgrounds): the free planets in route order, each with arrows to move it one place earlier or later among those of its kind (done, begun, not visited). Where the map is at least 1400 px wide, "PLANET ORDER" stands before them with "Skip Ahead follows it" under it, which becomes the link Undo order changes once the order is changed; on a narrower map Undo order changes floats centred just above the planets. Either way nothing moves when it comes. The stops stand clear of it at zoom 1 (`map.bar-h`). Where the map is under 1300 px wide the buttons are tighter, so KotOR II's four fit at 1280 by 780. Until 2026-11-06 it was a row under the map with a note and Undo on a second row, and the map shrank when Undo came. The order changes nothing in the save; Skip Ahead and Go Back follow it. It is kept until another save opens |
| Played | a chapter's journal entries reached, or its journal quests taken out of the journal by a chapter the save has played (`chapters_played`); since 2026-11-06 also every chapter before a played one on the story's line, and all the free planets before a chapter after them (KotOR's Leviathan excepted, which follows the third). A KotOR save on the Ebon Hawk after the Leviathan had no Dantooine entry left and read Dantooine as never played, so the map put the save in Dantooine |
| Other saves | every other save of the game, any character's, as a tag with their number high on the right of the furthest stop each has played on its own route; green (blue in KotOR II); pointing at a tag of several opens a ring of them (kept inside the map and never taller than it: sixteen saves at KotOR's Endar Spire ran off the map's top until 2026-11-06), pointing at one shows its picture, name, character, level and XP, how far its story is and where it loads. A click asks first (since 2026-11-06, the maintainer: "there needs to be a warning where you can cancel"): a warning names the save, its character, level and story, says the save open now closes and that nothing is written, and offers Cancel (or Escape) and Open This Save. Changes not saved still stop it, as before |
| Zoom and move | the wheel zooms about the pointer, from all of the map (1) to four times (`map.zoom`; each step 1.25); the planets' distances grow with it and the globes by its square root, so they spread apart. Dragging a part of the map with nothing on it moves it, never past the map's own edges at that zoom; a press on a planet still opens it. A double-click, or Fit, shows all of the map again, and so does opening another save. Out, in (1.5 a press) and Fit sit in the top right corner, where no stop is; "Scroll to zoom, drag to move" is in the small print along the top. Since 2026-11-06, the maintainer: "make the timeline zoomable and movable with mouse"; checked in the program in both games at 1600 by 900 and 1280 by 780 |
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

Going back uses the story's items and credits by default (`keep_items = false`). It
removes mapped rewards still held, bought ones of the same kind too (the save cannot tell
them apart), and restores what the story operation restores. This is not an earlier snapshot
of everything the player bought, sold or used; that limit still applies. **Correction:** the
earlier interface offered a keep-items choice, and this reference described a separate
confirmation with a button on each row. The choice was removed; the selected row's prepared
result is reviewed and applied in this one popup.

**Keep my items and credits (2026-11-07, the maintainer; audit S-8).** The choice is back, as
a switch under the pop-up's "Inventory changes", off each time the pop-up opens. On, Go Back
runs with `keep_items = true`: no reward is taken back, credits stay, and the real items a
companion who leaves has on go to the party inventory (`Save::unequip`). Their starting gear
still in template form (only `EquippedRes`) stays with them: the game makes those items as
the save loads, and the inventory changes no longer count them for anyone
(`story_preview::accessible_items`). Checked by `tests/story_preview.rs`: with the switch on,
no inventory or credit change in either game (9 and 34 kinds of item taken back with it off).

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

**The quest tracker** (the Quests tab, Story, Quests until 2026-11-06; 2026-10-22, the maintainer: "what a person wants when
he enters the story mode ... is a quest tracker"): Story opened on Quests then (it opened on the timeline from 2026-10-24, and on Quests again since 2026-11-06, when the map became the Timeline tab), listed first before
the modder sections, and the party list steps aside. Left: the quests in three tabs, Active,
Completed (the journal entry is one of the quest's endings) and Not Started, each with where it
stands. Right: the picked quest, its current journal entry in full, and every stage as a
timeline (earlier stages filled, the current one a gold ring, later ones empty, endings
tagged, each with its XP); clicking a stage puts the quest there. The journal keeps only a
quest's current stage, so "earlier" means a lower stage number, not a stage the player is known
to have seen.

**Stage order, corrected 2026-11-06.** The stages were listed by number, and some quests are
not numbered in story order: KotOR's Carth sets stage 3 ("Carth's revenge is complete, Saul
Karath is dead") on the Leviathan, after stage 65, with a journal call that may lower the stage
(`bAllowOverrideHigher`; the quest data keeps that remark on the step). It came first, read
"The Beginning" and offered "Start the Quest". Such stages (`questdb::out_of_order_stages` since
2026-11-07; before, every stage with that remark, `questdb::lowered_stages`) now go
just before the current stage when the save has been where they are set (it keeps that area, or
the chapter the area belongs to is played; a save after the Leviathan no longer keeps the
Leviathan's areas), else after the other stages; the endings come last, and a stage set out of
order counts as passed by the same test. Seen in the program on a KotOR save after the
Leviathan: Carth's stages read ..., "won Carth's full trust", "revenge is complete" (passed),
"devastated" (current), the three endings.

**Skip Ahead and Go Back leave the player leading (2026-11-07).** Both set `PT_IS_LEADER` to 0
on every `PT_MEMBERS` entry and `PT_CONTROLLED_NP` to -1 in PARTYTABLE.res
(`Save::release_controlled_npc`). The game reads a member marked leader as the party member it
controls (CSWPartyTable::LoadTableInfo) and queues a switch to that member on every area
load. Found by KMRP's agent in the game: a save skipped from the Endar Spire kept slot 0
(Trask there) marked; at the end of Taris slot 0 is Bastila, and after the turret game the
switch fired where her creature was not and the game crashed in `CrownFutureLeader`. The same
save with the flag cleared played on to Dantooine. Checked by
`tests/audit_2026_11_07.rs` (`a_story_move_leaves_the_player_leading`), which fails without
the change on the maintainer's own Endar Spire save.

**Travel sets the current planet, KotOR only (2026-11-07).** A move to another module, by
travel or by a story move, sets the global number `K_CURRENT_PLANET` in GLOBALVARS.res to the
value the game writes for the target's planet (`Save::set_planet_for`), and only when it
differs. The values, measured in the game's own saves and read in the Ebon Hawk's arrival
scripts (`k_ebn12_<planet>`): `end_` 5, `tar_` 10, `dan` 15, `kas_` 20, `man` 25, `korr_` 30,
`tat_` 35, `lev_` 40, `unk_` 45, `sta_` 50. The Ebon Hawk keeps the planet it left. KotOR 2 has
the global but never sets it (0 in every save of it here, on Peragus and Telos), so it is not
touched. Without it, after a trip
from Manaan to Dantooine Juhani still spoke of Manaan (the maintainer, in the game). Checked
by `travel_sets_the_current_planet`.

**A clicked map spot keeps out of the ways out (2026-11-07).** A spot clicked on the map moves
to the nearest walkable point that is clear, with 0.75 to each side, of every transition
trigger's shape in the area's GIT (`Area::clear_of_exits`), as the offered exit spots already did. Stored area copies now keep
each door's and trigger's own `Tag`. It keeps out of the area's objects too (`Area::clear_spot`):
each placeable's walkmesh (its model's `.pwk`, from `placeables.2da` `modelname`, turned by its
bearing about its position), with a character's reach of 0.45. Clicked at the middle of 2,186
placeables in the first 60 modules of each game, 2,125 would have put the player inside one before
(`exits.rs`).

**A small window shows everything smaller (2026-11-07).** The layout is drawn for 1280 by 780
logical pixels or more. When the window has fewer real pixels than that at Windows' scaling
(150% on a 1920x1080 screen leaves about 690 of height), `fit_to_window` in main.rs lowers the
window's scale factor until 1280 by 780 fits, by dispatching Slint's ScaleFactorChanged and
Resized events every tenth of a second when the size calls for it. Text, pictures, tables and
spacing then shrink together and keep their places; at the design size or larger Windows' own
factor is used, and a change of it (another monitor) is followed. The window can be made as
small as half the design size. Run on this PC at 1100 by 680 and 860 by 530.

**The audit's smaller findings (2026-11-08).** Each is listed with its reason in
`docs/audits/WINDOWS_FULL_AUDIT.md` (section H) and checked by `tests/audit_p3.rs` and
`tests/audit_p3_formats.rs` where a test can reach it. What a player notices: Revert asks first;
Skip Ahead and Go Back show what they cannot do before Apply, and are worked out again when the
save was edited since; a member the story keeps out of the party cannot be put in it; Change
Class swaps the classes' own feats; a feat added without its prerequisites, XP below the level's
need and a portrait that changes the gender are said, not refused; KotOR's star map quest names
the map found last, as the game does; a planet shows on only when the galaxy map has both of its
bits; a game folder the editor cannot use is refused with the reason, and backups go to the
editor's own folder when the game's cannot be written; a module name from a save must be a plain
name before it becomes a path.

**Advanced, Creatures asks before removing (2026-11-07).** Removing a creature from the area
asks first, naming it and saying what removing does; a party member's own copy is still
refused. An edit of a party member no longer changes a copy of them the editor spawned
("Copy of one in this area"): `Save::edit` leaves alone the creatures whose object ids
`spawned.txt` records for the area, and those it spawns itself (`Save::spawned`). Checked on a
save where companions stand in the area (`a_spawned_copy_of_a_party_member_is_not_edited_with_them`).

**Stages in the story's order (2026-11-07).** A quest's stages are taken in the order the story
reaches them (`questdb::story_order`) by the Quests tab, Finish Quest (which stages are still
ahead, and which give journal XP) and Skip to stage (which are passed, and where the journal
ends up). That is number order, except for a stage set out of number order
(`questdb::out_of_order_stages`): a step may lower the journal to it (`bAllowOverrideHigher`),
and no later-numbered stage short of an ending is set in a chapter it is set in. In KotOR
that is Carth's 3 (the Leviathan) and Bastila's 1 and 2 (the Leviathan, the Unknown World); in
KotOR II none. Such a stage goes just before the current stage when the save has been where it
is set, else after the others; endings last. By number, Carth's quest at 3 replayed his talks
10 to 70 (they set `K_SWG_CARTH_LEVEL`) and paid their journal XP again. Checked by
`tests/finish_quest_audit.rs`.

**Party XP from quests (2026-11-07).** Journal and step XP go to the player and `PT_XP_POOL`;
companions are not given theirs directly. They need not be: both games raise each companion
to 80% of the pool as the save loads, as the maintainer saw in the game. In KotOR, Carth
brought from Taris showed 69,389 of a pool of 86,735. In KotOR II, a Telos save with the fuel
quest finished (the pool 1,100 higher, the companions left as they were in the file) showed
Atton and Kreia at 28,993, 80% of the new pool of 36,240 rounded up, where the file held
28,258 and 28,257. A finish that moves another quest forward also gives that quest's journal
XP, as the game does: KotOR's AddJournalQuestEntry (0x005483F0, the maintainer's
reconstruction) pays the new stage's XP for whichever quest a script names, and only when the
stage goes up, which is the rule Finish Quest follows.

**Finish Quest, corrected 2026-11-07 (the Windows audit).** The quest data lists every path
through one conversation line or script as its own step (a reply chosen, a condition met, an
"otherwise"). Finish Quest applied all of them together, so KotOR II's DroidRetrieval stage 20
paid Kodin on 63 paths (46,400 credits) and other quests gave rewards two to four times. Now
(`questdb::one_way_in`) the steps of one way are grouped into sets of alternatives: the same
line reached the same way, with the same kind of last condition (the node's own script, "before
this line: ..."; the line's own test, "if ..." or "otherwise"; or the line that follows a
reply, "line E66"). One step of each set is taken: the first a condition on story values says
holds ("if DAN_ELISE_PLOT != 0", "if 501_FIRST_ENTER is TRUE"), read from the save as the run
goes, so a value an earlier stage sets counts; else the first that does something; else the
first. Steps at different points of a line are all run. A step's `needs` are not read: they
hold what reaching the line takes, which the run itself sets on the way. A first version of
this fix took one step per way and was wrong: a second review found it dropped the 110 XP of
KotOR II's tutorial_3CFD and KotOR's dan_murder; `tests/finish_quest_audit.rs` now checks the
XP of those stages against values worked out by hand from the data. Steps that are not
paths of one line are each kept, unless the save says their condition fails. Also: KotOR's star map quests set this planet's map flag
only, and the maps are counted the game's way (all four were found before); the companions a
step recruits or lets go (`AddAvailableNPCByTemplate`, `RemoveAvailableNPC`) are recruited or
let go, as Skip Ahead does (they were only listed); and a quest at an ending already, or at
the stage asked, is refused (finishing it again gave the rewards twice). Checked by
`tests/finish_quest_audit.rs`. What stays unverified in the game: the paths taken where the
save cannot decide (a conversation's local flags, a reply), and whether journal XP reaches the
party the way the game hands it out (see the audit's remaining issues).

**Finish Quest, as if played** (2026-11-06, the maintainer: "it shouldn't be [optional] ... it
should give you the exact same items, class, changes, everything ... If there are options
that you have to pick, then you can pick that as well"). Until then a pop-up listed the endings
with their XP, and a switch, "As if played", added the quest data's steps, every way through a
stage at once and branch steps only when ticked. Now it is always as if played, from where the
save stands (`questdb::finish_quest`, `story_preview::Plan::Quest`): every stage after the
quest's current one takes one way, the one that suits the side (`questdb::one_way`, as Skip
Ahead does), and its steps are done: journal stages, story values, items (credits as the game
adds them), credits, XP, alignment, doors and the flags people read. The script lines the data
keeps as text are done where the editor can do them as the script does: a journal quest
removed, a story counter raised by a number or set to the player's level or another counter, XP
of the player's level times a number. Each quest stage those steps reach that is new to the
save gives the journal's own experience (plot.2da `xp` times the stage's `XP_Percentage`); the
quest data's XP and the journal's are separate (measured that day: in KotOR 74 stages give XP
by their steps alone, 135 by the journal alone, 3 both, never the same amount), so both are
given. The quest then stands at the ending.

The pop-up shows the endings in full on the left (a long one began mid-sentence in a box of
88 px), and "Your path", light or dark, where the side changes what this ending does from where
the save stands (`questdb::finish_side_matters`; in the data, 28 of KotOR's 101 quests and 26
of KotOR II's 112 play differently by side). On the right, the result is prepared on a copy and
measured as Skip Ahead's is: the values before and after, what is gained and lost (companions,
feats, powers, journal quests finished, begun, moved on or taken out), the inventory changes,
and what only the game does (a cutscene, a conversation, people leaving an area the save has
not been to). Finish Quest takes exactly that result. Checked by `tests/finish_quest.rs`: every
quest under way in three saves of each game, finished at each ending on both sides (84 in
KotOR, 52 in KotOR II), stands at the ending, never loses XP, and leaves the save it was
prepared from untouched. Seen in the program: Genoharadan: Ithorak's "bounty collected" ending
gives 1,000 XP and the GenoHaradan Power Gloves; a KotOR II bonus quest 1,050 XP (1,000 from
its script, 50 from the journal). **In the game, untested.**

An ending is tagged LIGHT SIDE or DARK SIDE where its own stage raises or lowers alignment on
both its ways (`questdb::ending_side`; an earlier stage every ending shares says nothing about
any one of them). Counted on 2026-11-06: in KotOR II 10 endings read light, 12 dark, 221
neither. In the list of changes, what goes up is green, what goes down red and the value after
gold, as everywhere a change is shown (DESIGN.md, the Change Colours Rule); the right side is
one scroll, so a reward's items no longer hide the last changes.

**The story pop-ups in the game's look** (2026-11-07, the maintainer, approving the mock-up:
"this looks amazing ... for finishing the quests, and the timeline as well"): Finish Quest,
the timeline's travel pop-up and Go Back are drawn as a screen of KotOR's own menus (DESIGN.md,
Story Dialog): a framed dialog, a gold title, framed panels with lit section headings, and a
change table with an icon on each row. The icons are the games' notification icons traced
into vector outlines (XP, credits, level, the journal, an item received or taken, KotOR II's
party; alignment is the dark side's icon and the light side's, half each), drawn sharp at any
size in the open game's colours; nothing is read from the game for them. Health, Force,
influence, class, attributes, skills and components show the game's own icons, read from the
open game (`game_icon` in main.rs: the Cure power's, the Force and influence gain and loss
icons, KotOR's Force Sensitivity icon, the Character, Abilities and Inventory menu icons). The pop-ups are sized
for a window of 1600 by 900 and grow in step with a larger one. "What only the game does" is now **Happens in the Game** (the
maintainer found it unclear): what the game does itself when it gets there. Looked at in the
program in both games at 1600 by 900 and in KotOR at 1280 by 780.

**The quest view, restyled 2026-11-06** (the maintainer: it "looks a bit like not a part of the
rest of the program"; reviewed with the Impeccable design rules): spaced section labels
(JOURNAL ENTRY, STAGE 2 OF 6) over plain text on the panel, as the other tabs have, where the
journal entry and the picked stage sat in bordered boxes that read like input fields, the stage
in a yellow frame; its buttons are outlined, as the one filled button of the editor is Save
Changes; Finish Quest, Start Quest and Take Out of the Journal sit along the bottom behind a
hairline. A stage's XP is green.

**Removing what was spawned** (2026-10-22): a creature the editor spawned, picked on the map,
offers Remove This Creature; the area's own creatures do not (Advanced, Creatures still lists
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
Right weapon), each drawn as the
shape of the game's slot frames (lbl_top, lbl_mid, lbl_bot) so the rows interlock as in the
game. An empty slot shows the game's own picture of what goes there (`iimplant`, `ihead`,
`ihands`, `iforearm_l`, `iarmor`, `iforearm_r`, `iweap_l`, `ibelt`, `iweap_r` from the GUI
texture pack) on a faint accent fill (KotOR II's pictures fill their texture, KotOR's leave
the frame's margin, so KotOR II's are drawn inside the frame); a worn item sits on the plain
frame, as large as the frame's inside (since 2026-11-06; it was half the slot). The grid is
centred in the room under its label. **KotOR II** has the same 3 by 3 grid; its second weapon
set (equip_p.gui) takes the weapons' places when "Weapons 2" is picked under the grid, as the
game's swap button does. Until 2026-11-06 the second set was a fourth row, which made every
slot smaller (the maintainer: "make it same layout as kotor 1"); KotOR II's credits,
components and chemicals sit side by side above it for the same room. Clicking a slot turns the list beside it into the party's items that fit (baseitems.2da
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
   when the save is opened, and the copy is read back and compared with the save (where the
   newest backup already holds exactly these files, that one is kept instead of a second
   copy). Before anything is written, a read-only health check compares that backup with the
   save on disk, byte for byte; where it is missing, damaged or out of date (a second save in
   one session), a new backup is made and checked the same way, and the write stops if that
   fails. *Changed 2026-11-07, the maintainer:* until then the backup was made at saving.
   A save outside the game keeps its backups in the folder that holds it. The History button,
   between Copy and Settings, lists the open save's backups oldest first, the one the next
   save checks marked, each with Show in Folder. A backup picked there and **Revert to This
   Backup** (asked once more, with Cancel) puts the save back to it through `save::restore`.
   It is refused where Save Changes is (an Xbox save, `pifo.ifo`, the game running), for a
   folder that is not one of this save's own backups, for a backup that does not open as a
   save of the same game, and where the save is already that backup. Otherwise:
   1. the save as it is is backed up and that copy checked, so the revert shows in History
      and can be undone;
   2. each file of the backup is written beside the save as `<stem>.kmrp-new`, flushed and
      read back;
   3. the files are swapped with the old ones waiting as `<stem>.kmrp-old`, so a failure part
      way puts every one back;
   4. the result is compared with the backup byte for byte and opened as a save.
   Changes not saved are lost, and the pop-up says so. `tests/restore.rs` runs all of this on
   copies, including a file held open part way through the swap.
   Each backup made since 2026-11-07 has a record beside it, `<backup folder>.source`, naming
   the save folder it was copied from (by its path inside the folder that holds the backups
   folder, so a moved game keeps its records; by its full path when it is not in there), so two saves with one folder name (two Steam accounts'
   cloud saves, a copy kept in the game folder) share a backups folder without seeing each
   other's backups; backups made before that have no record and are listed for every save of
   their folder name, as before.
   *Corrected 2026-11-02:* backups were in the folder that holds the save until then, and for a
   KotOR 2 Steam Cloud save in the game's `saves` folder. Steam Cloud syncs all of KotOR 1's
   `Saves`, backups too (`remotecache.vdf` listed them), so they moved out. Backups made before
   that date stay where they are and are not moved.
   After a save the editor says "Saved. The backup is in" and the folder.
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
