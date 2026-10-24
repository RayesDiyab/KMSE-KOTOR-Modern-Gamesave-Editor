## What this changes

<!-- One subject per pull request. What does the editor do differently afterwards? -->

## Why

<!-- The cause, not only the symptom: the save fields involved, what the game does with
     them, and where that was measured. -->

## How it was verified

<!-- What you MEASURED, and how: a test, a value read back from a written save, a save
     loaded in the game. Include the numbers and say which game. -->

## Not verified

<!-- Say plainly what you could not check: the other game, a platform not built, a save
     not loaded in the game. An honest gap is far more useful than implied coverage. -->

## Checklist

- [ ] No game files or game text are committed (archives, textures, movies, `dialog.tlk` text, executables); saves only as test copies if at all
- [ ] Every write still goes through the backup, and the Xbox, game-running and no-game refusals are unchanged
- [ ] Tests write only to copies of saves
- [ ] Interface changes: a mockup made from a real screenshot was approved first
- [ ] `build.ps1` (or `cargo test --profile tests`) passes
- [ ] Affected documentation in `docs/` updated, including any earlier statement this proves wrong
- [ ] `CHANGELOG.md` entry added under `## [Unreleased]`
- [ ] `python .github/scripts/check_links.py` passes
