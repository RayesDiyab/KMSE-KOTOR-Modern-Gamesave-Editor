# KMSE 1.0 press kit

Release art for KMSE 1.0, the KOTOR Modern Gamesave Editor: a cover and one page for each part
of the editor, for mod pages, posts and articles. Use them as they are; please do not crop a
window out of its page without saying what it shows.

Thirteen pages, in the order to show them, each 1920x1080.

| File | What |
| --- | --- |
| [`01-cover.png`](01-cover.png) | Cover: the holocron, KMSE 1.0, the editor's parts, KotOR and KotOR II, Windows, macOS and Linux |
| [`02-load-screen.png`](02-load-screen.png) | The save list, shaped like the game's Load screen |
| [`03-character.png`](03-character.png) | Character: name, experience, classes, alignment, attributes, Health and Force, look and voice |
| [`04-abilities.png`](04-abilities.png) | Abilities: skills, feats and Force powers |
| [`05-inventory.png`](05-inventory.png) | Inventory: credits, items, stack sizes, what the player wears |
| [`06-party.png`](06-party.png) | Party: recruited, with you, can be picked, party experience |
| [`07-timeline.png`](07-timeline.png) | Timeline: the story as a galaxy map |
| [`08-quests.png`](08-quests.png) | Quests: the quest tracker and journal entries |
| [`09-pazaak-destinations.png`](09-pazaak-destinations.png) | Pazaak & Destinations: where the Ebon Hawk can fly, and the side deck |
| [`10-map.png`](10-map.png) | Map: the area map with its people, doors and containers |
| [`11-advanced.png`](11-advanced.png) | Advanced: story values, the save's name and time played |
| [`12-kotor-2.png`](12-kotor-2.png) | KotOR II: its save list in its own colours, with Steam Cloud saves |
| [`13-safe.png`](13-safe.png) | Your saves are safe: Save Changes, the backup, the game closed, well-formed changes |

## How they were made

- **The pages** are drawn by [`tools/press_kit/build.py`](../../../tools/press_kit/build.py):
  `python tools/press_kit/build.py releases/v1.0/screenshots` (or `--only 07` for one page).
  It needs Pillow and numpy, and macOS's Georgia for the silver KMSE.
- **The look is KMSE's own**, settled by the maintainer on 2026-11-13 ("make a KMSE inspired
  kit", not KMRP's release art): the editor's dark KotOR colours from `ui/theme.slint`, the
  timeline's stars (`assets/timeline/stars.png`), the game-menu frame with bright corner
  brackets that the DMG window has, KMSE in the crest's silver with the version in holo cyan,
  and the KMRP Original HD fonts the editor uses (`fonts/`). The KotOR II page uses the
  editor's KotOR II teal.
- **The holocron on the cover** is `assets/icon/art.png`, the icon's art without its letters.
- **Nothing inside a window is retouched or cropped:** each one is the whole window, scaled
  down, with its size in points under it.
- **The screenshots** are in [`../screenshots`](../screenshots), 3024x1736 pixels (1512x868
  points, Retina). They were taken on 2026-11-13 on macOS 27.0.1 from `dist/KMSE.app` built
  from `f6b4922` (KMSE 1.0.0), each window captured alone with `screencapture -l`. The KotOR
  pages show the maintainer's Steam KotOR save "Trial succcess" (Manaan, 15h 56m), opened and
  not changed ("No changes" on every page); the KotOR II page shows the maintainer's Steam
  KotOR II saves. The editor's settings were the maintainer's own (dark, Medium text) and
  were left as they were.
- **The words** come from the README's "What it does" and "Your saves are safe" lists. The
  count of story values on page 11 is the editor's own line for that save.

## Not shown

- **Windows and Linux.** Every window is the macOS build; the editor looks the same on the
  other two, but no page shows them.
- **The light theme and the text sizes** in Settings.
- **The editor writing a save.** The pages show a save opened and left unchanged.
