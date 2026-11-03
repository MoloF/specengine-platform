#!/bin/sh
# specengine-entrypoint, as in
# docs/features/hub-deploy.md "`entrypoint.sh` (`specengine-entrypoint`)": serves
# every folder of the projects directory that holds a specengine.toml, in byte
# order, as one hub on 127.0.0.1:7777, the port Traefik's service URL names.
# Reads HUB_HOST, SPECENGINE_PUSH and SPECENGINE_PROJECTS (default /projects).
# Creates nothing; the daemon judges the rest.
set -eu
LC_ALL=C
export LC_ALL

me=specengine-entrypoint
# The daemon's port: a constant, equal to traefik/dynamic/hub.yml's service URL (S2).
port=7777

if [ -z "${HUB_HOST:-}" ]; then
    printf '%s: HUB_HOST is not set: give it in .env\n' "$me" >&2
    exit 2
fi

push=${SPECENGINE_PUSH:-}
case $push in
    '' | 0) push= ;;
    1) push=--push ;;
    *)
        printf '%s: SPECENGINE_PUSH is %s: give 0 or 1\n' "$me" "$push" >&2
        exit 2
        ;;
esac

dir=${SPECENGINE_PROJECTS:-/projects}
roots=0
set --
# `*` and not `*/`: a name sorts before the names it prefixes (`a` before
# `a-1`), which the trailing `/` of `*/` would break; files are passed over.
for entry in "$dir"/*; do
    [ -d "$entry" ] || continue
    if [ -f "$entry/specengine.toml" ]; then
        set -- "$@" --root "$entry"
        roots=$((roots + 1))
    else
        printf '%s: skipped %s: no specengine.toml\n' "$me" "$entry" >&2
    fi
done

if [ "$roots" -eq 0 ]; then
    printf '%s: no project under %s holds a specengine.toml: bring one over first (scripts/deploy/README.md)\n' "$me" "$dir" >&2
    exit 2
fi

if [ -n "$push" ]; then
    exec specengine-http "$@" --public-origin "https://$HUB_HOST" --port "$port" "$push"
fi
exec specengine-http "$@" --public-origin "https://$HUB_HOST" --port "$port"
