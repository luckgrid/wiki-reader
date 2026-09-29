#!/usr/bin/env python3
"""Check relative markdown links and required frontmatter in wiki/ + README.md."""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCAN_ROOTS = [ROOT / "wiki", ROOT / "README.md"]
REQUIRED_FM = ("id", "title", "summary", "status")
LINK_RE = re.compile(r"(?<!!)\[([^\]]*)\]\(([^)]+)\)")
HEADING_RE = re.compile(r"^(#{1,6})\s+(.+)$", re.M)
FM_RE = re.compile(r"^---\n(.*?)\n---\n", re.S)
FENCE_RE = re.compile(r"```.*?```", re.S)
INLINE_CODE_RE = re.compile(r"`[^`]+`")


def github_slug(text: str) -> str:
    text = re.sub(r"<[^>]+>", "", text)
    text = text.strip().lower()
    text = re.sub(r"[^\w\s-]", "", text, flags=re.U)
    text = re.sub(r"[\s_]+", "-", text)
    return text.strip("-")


def collect_md_files() -> list[Path]:
    files: list[Path] = []
    for root in SCAN_ROOTS:
        if root.is_file():
            files.append(root)
        else:
            files.extend(sorted(root.rglob("*.md")))
    return files


def parse_frontmatter(text: str) -> dict[str, str]:
    m = FM_RE.match(text)
    if not m:
        return {}
    out: dict[str, str] = {}
    for line in m.group(1).splitlines():
        if ":" in line:
            k, v = line.split(":", 1)
            out[k.strip()] = v.strip()
    return out


def anchors_in(text: str) -> set[str]:
    return {github_slug(h.group(2)) for h in HEADING_RE.finditer(text)}


def prose_spans(text: str) -> str:
    """Blank out fenced and inline code so example links are not checked."""
    text = FENCE_RE.sub(lambda m: "\n" * m.group(0).count("\n"), text)
    return INLINE_CODE_RE.sub(" ", text)


def main() -> int:
    errors: list[str] = []
    files = collect_md_files()
    by_path = {f.resolve(): f.read_text(encoding="utf-8") for f in files}

    for path, text in by_path.items():
        rel = path.relative_to(ROOT)
        checkable = prose_spans(text)

        for m in re.finditer(r"(?<![\w./-])docs/[A-Za-z0-9_./#-]+", checkable):
            errors.append(f"{rel}: lingering docs/ reference `{m.group(0)}`")

        is_readme = path.name == "README.md"
        if path.is_relative_to(ROOT / "wiki") and not is_readme:
            fm = parse_frontmatter(text)
            for key in REQUIRED_FM:
                if key not in fm:
                    errors.append(f"{rel}: missing frontmatter `{key}`")

        for m in LINK_RE.finditer(checkable):
            target = m.group(2).strip()
            if target.startswith(("http://", "https://", "mailto:")):
                continue
            if target.startswith("#"):
                slug = target[1:]
                if slug and slug not in anchors_in(text):
                    errors.append(f"{rel}: broken anchor `{target}`")
                continue
            if "://" in target:
                continue

            path_part, _, frag = target.partition("#")
            if not path_part:
                continue
            dest = (path.parent / path_part).resolve()
            if not dest.exists():
                errors.append(f"{rel}: broken link `{target}`")
                continue
            if frag:
                dest_text = by_path.get(dest)
                if dest_text is None and dest.suffix == ".md":
                    dest_text = dest.read_text(encoding="utf-8")
                if dest_text is not None and frag not in anchors_in(dest_text):
                    errors.append(f"{rel}: broken anchor in `{target}`")

    if errors:
        print("link-check failed:", file=sys.stderr)
        for e in errors:
            print(f"  {e}", file=sys.stderr)
        return 1
    print(f"ok: {len(files)} markdown files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
