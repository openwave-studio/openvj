#!/usr/bin/env python3
"""Imports Ghost Arcade's ISF shaders into shaders/ghost-arcade/.

Ghost Arcade (https://github.com/riskcapital/ghost-arcade) is AGPL-3.0. Its shaders are
bundled with openVJ in their own folder, with Ghost Arcade's LICENSE and a NOTICE.md.

The script:
  1. fetches the pinned commit below (or uses --src),
  2. collects the .fs files, drops exact duplicates and same-named copies,
  3. leaves out files that come from elsewhere under other licences (Shadertoy,
     glslsandbox, mojovideotech: usually CC BY-NC-SA, which is non-commercial and not
     GPL-compatible), see EXCLUDE_PATTERNS and EXCLUDE_FILES,
  4. drops files openVJ can't load (checked with the check_isf example),
  5. writes LICENSE and NOTICE.md.

Run from the repo root:  python3 scripts/import-ghost-arcade.py [--src <checkout>]
"""

import argparse
import hashlib
import json
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = "https://github.com/riskcapital/ghost-arcade.git"
COMMIT = "a2c62b104fab82f93081382cd592826f0457fd01"

# Upstream folder -> folder under shaders/ghost-arcade, in priority order (the first copy of a
# file name wins).
SOURCES = [
    ("public/ISF/cube shaders", "cube"),
    ("public/ISF", ""),
    ("ISF", "extra"),
    ("CuratedISF/synth", "curated/synth"),
    ("CuratedISF", "curated"),
    ("user-shaders", "examples"),
]

# Text that marks a file as derived from code under another licence.
EXCLUDE_PATTERNS = [
    r"shadertoy",
    r"glslsandbox",
    r"creative\s*commons",
    r"\bcc[- ]by",
    r"mojovideotech",
    r"iquilez",
    r"inigo\s+quilez",
]

# Files left out by hand, with the reason.
EXCLUDE_FILES = {
    "Magpie-Bloom.fs": "credited as made after another artist's work ('Visual Magpie')",
}

OUT = Path("shaders/ghost-arcade")


def header(src: str) -> dict:
    m = re.search(r"/\*\s*(\{.*?\})\s*\*/", src, re.S)
    if not m:
        return {}
    try:
        return json.loads(m.group(1))
    except json.JSONDecodeError:
        return {}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--src", type=Path, help="an existing ghost-arcade checkout (at any commit)")
    args = ap.parse_args()

    tmp = None
    src = args.src
    if src is None:
        tmp = Path(tempfile.mkdtemp(prefix="ghost-arcade-"))
        src = tmp / "ghost-arcade"
        subprocess.run(["git", "clone", "--quiet", "--filter=blob:none", REPO, str(src)], check=True)
        subprocess.run(["git", "-C", str(src), "checkout", "--quiet", COMMIT], check=True)
    commit = subprocess.run(
        ["git", "-C", str(src), "rev-parse", "HEAD"], capture_output=True, text=True
    ).stdout.strip() or "unknown"

    if OUT.exists():
        shutil.rmtree(OUT)
    OUT.mkdir(parents=True)

    seen_hashes = set()
    seen_names = set()
    kept = {}  # out path -> header
    excluded = []  # (upstream path, reason)
    for upstream, dest in SOURCES:
        folder = src / upstream
        if not folder.is_dir():
            continue
        for path in sorted(folder.glob("*.fs")):
            rel = path.relative_to(src).as_posix()
            data = path.read_bytes()
            digest = hashlib.sha1(data).hexdigest()
            if digest in seen_hashes or path.name in seen_names:
                continue
            seen_hashes.add(digest)
            seen_names.add(path.name)
            text = data.decode("utf-8", "replace")
            reason = EXCLUDE_FILES.get(path.name)
            if reason is None:
                hit = next((p for p in EXCLUDE_PATTERNS if re.search(p, text, re.I)), None)
                if hit:
                    reason = f"mentions /{hit}/: derived from code under another licence"
            if reason:
                excluded.append((rel, reason))
                continue
            out = OUT / dest / path.name
            out.parent.mkdir(parents=True, exist_ok=True)
            out.write_bytes(data)
            kept[out] = header(text)

    # Drop what openVJ can't load.
    check = subprocess.run(
        ["cargo", "run", "--quiet", "-p", "openvj-render", "--example", "check_isf", "--", str(OUT)],
        capture_output=True,
        text=True,
    )
    broken = []
    for line in check.stdout.splitlines():
        if line.startswith("ERROR\t"):
            _, p, msg = line.split("\t", 2)
            p = Path(p)
            p.unlink(missing_ok=True)
            kept.pop(p, None)
            broken.append((p.relative_to(OUT).as_posix(), msg))
    if check.returncode not in (0, 1):
        sys.stderr.write(check.stderr)
        return 1
    for d in sorted(OUT.rglob("*"), reverse=True):
        if d.is_dir() and not any(d.iterdir()):
            d.rmdir()

    shutil.copy(src / "LICENSE", OUT / "LICENSE")
    lines = [
        "# Ghost Arcade ISF shaders",
        "",
        "These shaders come from [Ghost Arcade](https://github.com/riskcapital/ghost-arcade)",
        f"(commit `{commit}`), © Risk Capital Media LLC and the Ghost Arcade contributors.",
        "They are licensed under the GNU Affero General Public License v3.0 (see LICENSE in",
        "this folder), not under openVJ's GPL-3.0. openVJ loads them at runtime as data files.",
        "",
        "Imported with `scripts/import-ghost-arcade.py`. Files are unchanged; openVJ translates",
        "Ghost Arcade's own names (`audioBass`, `sampleFFT`, …) when it compiles them.",
        "",
        f"## Included ({len(kept)} files)",
        "",
        "| File | Credit |",
        "| --- | --- |",
    ]
    for p in sorted(kept, key=lambda p: p.as_posix().lower()):
        credit = str(kept[p].get("CREDIT", "")).replace("|", "/").strip() or "Ghost Arcade"
        lines.append(f"| {p.relative_to(OUT).as_posix()} | {credit} |")
    lines += [
        "",
        f"## Left out: other licences ({len(excluded)} files)",
        "",
        "These are adapted from shaders published elsewhere (Shadertoy's default licence is",
        "CC BY-NC-SA 3.0, which forbids commercial use and isn't GPL-compatible). Users can",
        "still copy them into their own effects folder.",
        "",
    ]
    lines += [f"- `{p}`: {r}" for p, r in excluded]
    lines += [
        "",
        f"## Left out: don't load in openVJ yet ({len(broken)} files)",
        "",
    ]
    lines += [f"- `{p}`: {m}" for p, m in broken]
    (OUT / "NOTICE.md").write_text("\n".join(lines) + "\n")

    if tmp:
        shutil.rmtree(tmp, ignore_errors=True)
    print(f"{len(kept)} shaders in {OUT}, {len(excluded)} left out for licences, {len(broken)} broken")
    return 0


if __name__ == "__main__":
    sys.exit(main())
