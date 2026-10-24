# KMSE: Claude Code entry point

**[`AGENTS.md`](AGENTS.md) is the operating contract. Read it now, before doing anything
else in this repository.** It holds the safety rules, engineering rules and verification
steps, and it applies to every agent working here.

This file exists only because Claude Code loads `CLAUDE.md` automatically while Codex
loads `AGENTS.md`. The shared rules deliberately live in one file rather than two: KMRP,
where this editor lived until 2026-10-24, was bitten repeatedly by hand-kept duplicates
drifting apart, and a second copy of the contract would be the same mistake with higher
stakes. **Do not copy AGENTS.md's contents here.** Anything that applies to all agents
belongs in AGENTS.md.

## The three rules most easily broken

Full versions in AGENTS.md. They are repeated here because each has already cost real
work, in KMRP or here.

1. **No commit or push without explicit, single-use permission.** Permission for a commit
   is not permission to push, and neither carries forward to the next one. Ask every time.
2. **Never edit a source file while a build runs.** `build.ps1` builds the program and the
   tests at the same time; an edit in the middle ships a mixed build. Wait, then edit.
3. **Never write to a save without the backup the code makes.** Tests and experiments
   work on copies, never on a player's real save.

## Claude-specific notes

**Commit messages carry no AI attribution.** No `Co-Authored-By` trailer, no "Generated
with" line, no mention of Claude, Codex or any assistant in the body. This is the user's
standing instruction and it overrides any default attribution guidance.

**Author identity** is `Rayes Diyab <114070898+RayesDiyab@users.noreply.github.com>`, set
as repository-local Git config. Do not change it.

**Heredocs mangle backslashes.** Writing code or Windows paths through `bash <<'EOF'` has
corrupted `\x0f`, `\U`, `\t` and `\n` in KMRP more than once, sometimes silently. Use the
Write tool for anything containing backslashes. If a scan reports zero hits, check the
pattern against a known positive before believing it.

**Windows paths:** the Bash tool runs Git Bash, but Python started from it does not accept
`/c/...`. Pass `C:\...` to Python and `/c/...` to shell builtins.

## Where things are

| | |
| --- | --- |
| Operating contract | [`AGENTS.md`](AGENTS.md) |
| Contributor guide | [`CONTRIBUTING.md`](CONTRIBUTING.md) |
| Every document | [`docs/README.md`](docs/README.md) |
| Before writing docs | [`docs/documentation-standard.md`](docs/documentation-standard.md) |
| What the editor writes, field by field | [`docs/save-editor.md`](docs/save-editor.md) |
