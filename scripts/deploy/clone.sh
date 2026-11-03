#!/bin/sh
# specengine-clone <url> <name> <committer-name> <committer-email>, as in
# docs/features/hub-deploy.md "`clone.sh` (`specengine-clone`)": brings one
# project over, inside the image, so the container's git, user and deploy key
# make the checkout. A full clone (every blob, no depth or filter, no shallow
# source), sparse to the set `spec sparse-set` prints, with its upstream and a
# committer identity, under SPECENGINE_PROJECTS (default /projects). A bad
# argument exits 2 with nothing cloned; a failure after the clone exits 1 and
# leaves the folder, named, for the operator.
set -eu
LC_ALL=C
export LC_ALL

me=specengine-clone
dir=${SPECENGINE_PROJECTS:-/projects}

usage() {
    printf '%s: %s\n' "$me" "$1" >&2
    printf 'usage: %s <url> <name> <committer-name> <committer-email>\n' "$me" >&2
    exit 2
}

[ "$#" -eq 4 ] || usage "give four arguments, not $#"
url=$1
name=$2
committer_name=$3
committer_email=$4

case $name in
    '' | [!a-z0-9]* | *[!a-z0-9-]*)
        usage "the name $name must match [a-z0-9][a-z0-9-]*"
        ;;
esac
[ "${#name}" -le 64 ] || usage "the name $name is longer than 64 bytes"
target=$dir/$name
if [ -e "$target" ] || [ -L "$target" ]; then
    usage "$target exists: pick another name, or remove it first"
fi

# After the clone every failure leaves the folder for the operator to remove;
# a clone git gave up on is gone already.
fail() {
    if [ -e "$target" ]; then
        printf '%s: %s; %s is left: remove it before cloning again\n' "$me" "$1" "$target" >&2
    else
        printf '%s: %s; nothing is left at %s\n' "$me" "$1" "$target" >&2
    fi
    exit 1
}

# A full clone, never partial or shallow: a sync's look refuses either, so a
# shallow source is refused too (git 2.34 or later), leaving no folder.
git clone --no-checkout --reject-shallow --origin origin -- "$url" "$target" ||
    fail "git clone failed"

# The worktree on the default branch with the config alone: `spec sparse-set`
# needs it on disk, though it reads HEAD's.
printf '/specengine.toml\n' |
    git -C "$target" sparse-checkout set --no-cone --stdin ||
    fail "the sparse checkout of /specengine.toml failed"
git -C "$target" checkout --quiet ||
    fail "the checkout of the default branch failed"

branch=$(git -C "$target" symbolic-ref --quiet --short HEAD) ||
    fail "HEAD is on no branch"

# The set, anchored at the top; a directory keeps its trailing `/`. Glob
# characters are escaped, so each path names itself alone. Its warnings are
# expected here: the set is not checked out yet.
if ! paths=$(spec sparse-set --root "$target" 2>/dev/null); then
    why=$(spec sparse-set --root "$target" 2>&1 >/dev/null || true)
    fail "spec sparse-set failed: $why"
fi
[ -n "$paths" ] || fail "spec sparse-set printed no path"
printf '%s\n' "$paths" |
    sed -e 's/[[*?\\]/\\&/g' -e 's|^|/|' |
    git -C "$target" sparse-checkout set --no-cone --stdin ||
    fail "the sparse checkout of the set failed"

git -C "$target" config user.name "$committer_name" ||
    fail "setting user.name failed"
git -C "$target" config user.email "$committer_email" ||
    fail "setting user.email failed"

upstream=$(git -C "$target" rev-parse --abbrev-ref --symbolic-full-name '@{upstream}' 2>/dev/null) ||
    fail "the branch $branch has no upstream: it must track origin/$branch"
[ "$upstream" = "origin/$branch" ] ||
    fail "the branch $branch tracks $upstream, not origin/$branch"

if ! warnings=$(spec sparse-set --root "$target" 2>&1 >/dev/null); then
    fail "spec sparse-set failed: $warnings"
fi
case $warnings in
    *warning:*) fail "the checkout misses part of the set: $warnings" ;;
esac

count=$(printf '%s\n' "$paths" | wc -l | tr -d ' ')
printf 'cloned %s: %s tracking origin/%s, %s paths\n' "$name" "$branch" "$branch" "$count"
