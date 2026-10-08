# SpecEngine's task gate, run by the plugin's PreToolUse hook (hooks.json).
# `spec gate --stdin` decides; this wrapper only makes sure no failure opens
# the gate. Exit 0 lets the write go on; exit 2 refuses it and hands stderr
# to the model. Claude Code lets the write through on any other code and on
# its own timeout, so every code but 0 becomes 2, spec's stdout is never
# passed on, and spec gets at most `limit` seconds (the first run of a fresh
# binary may wait on the system's scan) before the wrapper refuses, below
# the hook's timeout. A missing spec, or one without `gate`, is decided by
# the session's project: 2 only when its config names `[gate]`. POSIX sh;
# it reads no stdin itself.

limit=7

# A line naming the `gate` table, as `spec` tests a config that is not TOML:
# `[gate]`, `[ gate ]`, `["gate"]`, `[gate.x]`, `gate.mode = …`, `gate = {…}`.
gate_line='^[[:space:]]*(\[[[:space:]]*)?["'"'"']?gate["'"'"']?[[:space:]]*[].=]'

# Messages go to fd 4, the hook's stderr; the shell's own notices (a job
# killed) go nowhere; a closed reader never kills the cleanup.
exec 4>&2 2>/dev/null
trap '' PIPE

err=
pid=
dog=

finish() {
  if [ -n "$dog" ]; then
    kill "$dog" 2>/dev/null
  fi
  if [ -n "$err" ]; then
    [ -s "$err" ] && cat "$err" >&4
    rm -f "$err"
  fi
  exit "$1"
}

# Stopped (the hook ended by Claude Code, or spec past the limit): spec and
# the watchdog killed, the temp file removed, then the refusal.
stop() {
  if [ -n "$pid" ]; then
    kill -KILL "$pid" 2>/dev/null
  fi
  if [ -n "$dog" ] && [ "$2" != watchdog ]; then
    kill "$dog" 2>/dev/null
  fi
  dog=
  if [ -n "$err" ]; then
    rm -f "$err"
  fi
  err=
  echo "specengine gate: no verdict ($1): the write is refused." >&4
  exit 2
}

trap 'stop "spec gave no answer within $limit s" watchdog' USR1
trap 'stop "the hook was stopped"' HUP INT TERM

# No verdict from spec itself: the first specengine.toml at or above the
# session's project decides. It names `[gate]` (or cannot be read): exit 2
# with the cause $1 and the step $2; else exit 0.
by_project() {
  if [ -n "$err" ]; then
    rm -f "$err"
  fi
  err=
  d=${CLAUDE_PROJECT_DIR:-$PWD}
  while :; do
    if [ -f "$d/specengine.toml" ]; then
      grep -Eq "$gate_line" "$d/specengine.toml"
      case $? in
        1) finish 0 ;;
      esac
      echo "specengine gate: no verdict ($1): the write is refused. $2" >&4
      finish 2
    fi
    case $d in /|.|'') finish 0 ;; esac
    d=$(dirname "$d")
  done
}

if ! command -v spec >/dev/null 2>&1; then
  by_project "spec is not on PATH" "Put the directory of spec on the PATH Claude Code starts with."
fi

# spec's stderr goes to a file, so a spec still held past the limit keeps
# no pipe of Claude Code's open.
err=$(mktemp "${TMPDIR:-/tmp}/specengine-gate.XXXXXX" 2>/dev/null) || err=

exec 3<&0
if [ -n "$err" ]; then
  spec gate --stdin <&3 3<&- 4>&- >/dev/null 2>"$err" &
else
  spec gate --stdin <&3 3<&- >/dev/null 2>&4 4>&- &
fi
pid=$!
exec 3<&-

# The watchdog: one second at a time, `limit` times, then USR1 to the
# wrapper, which it signals only while it is still its parent: a wrapper gone
# (its PID free or reused) is never signalled, and the watchdog ends within a
# second of it. Killed early (no trap), it leaves at most one `sleep 1`.
sh -c 'i=0
while [ "$i" -lt "$1" ]; do
  sleep 1
  [ "$(ps -o ppid= -p $$ | tr -d " ")" = "$2" ] || exit 0
  i=$((i + 1))
done
kill -USR1 "$2"' specengine-gate-watchdog "$limit" "$$" </dev/null >/dev/null 2>&1 3<&- 4>&- &
dog=$!

wait "$pid"
code=$?
pid=
if [ "$code" -eq 0 ]; then
  finish 0
fi
if [ "$code" -eq 2 ] && [ -n "$err" ] && grep -q 'unrecognized subcommand' "$err"; then
  by_project "this spec has no gate" "Reinstall spec and the plugin from one commit."
fi
if [ "$code" -ne 2 ]; then
  echo "specengine gate: no verdict (spec ended with status $code): the write is refused." >&4
fi
finish 2
