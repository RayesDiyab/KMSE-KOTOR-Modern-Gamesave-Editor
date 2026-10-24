# Documentation

> **Documentation standard.** This document follows
> [`documentation-standard.md`](documentation-standard.md). Read it before editing this
> file, and check the result still meets it: measured claims only, rejected alternatives
> and corrections kept visible, and anything untested labelled as untested.

Every document in the KOTOR Modern Gamesave Editor (KMSE) repository.

## Where to start

- **Using the editor?** The [main README](../README.md) says what it does and how to get
  it.
- **Want to know exactly what an edit changes?** [save-editor.md](save-editor.md), section
  3, lists every value the editor writes, with the file, the field and the rule.
- **Working on the save format?** [save-editor-format.md](save-editor-format.md), with
  where each fact was measured.
- **Starting an agent task?** Read the root [`AGENTS.md`](../AGENTS.md) first.

## The editor and the save format

| Document | What it covers |
| --- | --- |
| [save-editor.md](save-editor.md) | **What the editor reads and writes, field by field:** where saves are found, every value written and its rule, the map, travel and Skip Ahead, the timeline, how a write stays safe, limits, what is deliberately not changed, and what was tested and how. |
| [save-editor-format.md](save-editor-format.md) | **The KotOR 1 and KotOR 2 PC save format:** every file, struct and field, what the games read and recompute when a save loads, and where saves live on each platform, each fact tagged with its source. |
| [documentation-standard.md](documentation-standard.md) | **The standard every document here is held to.** Read before writing or updating one. |

## Beside the code

| Document | What it covers |
| --- | --- |
| [../CHANGELOG.md](../CHANGELOG.md) | What changed, with dates. Changes before 2026-10-24 are in KMRP's CHANGELOG. |
| [../DESIGN.md](../DESIGN.md) | The design system: colours, type, layout and the rules behind them. |
| [../PRODUCT.md](../PRODUCT.md) | Who the editor is for, what it must do, and the maintainer's decisions on its shape. |
| [../assets/icon/README.md](../assets/icon/README.md) | The program's icon, a holocron with a datapad, and how to remake it. |
| [../assets/timeline/README.md](../assets/timeline/README.md) | The timeline's pictures and where each comes from. |
| [../data/quests/k1-overview.md](../data/quests/k1-overview.md) | KotOR 1's quest databases, chapter by chapter, and what in them is uncertain. KotOR 2 has no overview page yet. Each chapter of both games has a review page (`data/quests/*-review.md`), identifiers only. |

## Working on the project

| Document | What it covers |
| --- | --- |
| [../AGENTS.md](../AGENTS.md) | The operating contract for coding agents. [../CLAUDE.md](../CLAUDE.md) points to it. |
| [../CONTRIBUTING.md](../CONTRIBUTING.md) | How to build, test and send a change. |
| [../SECURITY.md](../SECURITY.md) | How to report a vulnerability or a bug that damages saves, and what the editor writes where. |
| [../CODE_OF_CONDUCT.md](../CODE_OF_CONDUCT.md) | The code of conduct. |
| [../THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md) | Everything here that is not the maintainer's own work, with its licence, and the position on the games' material. |
