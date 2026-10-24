#!/usr/bin/env python3
"""Fail if a Markdown link to a file inside this repository does not resolve.

Only relative links are checked. External URLs are left alone, because a link
checker that reaches the network turns an unrelated outage into a red build.
Anchors and query strings are stripped before the path is tested.

Adapted from KMRP's .github/scripts/check_links.py (2026-10-24): the same rule,
with this repository's build folders skipped.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path
from urllib.parse import unquote

ROOT = Path(__file__).resolve().parents[2]
# build/ holds Cargo's output and the quest database cache; dist/ the built program;
# data/quests/local/ the review pages with the game's own text, never committed.
SKIP_DIRS = {".git", "build", "dist", "logs", "tmp", "__pycache__", "local"}
LINK = re.compile(r"\[[^\]]*\]\(([^)]+)\)")


def main() -> int:
    broken: list[str] = []
    checked = 0
    for path in sorted(ROOT.rglob("*.md")):
        if any(part in SKIP_DIRS for part in path.relative_to(ROOT).parts):
            continue
        for target in LINK.findall(path.read_text(encoding="utf-8", errors="replace")):
            target = target.strip()
            if target.startswith(("http://", "https://", "mailto:", "#")):
                continue
            target = unquote(target.split("#", 1)[0].split("?", 1)[0])
            if not target:
                continue
            checked += 1
            if not (path.parent / target).exists():
                broken.append(f"{path.relative_to(ROOT)} -> {target}")
    print(f"checked {checked} relative links")
    for item in broken:
        print(f"  BROKEN  {item}")
    return 1 if broken else 0


if __name__ == "__main__":
    sys.exit(main())
