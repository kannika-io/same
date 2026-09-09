#!/usr/bin/env bash
# Prepend GitHub-generated release notes for a tag to CHANGELOG.md.
#
# Usage: scripts/update-changelog.sh <tag> [changelog-file]
#
# Asks GitHub to generate release notes for <tag> (the tag does not need to
# exist yet; notes are generated against the current HEAD) and inserts them as
# a new "## [<tag>] - <date>" section above the most recent existing section.
# Idempotent: exits without changes when the section already exists.
set -euo pipefail

tag="${1:?usage: $0 <tag> [changelog-file]}"
file="${2:-CHANGELOG.md}"

if grep -q "^## \[${tag}\]" "$file"; then
  echo "CHANGELOG already has a section for ${tag}, nothing to do"
  exit 0
fi

date="$(date -u +%F)"
body="$(gh api "repos/{owner}/{repo}/releases/generate-notes" \
  -f tag_name="$tag" \
  -f target_commitish="$(git rev-parse HEAD)" \
  -q .body | tr -d '\r' | sed 's/^#/##/')"

section="$(printf '## [%s] - %s\n\n%s\n' "$tag" "$date" "$body")"

tmp="$(mktemp)"
first="$(grep -n '^## \[' "$file" | head -n1 | cut -d: -f1 || true)"
if [ -n "$first" ]; then
  head -n "$((first - 1))" "$file" > "$tmp"
  printf '%s\n\n' "$section" >> "$tmp"
  tail -n "+${first}" "$file" >> "$tmp"
else
  cat "$file" > "$tmp"
  printf '\n%s\n' "$section" >> "$tmp"
fi
mv "$tmp" "$file"

echo "Added ${tag} section to ${file}"
