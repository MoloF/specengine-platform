#!/bin/sh
# specengine-entrypoint, as in
# docs/features/hub-deploy.md "`entrypoint.sh` (`specengine-entrypoint`)": serves
# every folder of the projects directory that holds a specengine.toml, in byte
# order, as one hub on 127.0.0.1:7777, the port Traefik's service URL names.
# Reads HUB_HOST, SPECENGINE_PUSH, SPECENGINE_SLOW_MS (unset: the daemon's
# own threshold) and SPECENGINE_PROJECTS (default /projects).
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

# The slow-line threshold (docs/features/write-cost.md "The hub's slow-line
# threshold"): unset, the daemon's default and today's arguments; set, whole
# milliseconds from 0 to 3600000, else nothing runs. The value is never
# echoed: one line, whatever it holds.
slow=
if [ -n "${SPECENGINE_SLOW_MS+set}" ]; then
    slow=$SPECENGINE_SLOW_MS
    case $slow in
        '')
            printf '%s: SPECENGINE_SLOW_MS is set but empty: give whole milliseconds from 0 to 3600000, or remove it from .env\n' "$me" >&2
            exit 2
            ;;
        *[!0-9]* | ???????????*)
            printf '%s: SPECENGINE_SLOW_MS is not whole milliseconds from 0 to 3600000: fix it or remove it from .env\n' "$me" >&2
            exit 2
            ;;
    esac
    if [ "$slow" -gt 3600000 ]; then
        printf '%s: SPECENGINE_SLOW_MS is over 3600000 (an hour): fix it or remove it from .env\n' "$me" >&2
        exit 2
    fi
fi

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

if [ -n "$slow" ]; then
    set -- "$@" --public-origin "https://$HUB_HOST" --port "$port" --slow-ms "$slow"
else
    set -- "$@" --public-origin "https://$HUB_HOST" --port "$port"
fi
if [ -n "$push" ]; then
    exec specengine-http "$@" "$push"
fi
exec specengine-http "$@"
