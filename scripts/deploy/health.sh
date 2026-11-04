#!/bin/bash
# specengine-health, as in docs/features/hub-ops.md "`specengine-health`":
# the daemon's probe, run by specengine's healthcheck inside Traefik's network
# namespace. Bash for its /dev/tcp alone, POSIX sh having no socket (S7);
# Debian's bash is Essential, so the image gains no package and no HTTP
# client. Alive is two answers:
#   1. a loopback `GET /` answered `HTTP/1.1 403 ` within 5 s, the fence
#      refusing a loopback Host (docs/canon/hub.md "Fence", step 1), so
#      nothing is read or recorded; no Remote-User, Origin or Sec-Fetch-*
#      header is sent (R6);
#   2. the proxy's port accepting a connection, which a Traefik crash leaves
#      dead in this namespace.
# Reads SPECENGINE_HEALTH_PORT (the entrypoint's 7777) and
# SPECENGINE_HEALTH_PROXY_PORT (websecure's 443). Exit 0 alive, 1 not, one
# stdout line either way. Bash 3.2 and 5.
set -u
export LC_ALL=C
# A peer that closes before the request is written fails the write, never
# the probe's own line.
trap '' PIPE

port=${SPECENGINE_HEALTH_PORT:-7777}
proxy_port=${SPECENGINE_HEALTH_PROXY_PORT:-443}

for value in "$port" "$proxy_port"; do
    case $value in
        '' | *[!0123456789]* | 0*)
            printf 'not alive: %s is not a port: give 1 to 65535\n' "${value//[[:cntrl:]]/?}"
            exit 1
            ;;
    esac
    if [ "${#value}" -gt 5 ] || [ "$value" -gt 65535 ]; then
        printf 'not alive: %s is not a port: give 1 to 65535\n' "$value"
        exit 1
    fi
done

if ! { exec 3<>"/dev/tcp/127.0.0.1/$port"; } 2>/dev/null; then
    printf 'not alive: no answer from 127.0.0.1:%s\n' "$port"
    exit 1
fi
printf 'GET / HTTP/1.1\r\nHost: 127.0.0.1:%s\r\nConnection: close\r\n\r\n' "$port" >&3 2>/dev/null
line=
IFS= read -r -t 5 line <&3
exec 3<&- 3>&-
line=${line%$'\r'}
if [ -z "$line" ]; then
    printf 'not alive: no answer from 127.0.0.1:%s\n' "$port"
    exit 1
fi
line=${line//[[:cntrl:]]/?}
case $line in
    'HTTP/1.1 403 '*) ;;
    *)
        printf 'not alive: 127.0.0.1:%s answered %s: a hub answers a loopback probe 403\n' "$port" "$line"
        exit 1
        ;;
esac

if ! { exec 4<>"/dev/tcp/127.0.0.1/$proxy_port"; } 2>/dev/null; then
    printf 'not alive: nothing listens on 127.0.0.1:%s: the proxy left this namespace; run docker compose up -d\n' "$proxy_port"
    exit 1
fi
exec 4<&- 4>&-
printf 'alive: %s\n' "$line"
exit 0
