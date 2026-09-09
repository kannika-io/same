#!/usr/bin/env bash
# Regenerate CHANGELOG.md from GitHub release notes.
#
# Usage: scripts/generate-changelog.sh [new-tag] [changelog-file]
#
# Writes one section per semver tag, newest first. Sections come from the
# published GitHub release when one exists; otherwise GitHub generates notes
# for the tag range on the fly. When <new-tag> does not exist yet, notes for it
# are generated against the current HEAD and placed at the top, dated today.
# The file is replaced wholesale, so do not edit it by hand.
set -euo pipefail

new_tag="${1:-}"
file="${2:-CHANGELOG.md}"

mapfile -t tags < <(git tag --list --sort=-v:refname | grep -E '^[0-9]+\.[0-9]+\.[0-9]+$')

if [ -n "$new_tag" ] && ! printf '%s\n' "${tags[@]}" | grep -qx "$new_tag"; then
  tags=("$new_tag" "${tags[@]}")
fi

# Print the notes body with headings demoted one level and the
# "New Contributors" block removed.
clean() {
  tr -d '\r' | sed 's/^#/##/' | awk '
    /^#+ New Contributors/ { skip = 1; next }
    skip && /^$/          { skip = 0; next }
    skip                  { next }
    { print }
  '
}

section() {
  local tag="$1" prev="$2" date body
  if body="$(gh release view "$tag" --json body -q .body 2>/dev/null)"; then
    date="$(gh release view "$tag" --json publishedAt -q .publishedAt)"
    date="${date%%T*}"
  else
    local args=(-f tag_name="$tag")
    [ -n "$prev" ] && args+=(-f previous_tag_name="$prev")
    if git rev-parse -q --verify "refs/tags/$tag" >/dev/null; then
      date="$(git log -1 --format=%cs "$tag")"
    else
      args+=(-f target_commitish="$(git rev-parse HEAD)")
      date="$(date -u +%F)"
    fi
    body="$(gh api "repos/{owner}/{repo}/releases/generate-notes" "${args[@]}" -q .body)"
  fi
  printf '## [%s] - %s\n\n%s\n\n' "$tag" "$date" "$(printf '%s' "$body" | clean)"
}

tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT
{
  printf '# Changelog\n\n'
  printf 'All notable changes to this project are documented here. This file is\n'
  printf 'generated from GitHub release notes by scripts/generate-changelog.sh;\n'
  printf 'do not edit it by hand.\n\n'
  for i in "${!tags[@]}"; do
    section "${tags[$i]}" "${tags[$((i + 1))]:-}"
  done
} > "$tmp"
mv "$tmp" "$file"

echo "Wrote ${file} with ${#tags[@]} release(s)"
