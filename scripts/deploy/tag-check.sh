#!/bin/sh
# tag-check.sh <tag> (docs/features/hub-deploy.md "`tag-check.sh`"), image.yml's
# first step: one version, one build (ADR-0015). Exit 0 printing the version
# when the tag is exactly `v` + the plugin's version and .env.example's
# SPECENGINE_VERSION is that version; else exit 1. Paths from this script's
# own location.
set -eu
LC_ALL=C
export LC_ALL

here=$(cd "$(dirname "$0")" && pwd)
plugin=$here/../../plugin/specengine/.claude-plugin/plugin.json
example=$here/.env.example

tag=${1-}
# Two matches leave a newline in the value, which no tag equals.
version=$(sed -n 's/^[[:space:]]*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*$/\1/p' "$plugin" 2>/dev/null || true)
pinned=$(sed -n 's/^SPECENGINE_VERSION=//p' "$example" 2>/dev/null || true)

if [ "$#" -eq 1 ] && [ -n "$version" ] && [ "$tag" = "v$version" ] && [ "$pinned" = "$version" ]; then
    printf '%s\n' "$version"
    exit 0
fi
printf 'tag-check: %s is not v%s (plugin.json, .env.example)\n' "$tag" "$version" >&2
exit 1
