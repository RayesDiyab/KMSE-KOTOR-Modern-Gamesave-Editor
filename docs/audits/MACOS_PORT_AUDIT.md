# KMSE macOS port audit, 2026-11-08

> A **lab record**, held to the [documentation standard](../documentation-standard.md). It says
> what was changed for macOS, what was found, what was fixed and how each claim was checked.
> The Windows reference is [WINDOWS_FULL_AUDIT.md](WINDOWS_FULL_AUDIT.md). What the editor
> writes, field by field, stays in [save-editor.md](../save-editor.md).

## A. Environment

| | |
| --- | --- |
| Audited | branch `master`, commit `47a2433` (clean clone), plus the changes below (uncommitted when the tests ran) |
| Machine | Apple M5, built-in Liquid Retina XDR display, 3024 x 1964 (Retina, scale 2) |
| macOS | 27.0.1 (build 26A434); startup volume APFS, case-insensitive |
| Rust | rustc 1.98.1, cargo 1.98.1; targets aarch64-apple-darwin, x86_64-apple-darwin; x86_64-pc-windows-msvc and x86_64-unknown-linux-gnu added for `cargo check` |
| Apple tools | Command Line Tools 27.0 (SDK 27.0), no Xcode; codesign, lipo, iconutil, sips, hdiutil, plutil, notarytool 1.1.3; Rosetta 2 installed |
| Other | ffmpeg 9.0.2 (Homebrew), for the timeline's Yavin Station |
| Signing | no Developer ID identity in the keychain (`security find-identity -p codesigning -v`: 0) |

**Games on this Mac** (every path read on disk, 2026-11-08):

