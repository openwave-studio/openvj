#!/usr/bin/env bash
# Prints the GitHub release notes for a version: a download line, then that version's section of
# CHANGELOG.md (from its "## [x.y.z]" heading to the next one), with repo-relative links made
# absolute so they work on the release page.
#
#   scripts/release-notes.sh 0.1.0     # or v0.1.0
#
# Fails if CHANGELOG.md has no section for the version, so a release isn't published without notes.
set -euo pipefail

version="${1:?usage: release-notes.sh <version>}"
version="${version#v}"
root="$(cd "$(dirname "$0")/.." && pwd)"
repo_url="https://github.com/openwave-studio/openvj"

section="$(awk -v v="$version" '
  $0 ~ "^## \\[" v "\\]" { found = 1; next }
  found && /^## \[/ { exit }
  found && /^\[[^]]+\]: / { next }
  found { print }
' "$root/CHANGELOG.md" | sed -e '/./,$!d')"

if [[ -z "$section" ]]; then
  echo "error: CHANGELOG.md has no section for $version" >&2
  exit 1
fi

cat <<NOTES
**Download:** \`openVJ-$version-macos-arm64.zip\` under Assets below (Apple Silicon, macOS 11 or later). Unzip it and drag openVJ.app to Applications. It's signed and notarized, with FFmpeg and Syphon built in.

NOTES
# Links like (THIRD_PARTY_NOTICES.md) or (docs/x.md) point into the repo at this tag.
sed -E "s#\]\(([A-Za-z0-9_./-]+\.(md|txt))\)#](${repo_url}/blob/v${version}/\1)#g" <<<"$section"
