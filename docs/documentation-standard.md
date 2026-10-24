# The documentation standard

> **This file _is_ the standard.** Every other document in this repository is held to
> it, and each one links back here. Changing the rules here changes what the rest of the
> documentation has to meet.

Every document in this repository is held to this. It is KMRP's documentation standard,
where this editor lived until 2026-10-24, with its rules about executable bytes turned
into rules about save files. The benchmark here is
[`save-editor-format.md`](save-editor-format.md), section 3.4 (Health and Force: what is
stored, what the game recomputes), and the field tables of
[`save-editor.md`](save-editor.md). Match those or improve on them.

The test a document has to pass: **someone who was not here, working only from this
repository, a copy of the game and some saves, can check every claim and redo the work.**

## Two kinds of document, kept apart

| kind | holds | example |
| --- | --- | --- |
| **Lab record** | What was tried, in the order it happened. Failures, dead ends, rejected ideas, disproved theories. Never edited to look tidy. | `save-editor.md`, section 8; the quest review pages in `data/quests/*-review.md` |
| **Reference** | The finished mechanism. What the code does now, field by field. | `save-editor.md`, sections 1 to 7; `save-editor-format.md` |

Each says at the top which it is, and links to the other.

## The rules

**1. Every claim is measured, and says how.** "Read from the GalaxyMap save", "read from
the game's `feat.2da`", "from the maintainer's own private reverse engineering",
"checked by `tests/levels.rs`". If a fact came from community documentation or another
editor's readme rather than from a save, the game's files or the engine, label it as such:
it may be wrong. `save-editor-format.md` tags each fact `[file]`, `[RE]`, `[game]`,
`[doc]` or UNCONFIRMED.

> This is not hypothetical. The format notes once said a party member's current Health is
> stored relative to the maximum. It is stored relative to level times the Constitution
> modifier; the earlier sentence was written by analogy with the player, not measured.

**2. State the build and the game.** Which editor build, which game (KotOR 1 or KotOR 2),
which release (Steam, GOG, Aspyr's ports, a language), and which mods (K1CP, TSLRCM). A
statement about saves means little without saying whose saves.

**3. Say where an engine address comes from.** An address in the game's executable names
the executable (KotOR 1 1.03, Steam `swkotor2.exe` 1.0.2) and the source: the public
decompilation, a disassembly made for the document, or the maintainer's own private
reverse engineering. The private sources are never named in any other way.

**4. Tables for data, prose for reasoning.** Every field gets a row: file, field, type,
what the game does with it, what the editor writes, the rule. Never bury a field name in
a sentence.

**5. Quote the evidence, trimmed.** Only the part that carries the argument: the few
fields of a GFF dump, the lines of a decompiled routine, a test's assertion.

**6. Give the formula, then a table of what it produces.** "Kill XP is the `xptable.2da`
row for the player's level, column trunc(CR)" is the rule; the XP it gives in two real
saves is the evidence that the rule is what the game does.

**7. Corroborate where you can, and name the source.** A second save, the other game, the
public decompilation, a community format document. Mark it as corroboration, not as
primary evidence.

**8. Close the loop.** The strongest check is one where the editor's rule reproduces what
the game itself wrote: `computed_maxima_match_what_the_game_wrote` compares the editor's
Health and Force maxima with every value the game stored. Find the check that makes a
claim falsifiable, and run it.

**9. Record what was rejected, and why.** The idea that was tried and failed is as useful
as the one that shipped: it stops the next person trying it again. Say what was appealing
about it and what killed it.

**10. Mark untested explicitly.** "Read back by the tests in all 23 saves on the
maintainer's PC; not yet loaded in the game." Never imply coverage you do not have. A
reader who finds one overstated claim has to doubt all of them. Seen in the editor, read
back by a test, and loaded in the game are three different claims.

**11. Corrections stay visible.** When a document was wrong, say what it said, what is
true, and why the mistake was made. Do not edit it silently. The mistakes are the reason
the rules exist.

**12. Say what is deliberately not changed.** A reader needs to tell "left alone on
purpose, and why" from "never looked at". The fields the editor leaves alone are often
the most important facts in the document.

**13. State the count, and how you established it.** "Every copy of the creature the save
holds: the player in `Mod_PlayerList`, in KotOR 2 also `pc.utc`, a party member in
`AVAILNPCn.utc` and in the area" is a count. If you believe there is one copy, say why.

**14. End with how to check by hand.** The steps, the files and the fields, and the checks
that catch the mistakes that part of the save invites. If a reader cannot redo your
checks without your machine, the document has failed.

## Writing

- Plain language and short sentences. No marketing words.
- No em dashes, and no `--` used as a dash.
- Dates as YYYY-MM-DD.
- Never the games' own text beyond a short name or label needed to say which thing is
  meant. Quest tags, resrefs and stage numbers are fine; journal text is not.

## Skeleton

```markdown
# <Part of the save or the editor>: what the editor reads and writes

<One paragraph: what this covers, what it does not, and which document has the rest.>
<A sentence on provenance: measured how, and where untested is marked.>

## The build and the games this describes
## 1. What the game does with it          <- evidence, corroboration, a check that closes the loop
## 2. The rules                           <- formulas, then a table of what they give
## 3. Every field the editor writes       <- tables: file, field, rule
## 4. How a write stays safe              <- backups, refusals, what is shared with other edits
## 5. Limits
## 6. What is deliberately not changed
## 7. Checking by hand
```

Not every document needs all of it. Every document needs 1, 3, 6 and 7.

## Anti-patterns, all of them real

| Anti-pattern | What happened |
| --- | --- |
| Restating another source as measurement | The party member's Health rule, written by analogy with the player's (rule 1). |
| Editing a field the game ignores | Other editors changed `MaxHitPoints`, which the games recompute on load; the edit was thrown away. |
| One copy assumed to be the only copy | An edit to a party member also reached an unrelated area creature with the same tag (the Endar Spire's Carth) until 2026-10-23. |
| A rule stated without its game | KotOR 2's Force maximum is computed from Wisdom alone; KotOR 1's is not. |
| Silent correction | Fixing a wrong sentence without saying it was wrong, which loses the record of why the rule exists. |
| Implying coverage | "Tested" when the tests ran and the save was never loaded in the game. |

## Before you commit a document

- [ ] Every claim traced to a save, the game's files, the engine, or a labelled outside source
- [ ] Build, game and release stated
- [ ] Fields in a table, with what the game does with each
- [ ] Formula plus the table it produces
- [ ] Anything untested labelled untested
- [ ] Rejected alternatives recorded with their reasoning
- [ ] Corrections visible, not silent
- [ ] "Not changed" section present
- [ ] No game text beyond short names; private sources cited only as the maintainer's own private reverse engineering
- [ ] A reader with the game and a save can check it without asking you
- [ ] `python .github/scripts/check_links.py` passes