| Distribution | Bundle | Game files | Saves |
| --- | --- | --- | --- |
| KotOR Steam (Aspyr 1.4.0, with KMRP) | `~/Library/Application Support/Steam/steamapps/common/swkotor/Knights of the Old Republic.app`, id `com.aspyr.kotor.steam` | `Contents/Assets` | `~/Library/Application Support/Knights of the Old Republic/saves` (14 folders, Steam Cloud) |
| KotOR Mac App Store (Aspyr 1.4.1) | `/Applications/Knights of the Old Republic.app`, id `com.aspyr.kotor` | `Contents/Assets` | its container `~/Library/Containers/com.aspyr.kotor`, closed to other programs (see F) |
| KotOR II Steam (Aspyr) | `steamapps/common/Knights of the Old Republic II/KOTOR2.app`, id `com.aspyr.kotor2` | `Contents/GameData` | `cloudsaves/73340463/` beside `KOTOR2.app` (6 saves, made on the maintainer's Windows PC, brought by Steam Cloud during this audit) |
| KotOR II Mac App Store (Aspyr 1.0.2, 2015) | `/Applications/KOTOR2.app`, id `com.aspyr.kotor2.appstore` | `Contents/GameData`, its `dialog.tlk` in `Localized/<Language>/` | container present, no saves in it |

Steam started at 12:52:01 during the audit and brought down KotOR saves 000934 to 000937
from the Windows PC. Some arrived without `SAVEGAME.sav` (Steam's `remotecache.vdf` lists
them). They are not the editor's doing: every file that was there before the tests was hashed
(SHA-256) first and matched afterwards, and no test or run wrote beside the real saves.

## B. Architecture

The game logic (formats, save model, quests, story moves, map) is shared and unchanged
apart from two cross-platform fixes (C, S-1 and S-2). macOS behaviour sits in adapters:

| Adapter | Where | macOS |
| --- | --- | --- |
| Install discovery | `games::find_installs`, `resource_root`, `has_game_files`, `identify`, `bundle_game`, `mac_bundles`, `spotlight_bundles` | Steam libraries as on every system; then `/Applications`, `~/Applications` and Spotlight (`mdfind` by bundle id, once per run, 2 s at most). An app bundle, or a folder holding one, stands for `Contents/Assets`, `Contents/GameData` or `Contents/KOTOR Data` |
| Save discovery | `games::save_roots`, `port_save_roots`, `mac_containers`, `blocked_save_folders` | Application Support, the App Store containers, `saves` and `cloudsaves` beside the bundle; a folder macOS refuses is named in a pop-up |
| Game running | `games::game_running`, `running_games`, `game_of_process`, `game_in_args` | libproc (`proc_listallpids`, `proc_pidpath`, `proc_name`) and `sysctl(KERN_PROCARGS2)` for Wine; fails safe |
| Pickers | `src/macos.rs` `pick_game_or_folder` | an `NSOpenPanel` that takes an app or a folder (rfd's folder picker cannot select an app) |
| Quit | `src/macos.rs` `route_quit_through_close` | Quit goes through the window's close check |
| Settings | `games::data_dir` | `~/Library/Application Support/KOTOR Modern Gamesave Editor/settings.txt`, unchanged |
| Durability | `save::sync_dir`, `sync_tree` | folder and backup flushes after renames (`F_FULLFSYNC` through Rust's `sync_all`) |
| Packaging | `build-macos.sh` | Universal 2 `dist/KMSE.app`, DMG, ZIP |

Windows keeps its own code paths: the registry and VirtualStore lookups, `tasklist`
(moved to `games.rs` unchanged), Explorer, the DPI fit. `cfg(windows)` and
`cfg(target_os = "macos")` are used in `games.rs`, `save.rs` (Unix flushes) and `main.rs`
(the picker and Quit) only.

## C. Implementation changes

| ID | Sev | What was wrong on macOS | Change | Test |
| --- | --- | --- | --- | --- |
| M-1 | P1 | No install found: the code looked for `KOTOR.app/Contents/KOTOR Data`, and `is_install` needed a Windows program | `resource_root`, bundle and Spotlight discovery; `is_install` checks the files (a KEY, a TLK, `modules`) and the game | `macos_port.rs` `an_app_bundle_stands_for_the_game_files_inside_it`, `the_installs_on_this_mac_are_found` (all four found) |
| M-2 | P1 | `identify` took the App Store KotOR II (chitin.key 406,035 bytes) for KotOR by the old size rule | by bundle id, then by the first module (`end_m01aa`, `001ebo`); no guess | `the_game_is_told_by_its_files_without_a_bundle` |
| M-3 | P1 | The game check looked for `pgrep -x KOTOR`; the ports run `AppBundleExe`, `KOTOR_Exe`, `KOTOR2`, `KOTOR2sub` | libproc listing, bundle id; Wine by its arguments; a listing that fails, a launcher whose bundle cannot be read or a Wine program whose arguments cannot be read counts as running | `a_running_port_is_seen`, `a_running_wine_game_is_seen` (real processes, ad hoc signed copies of `/bin/sleep` and `/bin/bash`), `the_ports_programs_are_told_apart` |
| M-4 | P2 | Finder's `.DS_Store` (and `._*` on other disks) made after opening refused the next write as "changed on disk"; the editor's own Show in Folder can cause it | `save::is_system_litter`: left out of the on-disk check, the backup check, copies and revert | `what_finder_leaves_in_a_save_folder_is_not_the_save` (fails before: the write is refused) |
| M-5 | P2 | Quit (Command-Q, the Dock) ended the program at once in winit: the "changes not saved" check never ran, and neither did the tidying after the window's loop | `applicationShouldTerminate:` added to winit's delegate: it closes the window as its close button does | in the app: a Quit Apple event closed the window, `settings.txt` was written by the code after `run()` (it is not reached when the program is ended), the program exited. The unsaved branch was not clicked (G) |
| M-6 | P2 | KotOR II Steam's `cloudsaves` beside `KOTOR2.app` were never listed | `save_roots` also looks beside the bundle | `saves_beside_the_bundle_are_listed`; `real_saves.rs` read all 6 |
| M-7 | P2 | The App Store KotOR II has no `dialog.tlk` beside `chitin.key` | `games::tlk_path`, `localized_dir`: English (the maintainer's choice, 2026-11-08: "when KMSE can't tell which language is installed, make English the default"); its `Override` is read after the main one | `a_game_with_a_text_per_language_is_read_in_english` |
| M-8 | P2 | A save folder macOS refuses (another app's container) was skipped with no word | `blocked_save_folders` and a pop-up once per folder per run | `macos_blocked.rs` (a closed folder in a made-up home) |
| M-9 | P3 | The Workshop folder was looked for two folders up, inside the bundle on macOS | the `steamapps` folder above the install | read in the code |
| M-10 | P3 | `chitin.key` was opened by its lower-case name only | case-insensitive | `chitin_key_is_found_in_any_case` |
| M-11 | P3 | A backup could go inside an app bundle if a save were in one | beside the bundle | `backups_never_go_inside_an_app_bundle` |
| M-12 | P3 | KotOR II's Override subfolders were taken in the order the disk lists them: by name on NTFS, in no set order on APFS | sorted as NTFS lists them | read in the code; NTFS results unchanged |
| M-13 | P3 | Folders and backups were not flushed after renames | `sync_dir`, `sync_tree` (Unix only) | the write and restore tests |
| M-14 | P3 | `build.rs` found no movie for Yavin Station on a Mac | the Mac bundles added; the frame lines up with the committed outline (looked at, 2026-11-08) | the build |

**Shared-logic defects found because the Mac's saves differ** (P3, both games):

| ID | What | Fix | Test |
| --- | --- | --- | --- |
| S-1 | Go Back changed the party table (`PT_IS_LEADER`) before refusing a save with a damaged level history. The window works on a copy, so no save took it | `release_controlled_npc` moved after the last refusal; a successful Go Back is the same | `story_safety.rs` `inconsistent_history_is_rejected_before_any_rewind_change`, which failed before on the Mac's KMRP test save |
| S-2 | none in the program: a test took the first entry of a backups folder, the `.source` record on APFS | the test picks the folder | `edits.rs` |

**Tests whose assumptions did not hold here** (fixture or environment, not defects): saves
Steam Cloud had not finished bringing down were taken as fixtures and failed to open (the
selectors now take saves that open, which on the Windows PC is every save); the port save
folders hold real saves on a Mac, so two made-up listings now filter to their own folder;
two tests took a hand-made KMRP save (no `GLOBALVARS.res`, no story played) as "after Taris"
and now skip it.

**Other files:** `build-macos.sh` (new), `build.sh` (runs it on macOS), the GitHub workflow
(builds the macOS app and keeps the DMG and ZIP), `Cargo.toml` (objc2, objc2-foundation and
objc2-app-kit for macOS only, the versions rfd already used; `Cargo.lock` gains no crate).

## D. Feature matrix

"Auto" is the automated tests on this Mac with the real installs and saves; "GUI" is the
program run from its bundle; "Game" is a save loaded in the game.

| Feature | Windows | macOS | Auto | GUI | Game | Limits |
| --- | --- | --- | --- | --- | --- | --- |
| Game and save discovery | Steam, GOG, discs, VirtualStore | Steam, App Store, Spotlight, containers, beside the bundle | passed | passed: 13 KotOR and 12 KotOR II saves listed | n/a | App Store saves blocked by macOS (F) |
| Save list, thumbnails, metadata, party portraits | yes | same code | passed (`real_saves`, `save_picture`) | passed (seen, both games) | n/a | |
| Game switch, theme, text size, settings persistence | yes | same code; settings in Application Support | n/a | passed: K2 light large and K1 dark small set through `settings.txt` and shown; written at Quit | n/a | switched through the file, not by clicking |
| Open, Save Changes, Save As, Copy, Revert, History, restore | yes | same code plus M-4, M-13 | passed (`backup_on_open`, `save_as`, `restore`, `write_faults`, `unicode_paths_round_trip`, `save_picture`) | **not run** | **not run** | |
| Character, abilities, inventory, party, companions from another save | yes | same code | passed (`edits`, `levels`, `abilities`, `level_grants`, `audit_*`) with both games' files | **not run** | **not run** | |
| Timeline, Skip Ahead, Go Back, Finish Quest, quests | yes | same code plus S-1 | passed (`story_*`, `finish_quest*`, `galaxy_route`, `star_maps`) | **not run** | **not run** | |
| Map, teleport, spawn | yes | same code | passed (`exits`) | **not run** | **not run** | |
| Advanced (globals, objects, creatures) | yes | same code | passed (`audit_fixes`) | **not run** | **not run** | |
| Game folder and save folder pickers | rfd | `NSOpenPanel` taking an app or a folder | n/a | **not run** | n/a | |
| Running-game refusal | tasklist | libproc, bundles, Wine arguments | passed (real processes) | n/a | **not run with a real game** | |
| Quit with unsaved changes | close check | through the close check (M-5) | n/a | Quit path seen; unsaved branch not clicked | n/a | a log out with changes is interrupted |
| Window scaling | U-6 fit | same code | n/a | passed at 3024 x 1964, scale 2 | n/a | not moved between displays |

## E. Save integrity on macOS

- **The write path is unchanged:** each new file is staged as `.kmrp-new` and flushed. Each
  old file goes aside as `.kmrp-old`, the new one goes in, and on a failure every old file
  goes back. New on Unix: the folder is flushed after the renames, and the backup is flushed
  before the save is written over.
- **Atomicity:** `rename` replaces one file atomically on APFS, as on NTFS. A write of
  several files is not atomic. A cut between two renames leaves `.kmrp-new` or `.kmrp-old`
  files, which the next open refuses and names. This is the same as on Windows.
- **What macOS does not do:** a file the game holds open can still be renamed over. No
  sharing error stops the write. The game check (M-3) carries all of it, and it refuses to
  write when it cannot tell.
- **Copies on APFS** are clones (`fs::copy` uses `clonefile`). Every backup is read back and
  compared byte for byte, as before.
- **Not changed:** the backup on opening, the check before each write, the Xbox and
  outside-the-archive refusals, the own-backup check in revert, and the link refusal (also
  tested on Unix, `a_link_in_a_save_folder_stops_the_backup`).
- **Backups:** KotOR's Steam saves keep them in
  `~/Library/Application Support/Knights of the Old Republic/Save Editor Backup`. KotOR II's
  Steam Cloud saves keep them in `.../Knights of the Old Republic II/Save Editor Backup`,
  outside `cloudsaves` and outside the bundle.

## F. Game distribution compatibility

| Distribution | Found | Files read (tests) | Saves | In game |
| --- | --- | --- | --- | --- |
| KotOR Steam | yes | yes: every KotOR test ran against it | 10 that open (4 incomplete from Steam Cloud, refused as they should be) | not run |
| KotOR Mac App Store | yes; its files read in the GUI runs | through the GUI only | **blocked**: its container is closed to other programs (`Operation not permitted` from this shell); the pop-up of M-8 tells the player; whether macOS then asks the player was not seen | not run |
| KotOR II Steam | yes | yes: every KotOR II test ran against it | 6 Steam Cloud saves (from Windows), all open and round-trip | not run |
| KotOR II Mac App Store | yes | layout tested (M-7); the GUI read its portraits | none on this Mac; the save folder inside its container is unconfirmed | not run |
| GOG for Mac, Wine, CrossOver, Whisky | not installed | process check tested with stand-ins only | n/a | n/a |

Windows saves opened on the Mac: all six KotOR II saves and KotOR saves 000936 and 000937
were made on the Windows PC. They open, back up, write and revert on copies here.

## G. Tests

| Run | Command | Result |
| --- | --- | --- |
| Baseline at `47a2433` | `CARGO_TARGET_DIR=build/target-tests cargo test --profile tests --no-fail-fast` | 142 passed, 4 failed. No install was found, so most game tests passed without checking |
| Installs found, before the test fixes | same | 144 passed, 15 failed: 13 took an incomplete Steam Cloud save as their fixture (one of them, `edits_survive_a_write`, then failed on the order assumption S-2), 1 took a hand-made save, 1 found a defect (S-1) |
| Final | `sh build-macos.sh` (runs the same tests first) | **164 passed, 0 failed**; with the save pictures (R-3) **168 passed, 0 failed**; with the list of copies (R-2) **170 passed, 0 failed** |
| Windows type check | `cargo check --target x86_64-pc-windows-msvc --all-targets` | passed, no warnings. Not run on Windows: CI runs the Windows tests |
| Linux type check | `cargo check --target x86_64-unknown-linux-gnu --all-targets` | **blocked**: needs Linux's fontconfig. CI covers it |
| Independent review | a second reviewer, read only, given the code and the layouts but not these conclusions | no P0. Its P1 (Wine) and P2s (cloudsaves, language, a lookup that fails counting as not running) are fixed with tests. Its P3s are fixed: `otool` for both halves, Spotlight cached, one name per closed container |
| App, relocated | `open -n --env HOME=<disposable> "<scratch>/Other Place ü/KMSE.app"` | launched, rendered, listed saves |
| App, DMG to /Applications | mounted the DMG, copied the app, `open` | signature valid; launched; KotOR II light theme shown |
| App, Intel half | `arch -x86_64` on the installed app | ran as "X86-64 (translated)" under Rosetta, rendered. Not tested on an Intel Mac |
| Quit | `osascript ... to quit` | window closed through the check, `settings.txt` written, exited |
| Real saves | SHA-256 of every file before and after | unchanged; no backup folder made beside them |

**Not run:** clicking through the window. The session had no desktop-control tool, so no save
was opened, edited, saved, reverted or restored through the window. The pickers were not
opened, and the unsaved-changes Quit was not clicked. No edited save was loaded in either
game.

## H. Remaining issues

| ID | Sev | What | Why left |
| --- | --- | --- | --- |
| R-1 | P2 | The GUI pass of D, and saves loaded in the game | needs a person at the Mac, or desktop control |
| R-2 | P2 | KotOR App Store saves sit in a container macOS closes to other programs. macOS showed no prompt of its own on this Mac (seen 2026-11-08). The editor now lists every copy with its state and offers Choose the Folder and Open Settings (Full Disk Access, the page checked to open); `game_copies` tested (`macos_port.rs`). Whether a folder chosen in the panel lets KMSE in, and for how long, was not seen | needs the player's click |
| R-3 | P3 | ~~Saves made from `000900 - KMRPTest` have no `Screen.tga`, so they show an empty picture in the editor and the game. The editor's Copy, Save As and Save Changes keep a picture byte for byte (`save_picture.rs`). These saves never had one~~ | **fixed** (2026-11-08, the maintainer chose it): a save without a picture gets its module's load screen when the editor writes it ([save-editor.md, section 3](../save-editor.md#3-every-value-the-editor-writes)); `save_picture.rs`, 5 tests. Not yet seen on the game's Load screen or clicked in the window |
| R-4 | P3 | A log out while changes are not saved is interrupted by the close check | the same check as Command-Q; chosen over losing changes |
| R-5 | P3 | `backup_root` folds case on macOS, so two folders differing only in case on a case-sensitive volume share a backups root | as on Windows; no such install seen |
| R-6 | P3 | Settings and `spawned.txt` written by two copies of the editor at once | as on Windows |
| R-7 | P3 | Wine, CrossOver and Whisky detection was tested with stand-ins, not a real bottle | none installed |

## I. Distribution

- `sh build-macos.sh` makes `dist/KMSE.app` (Universal 2: arm64 and x86_64, macOS 11 or
  later), `dist/KMSE-1.0.0-macOS.dmg` (the app and a link to Applications) and
  `dist/KMSE-1.0.0-macOS.zip`, and prints each SHA-256.
- Bundle: id `com.raymangt.kotor-modern-gamesave-editor` (the one `build.sh` already used),
  version 1.0.0, `AppIcon.icns` from `assets/icon/icon.png`. Fonts and pictures are compiled
  into the program.
- The script checks that only system libraries are loaded (both halves) and that no path
  under the builder's home is in the program (`--remap-path-prefix`).
- **Signing:** ad hoc. Gatekeeper does not accept the app (`spctl`). Before release it must
  be signed with a Developer ID (`KMSE_SIGN_IDENTITY`), notarized (`KMSE_NOTARY_PROFILE`)
  and stapled. The script does this when both are set. **Not run:** no identity on this Mac.
- **Reproducible:** yes for the program (two builds, one SHA-256; see the end of J).
- Nothing was uploaded or published.

## J. Release readiness

**NOT READY: BLOCKING ISSUES REMAIN.**

The blocking issues are verification not yet done. There is no known defect:
- no P0 or P1 is open. Every P1 and P2 found is fixed and tested;
- 170 tests pass on this Mac against real installs and saves of both games;
- the app builds, signs ad hoc, installs from its DMG, runs from any folder and in both
  architectures, and shows both games' saves.

What stops a limited beta is R-1: a short pass through the window on the Mac. Open a save of
each game, edit it, Save Changes, Revert, restore from History, pick a game app in Settings,
and press Command-Q with a change pending. Then load one edited save in each game. Once that
passes, the port is ready for a limited macOS beta. A release candidate also needs Developer
ID signing and notarization.

### Reproducible build

The release program was built twice from the same sources, the second time after deleting
both architectures' release build folders. The two builds (of commit `7ff2412`, before the
save pictures) were byte for byte the same:
`dist/KMSE.app/Contents/MacOS/KMSE`, 29,886,384 bytes, SHA-256
`dd22b752c9f5873da785770724dd17e6b918d781c653882d5381b9f0e0f5ee07`. The DMG and ZIP carry
time stamps, so their hashes differ between builds. The program built here contains the
Yavin Station frame from this Mac's KotOR, so a build on a Mac without KotOR or ffmpeg
gives a different program (as on Windows).
