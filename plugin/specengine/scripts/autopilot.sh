#!/bin/sh
# SpecEngine's auto runner (docs/features/auto-runner.md; ADR-0081): drives
# the pack the owner started with `spec auto start`, overnight. It loops
# `spec auto next`, a deterministic read, and does what it says: runs Claude
# Code headless per leg (`claude -p "/specengine:feature <T>"`, the plan in
# the main worktree, the implementation in a worktree of its own outside the
# repository), calls `spec auto approve` between the two and `spec auto land`
# after. Every choice is spec's; the runner judges a leg only from its
# stream-json output and the queue, never from claude's exit code or words.
# It never skips Claude Code's permissions: each leg runs with
# `--permission-prompts none`, so a tool the project's allowlist does not
# grant is denied and the task is held.
#
#   sh autopilot.sh [--root DIR] [--lanes N] [--by-department] [--until TIME]
#     [--max-tasks N] [--max-turns N] [--max-budget-usd X] [--model M]
#     [--accept-edits] [--poll S] [--pause S] [--leg-timeout S]
#     [--bg-wait-ms MS] [--worktrees DIR] [--log DIR] [--dry-run]
#
# Needs spec, claude, git and jq on PATH. Writes only the worktrees folder,
# the log folder, worktree registrations and task branches (through git);
# never commits, merges, pushes or edits the project's files itself.
# Exit 0: done, stopped, until or the task cap reached; 1: a lane stopped
# by two failures in a row, or spec failing three times in a row; 2: a bad
# flag or a missing tool, nothing run; 130 or 143: interrupted, every
# running leg sent TERM and waited for. Each leg leads its own process group
# (`setsid`, else perl), so a TERM reaches its tools and MCP server too.
# POSIX sh.

LC_ALL=C
export LC_ALL
# A line binding is the plan-line leg's alone.
unset SPECENGINE_AUTO_LINE

for tool in spec claude git jq; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    printf 'autopilot: %s is required on PATH\n' "$tool" >&2
    exit 2
  fi
done

root=.
lanes=2
by_department=0
until_time=
until_epoch=
max_tasks=
max_turns=200
max_budget=
model=
accept_edits=0
poll=30
pause_default=900
leg_timeout=14400
bg_wait_ms=3600000
worktrees=
log_dir=
dry_run=0
# Lock refusals in a row that end the run (`--poll` apart).
lock_retries=10

usage_error() {
  printf 'autopilot: %s\n' "$1" >&2
  exit 2
}

# A decimal of 1 to 9 digits.
is_count() {
  case $1 in
    '' | *[!0-9]*) return 1 ;;
  esac
  [ "${#1}" -le 9 ]
}

# A time in epoch seconds: a decimal of 1 to 12 digits.
is_epoch() {
  case $1 in
    '' | *[!0-9]*) return 1 ;;
  esac
  [ "${#1}" -le 12 ]
}

# `$2`, a count from `$3` to `$4`, for the flag `$1`.
counted() {
  is_count "$2" || usage_error "$1: \`$2\` is no whole number"
  if [ "$2" -lt "$3" ] || [ "$2" -gt "$4" ]; then
    usage_error "$1: $2 is out of range: $3 to $4"
  fi
}

while [ "$#" -gt 0 ]; do
  flag=$1
  value=
  case $flag in
    --by-department | --accept-edits | --dry-run) ;;
    --*=*)
      value=${flag#*=}
      flag=${flag%%=*}
      ;;
    --*)
      case $flag in
        --root | --lanes | --until | --max-tasks | --max-turns | --max-budget-usd | --model | \
          --poll | --pause | --leg-timeout | --bg-wait-ms | --worktrees | --log)
          [ "$#" -ge 2 ] || usage_error "$flag needs a value"
          value=$2
          shift
          ;;
      esac
      ;;
  esac
  case $flag in
    --root) root=$value ;;
    --lanes) counted "$flag" "$value" 1 16; lanes=$value ;;
    --by-department) by_department=1 ;;
    --until) until_time=$value ;;
    --max-tasks) counted "$flag" "$value" 1 999999999; max_tasks=$value ;;
    --max-turns) counted "$flag" "$value" 1 999999999; max_turns=$value ;;
    --max-budget-usd)
      case $value in
        '' | .* | *. | *[!0-9.]* | *.*.*) usage_error "--max-budget-usd: \`$value\` is no amount" ;;
      esac
      case $value in
        *[1-9]*) ;;
        *) usage_error "--max-budget-usd: $value is not above 0" ;;
      esac
      max_budget=$value
      ;;
    --model)
      case $value in
        '' | -*) usage_error "--model: \`$value\` is no model name" ;;
      esac
      model=$value
      ;;
    --accept-edits) accept_edits=1 ;;
    --poll) counted "$flag" "$value" 1 86400; poll=$value ;;
    --pause) counted "$flag" "$value" 1 604800; pause_default=$value ;;
    --leg-timeout) counted "$flag" "$value" 1 604800; leg_timeout=$value ;;
    --bg-wait-ms) counted "$flag" "$value" 0 999999999; bg_wait_ms=$value ;;
    --worktrees) worktrees=$value ;;
    --log) log_dir=$value ;;
    --dry-run) dry_run=1 ;;
    *) usage_error "unknown flag \`$flag\`" ;;
  esac
  shift
done

[ -n "$root" ] || usage_error "--root: an empty path"
if [ -n "$until_time" ]; then
  case $until_time in
    [0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z) ;;
    *) usage_error "--until: \`$until_time\` is no time YYYY-MM-DDTHH:MM:SSZ" ;;
  esac
  until_epoch=$(jq -n --arg t "$until_time" '$t | fromdateiso8601' 2>/dev/null)
  is_epoch "$until_epoch" || usage_error "--until: \`$until_time\` is no time YYYY-MM-DDTHH:MM:SSZ"
fi

root=$(cd "$root" 2>/dev/null && pwd -P) || usage_error "--root: no such directory"
top=$(git -C "$root" rev-parse --show-toplevel 2>/dev/null) ||
  usage_error "--root $root lies in no git worktree"
main=$(git -C "$root" worktree list --porcelain 2>/dev/null | sed -n '1s/^worktree //p')
main=$(cd "$main" 2>/dev/null && pwd -P) || usage_error "--root $root: git names no main worktree"
top=$(cd "$top" && pwd -P)
[ "$top" = "$main" ] ||
  usage_error "--root $root lies in the worktree $top: run it from the main worktree $main"
rel=$(git -C "$root" rev-parse --show-prefix 2>/dev/null)
rel=${rel%/}

# An absolute path of `$1`, from the current directory.
absolute() {
  case $1 in
    /*) printf '%s' "$1" ;;
    *) printf '%s/%s' "$(pwd -P)" "$1" ;;
  esac
}

[ -n "$worktrees" ] || worktrees=$(dirname "$main")/$(basename "$main").auto
worktrees=$(absolute "${worktrees%/}")
case $worktrees/ in
  "$main"/*) usage_error "--worktrees $worktrees lies inside the repository $main" ;;
esac
[ -n "$log_dir" ] || log_dir=$worktrees/.log
log_dir=$(absolute "${log_dir%/}")
case $log_dir/ in
  "$main"/*) usage_error "--log $log_dir lies inside the repository $main" ;;
esac

log_file=
state=
nap_pid=

now_epoch() {
  date -u +%s
}

now_iso() {
  date -u +%Y-%m-%dT%H:%M:%SZ
}

# The UTC time of the epoch second `$1`.
iso_of() {
  jq -nr --argjson s "$1" '$s | todateiso8601'
}

# One line, controls stripped.
plain() {
  printf '%s' "$1" | tr '\n\r\t' '   ' | tr -d '\000-\037\177'
}

# A runner's line: stderr and the log.
say() {
  said=$(plain "autopilot: $1")
  printf '%s\n' "$said" >&2
  if [ -n "$log_file" ]; then
    printf '%s %s\n' "$(now_iso)" "$said" >>"$log_file"
  fi
}

# `<UTC time> <item> <leg or act> <outcome> <detail>`: stderr and the log.
act_line() {
  acted=$(plain "$(now_iso) $1 $2 $3 $4")
  acted=$(printf '%s' "$acted" | sed 's/ *$//')
  printf '%s\n' "$acted" >&2
  if [ -n "$log_file" ]; then
    printf '%s\n' "$acted" >>"$log_file"
  fi
}

# `$1` as one shell word.
quote() {
  case $1 in
    '' | *[!A-Za-z0-9_./:=@%+,-]*)
      printf "'%s'" "$(printf '%s' "$1" | sed "s/'/'\\\\''/g")"
      ;;
    *) printf '%s' "$1" ;;
  esac
}

# A file name for the text `$1`: its bytes in hex.
hexname() {
  printf '%s' "$1" | od -An -tx1 | tr -d ' \n'
}

# A note `spec auto hold` takes: no control character, at most 1000 bytes.
clean_note() {
  jq -nr --arg s "$1" '
    $s | gsub("[\\x{0}-\\x{1f}\\x{7f}-\\x{9f}]"; " ")
    | if utf8bytelength <= 1000 then .
      else .[0:1000] | until(utf8bytelength <= 1000; .[0:length - 1]) end'
}

# Sleeps `$1` seconds, a signal ending it at once.
nap() {
  sleep "$1" &
  nap_pid=$!
  wait "$nap_pid" 2>/dev/null
  nap_pid=
}

# The `claude` argv of a leg whose prompt is `$1`, one word per line (no
# word holds a line end: a line's text has no control character).
leg_argv() {
  printf '%s\n' claude -p "$1" --output-format stream-json --verbose \
    --permission-prompts none --max-turns "$max_turns"
  if [ -n "$max_budget" ]; then
    printf '%s\n' --max-budget-usd "$max_budget"
  fi
  if [ -n "$model" ]; then
    printf '%s\n' --model "$model"
  fi
  if [ "$accept_edits" = 1 ]; then
    printf '%s\n' --permission-mode acceptEdits
  fi
}

# The leg's working directory: the project root in the main worktree, or
# in the task's worktree.
leg_dir() {
  case $1 in
    plan | plan-line) printf '%s' "$root" ;;
    *)
      if [ -n "$rel" ]; then
        printf '%s/%s/%s' "$worktrees" "$2" "$rel"
      else
        printf '%s/%s' "$worktrees" "$2"
      fi
      ;;
  esac
}

# The prompt of a leg: its subject `$1`, the task or a line's text.
leg_prompt() {
  printf '/specengine:feature %s' "$1"
}

# Reads `spec auto next`'s document `$1` into n_* variables.
read_next() {
  n_action=
  eval "$(printf '%s' "$1" | jq -r '
    def text: if . == null then "" else tostring end;
    @sh "n_action=\(.action | text)",
    @sh "n_reason=\(.reason | text)",
    @sh "n_why=\(.why | text)",
    @sh "n_pack=\(.pack | text)",
    @sh "n_task=\(.task | text)",
    @sh "n_text=\(.text | text)",
    @sh "n_lane=\(if .lane == null then "general" else .lane | tostring end)",
    @sh "n_branch=\(.branch | text)",
    @sh "n_base=\(.base | text)",
    @sh "n_line=\(
      if .action == "plan-line" then
        if (.item | text | contains("/")) then .item | text
        elif (.line | type) == "string" and (.line | contains("/")) then .line
        elif .line != null then "\(.pack)/\(.line)"
        else .item | text end
      else "" end)",
    @sh "n_item=\(if .item != null then .item | text else .task | text end)"
  ' 2>/dev/null)" || n_action=
  if [ "$n_action" = plan-line ]; then
    n_item=$n_line
  fi
}

# The log name of an item: a task's ID, or `<pack>-<n>`.
item_name() {
  printf '%s' "$1" | tr '/' '-'
}

# The first free `k` of the item's leg stream in the log.
next_k() {
  nk=1
  while [ -e "$log_dir/$(item_name "$1")-$2-$nk.jsonl" ]; do
    nk=$((nk + 1))
  done
  printf '%s' "$nk"
}

# The pid `$1` and every process below it, `$1` first.
tree_pids() {
  ps -A -o pid= -o ppid= 2>/dev/null | awk -v root="$1" '
    { parent[$1] = $2 }
    END {
      n = 1
      list[1] = root
      for (i = 1; i <= n; i++)
        for (p in parent)
          if (parent[p] == list[i]) list[++n] = p
      for (i = 1; i <= n; i++) print list[i]
    }'
}

# A process of the process group `$1` is alive.
group_alive() {
  ps -A -o pgid= 2>/dev/null | tr -d ' ' | grep -qx "$1"
}

# Sends the signal `$1` to the leg whose folder is `$2`: to its whole
# process group when it leads one, else to its process and every process
# below it (a process already orphaned is out of reach then).
signal_leg() {
  sg_pid=$(cat "$2/pid" 2>/dev/null)
  [ -n "$sg_pid" ] || return 0
  if [ -f "$2/group" ]; then
    kill -s "$1" -- "-$sg_pid" 2>/dev/null
  else
    for sg_one in $(tree_pids "$sg_pid"); do
      kill -s "$1" "$sg_one" 2>/dev/null
    done
  fi
}

# A process of the leg whose folder is `$1` is alive: of its group, else
# its own.
leg_alive() {
  la_pid=$(cat "$1/pid" 2>/dev/null)
  [ -n "$la_pid" ] || return 1
  if [ -f "$1/group" ]; then
    group_alive "$la_pid"
  else
    kill -0 "$la_pid" 2>/dev/null
  fi
}

# What a finished leg left in its group (a tool's child, an MCP server):
# TERM now, KILL 30 s on if any is still there.
sweep_group() {
  group_alive "$1" || return 0
  kill -s TERM -- "-$1" 2>/dev/null
  (
    sleep 30
    if group_alive "$1"; then
      kill -s KILL -- "-$1" 2>/dev/null
    fi
  ) >/dev/null 2>&1 &
}

legs_running() {
  lr=0
  for lr_dir in "$state"/legs/* "$state"/pending/*; do
    [ -d "$lr_dir" ] && lr=$((lr + 1))
  done
  printf '%s' "$lr"
}

# Starts the leg `$2` of item `$1` in the background: `$3` its lane, `$4`
# its task (none for a line), `$5` the prompt's subject, `$6` the line.
start_leg() {
  sl_item=$1
  sl_leg=$2
  sl_lane=$3
  sl_task=$4
  sl_subject=$5
  sl_line=$6
  sl_k=$(next_k "$sl_item" "$sl_leg")
  sl_base=$log_dir/$(item_name "$sl_item")-$sl_leg-$sl_k
  sl_dir=$state/legs/$(hexname "$sl_item")
  sl_cwd=$(leg_dir "$sl_leg" "$sl_task")
  sl_prompt=$(leg_prompt "$sl_subject")
  mkdir -p "$sl_dir"
  printf '%s' "$sl_item" >"$sl_dir/item"
  printf '%s' "$sl_leg" >"$sl_dir/leg"
  printf '%s' "$sl_lane" >"$sl_dir/lane"
  printf '%s' "$sl_task" >"$sl_dir/task"
  printf '%s' "$sl_subject" >"$sl_dir/subject"
  printf '%s' "$sl_line" >"$sl_dir/line"
  printf '%s' "$sl_base.jsonl" >"$sl_dir/stream"
  printf '%s' "$sl_base.err" >"$sl_dir/err"
  now_epoch >"$sl_dir/start"
  (
    cd "$sl_cwd" 2>/dev/null || {
      printf 'autopilot: the leg has no working directory %s\n' "$sl_cwd" >"$sl_base.err"
      : >"$sl_base.jsonl"
      : >"$sl_dir/nodir"
      echo 127 >"$sl_dir/exit.tmp"
      mv "$sl_dir/exit.tmp" "$sl_dir/exit"
      exit 0
    }
    CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS=$bg_wait_ms
    export CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS
    if [ "$sl_leg" = plan-line ]; then
      SPECENGINE_AUTO_LINE=$sl_line
      export SPECENGINE_AUTO_LINE
    fi
    set --
    while IFS= read -r word; do
      set -- "$@" "$word"
    done <<EOF
$(leg_argv "$sl_prompt")
EOF
    # The leg leads a process group of its own, so a TERM reaches the
    # agent's tools and MCP server too: `setsid` where there is one (it does
    # not fork here, this child leading no group), else perl's `setpgrp`;
    # with neither, the leg runs in the runner's group (see `signal_leg`).
    if command -v setsid >/dev/null 2>&1; then
      set -- setsid "$@"
      : >"$sl_dir/group"
    elif command -v perl >/dev/null 2>&1; then
      set -- perl -e 'setpgrp(0, 0) or die "autopilot: setpgrp: $!\n";
        exec { $ARGV[0] } @ARGV or die "autopilot: cannot run $ARGV[0]: $!\n"' "$@"
      : >"$sl_dir/group"
    fi
    "$@" </dev/null >"$sl_base.jsonl" 2>"$sl_base.err" &
    echo "$!" >"$sl_dir/pid"
    wait "$!"
    echo "$?" >"$sl_dir/exit.tmp"
    mv "$sl_dir/exit.tmp" "$sl_dir/exit"
  ) 2>/dev/null &
  echo "$!" >"$sl_dir/wpid"
  act_line "$sl_item" "$sl_leg" start "$sl_base.jsonl"
}

# The stream `$1` judged: one line `<kind>\t<reason>\t<note or resetsAt>`,
# kind `usage`, `hold`, `noresult` or `ok` (auto-runner.md "Classifying",
# steps 1 to 5).
classify() {
  jq -R -s -r '
    def note: gsub("[\\x{0}-\\x{1f}\\x{7f}-\\x{9f}]"; " ");
    def bytes($n): if utf8bytelength <= $n then .
      else .[0:$n] | until(utf8bytelength <= $n; .[0:length - 1]) end;
    [split("\n")[] | select(length > 0) | (fromjson? // empty) | select(type == "object")]
    as $ev
    | ($ev | map(select(.type == "rate_limit_event"
        and (.rate_limit_info | type) == "object"
        and .rate_limit_info.status == "rejected"))) as $limited
    | if ($limited | length) > 0 then
        ["usage", "", ($limited | last | .rate_limit_info.resetsAt
          | if type == "number" then floor | tostring
            elif type == "string" then (tonumber? // "" | tostring)
            else "" end)]
      else
        ($ev | map(select(.type == "system" and .subtype == "init")) | first) as $init
        | if $init == null or ((($init.mcp_servers // []) | map(select(type == "object"
              and ((.name // "") == "specengine" or ((.name // "") | tostring | endswith(":specengine")))
              and .status == "connected")) | length) == 0) then
            ["hold", "no-mcp", "the specengine MCP server was not connected"]
          else
            ($ev | map(select(.type == "result")) | last) as $result
            | if $result == null then ["noresult", "", ""]
              elif (($result.permission_denials // []) | length) > 0 then
                ["hold", "denied-tool", "denied: " + ($result.permission_denials
                  | map(.tool_name // "-" | tostring)
                  | reduce .[] as $name ([]; if any(.[]; . == $name) then . else . + [$name] end)
                  | .[0:8] | join(", "))]
              elif $result.subtype == "error_max_turns" then ["hold", "max-turns", ""]
              elif $result.subtype == "error_max_budget_usd" then ["hold", "max-budget", ""]
              elif (($result.subtype // "") | tostring | startswith("error_"))
                or $result.is_error == true then
                ["hold", "agent-error",
                  (($result.subtype // "-" | tostring) + ": "
                    + ($result.terminal_reason // "-" | tostring) + ": "
                    + ($result.result // "" | tostring | bytes(200)))]
              else ["ok", "", ""] end
          end
      end
    | map(note) | join("\t")
  ' "$1" 2>/dev/null || printf 'noresult\t\t'
}

# Holds item `$1` (`spec auto hold`): reason `$2`, note `$3` (none: the
# reason); `$4` the leg, for the log. Every value that may be free text is
# passed as `--flag=value`, so one starting with `-` is never read as a flag.
hold() {
  h_note=$(clean_note "$3")
  if [ -n "$h_note" ]; then
    h_out=$(spec auto hold --reason="$2" --note="$h_note" --root="$root" -- "$1" 2>&1)
  else
    h_out=$(spec auto hold --reason="$2" --root="$root" -- "$1" 2>&1)
  fi
  h_code=$?
  if [ "$h_code" -eq 0 ]; then
    act_line "$1" "$4" "held $2" "$h_note"
  else
    act_line "$1" "$4" skipped "spec auto hold exited $h_code: $(printf '%s' "$h_out" | head -n 1)"
  fi
}

# A failing leg on lane `$1`: two in a row stop it.
lane_failed() {
  lf_file=$state/lanes/$(hexname "$1")
  lf=0
  [ -f "$lf_file.fails" ] && lf=$(cat "$lf_file.fails")
  lf=$((lf + 1))
  printf '%s' "$lf" >"$lf_file.fails"
  printf '%s' "$1" >"$lf_file.name"
  if [ "$lf" -ge 2 ] && [ ! -f "$lf_file.stopped" ]; then
    : >"$lf_file.stopped"
    say "lane $1 stopped: two failures in a row"
  fi
}

lane_passed() {
  rm -f "$state/lanes/$(hexname "$1").fails"
}

# The stopped lanes' names, one per line.
stopped_lanes() {
  for sp in "$state"/lanes/*.stopped; do
    [ -f "$sp" ] && cat "${sp%.stopped}.name" && echo
  done
}

# Step 6: the queue after a leg that passed its stream.
queue_verdict() {
  qv_leg=$1
  qv_item=$2
  qv_task=$3
  qv_line=$4
  q_reason=
  q_note=
  case $qv_leg in
    plan | implement)
      qv_doc=$(spec task show --root="$root" --json -- "$qv_task" 2>/dev/null)
      qv_status=$(printf '%s' "$qv_doc" | jq -r '.status // "unknown"' 2>/dev/null)
      [ -n "$qv_status" ] || qv_status=unknown
      if [ "$qv_leg" = plan ]; then
        if [ "$qv_status" != review ]; then
          q_reason=not-planned
          q_note="$qv_task is $qv_status after the plan leg"
        fi
      elif [ "$qv_status" != done ]; then
        q_reason=not-done
        q_note=$(printf '%s' "$qv_doc" | jq -r --arg s "$qv_status" '
          (.runs // []) | last
          | if . == null then "\($s); no run"
            else "\($s); run \(.run) \(.outcome // "-")" end' 2>/dev/null)
        [ -n "$q_note" ] || q_note="$qv_status; no run"
      fi
      ;;
    plan-line)
      qv_bound=$(spec auto show --root="$root" --json 2>/dev/null | jq -r \
        --arg line "$qv_line" --arg n "${qv_line##*/}" --arg pack "${qv_line%%/*}" '
        [(.packs // [])[] | select((.pack | tostring) == $pack) | (.items // [])[]
          | select(.line != null and ((.line | tostring) == $line or (.line | tostring) == $n))]
        | first
        | if . == null then "\($line) is unbound"
          elif .task == null then "\($line) is \(.status // "unbound")"
          elif .status == "review" then "planned"
          else "\(.task) is \(.status)" end' 2>/dev/null)
      [ -n "$qv_bound" ] || qv_bound="$qv_line could not be read"
      if [ "$qv_bound" != planned ]; then
        q_reason=not-planned
        q_note="$qv_bound after the plan leg"
      fi
      ;;
  esac
}

# A finished leg (its `exit` file there), judged and forgotten.
finish_leg() {
  fl_dir=$1
  fl_item=$(cat "$fl_dir/item")
  fl_leg=$(cat "$fl_dir/leg")
  fl_lane=$(cat "$fl_dir/lane")
  fl_task=$(cat "$fl_dir/task")
  fl_subject=$(cat "$fl_dir/subject")
  fl_line=$(cat "$fl_dir/line")
  fl_stream=$(cat "$fl_dir/stream")
  fl_code=$(cat "$fl_dir/exit")
  fl_timed=0
  [ -f "$fl_dir/termed" ] && fl_timed=1
  fl_nodir=0
  [ -f "$fl_dir/nodir" ] && fl_nodir=1
  fl_err=$(cat "$fl_dir/err")
  wait "$(cat "$fl_dir/wpid")" 2>/dev/null
  if [ -f "$fl_dir/group" ] && [ -f "$fl_dir/pid" ]; then
    sweep_group "$(cat "$fl_dir/pid")"
  fi
  rm -rf "${fl_dir:?}"
  # The item a hold names: a line's leg holds the line.
  if [ "$fl_leg" = plan-line ]; then
    fl_held=$fl_line
  else
    fl_held=$fl_item
  fi
  if [ "$fl_timed" = 1 ]; then
    hold "$fl_held" agent-error "timed out after $leg_timeout s" "$fl_leg"
    lane_failed "$fl_lane"
    return
  fi
  # No working directory: no claude ran, so no stream to judge.
  if [ "$fl_nodir" = 1 ]; then
    hold "$fl_held" agent-error "$(head -n 1 "$fl_err")" "$fl_leg"
    lane_failed "$fl_lane"
    return
  fi
  fl_verdict=$(classify "$fl_stream")
  fl_kind=$(printf '%s' "$fl_verdict" | cut -f 1)
  fl_reason=$(printf '%s' "$fl_verdict" | cut -f 2)
  fl_note=$(printf '%s' "$fl_verdict" | cut -f 3-)
  case $fl_kind in
    usage)
      fl_now=$(now_epoch)
      if is_epoch "$fl_note"; then
        fl_until=$((fl_note + 60))
      else
        fl_until=$((fl_now + pause_default))
      fi
      [ "$fl_until" -gt "$pause_until" ] && pause_until=$fl_until
      fl_pending=$state/pending/$(hexname "$fl_item")
      mkdir -p "$fl_pending"
      printf '%s' "$fl_item" >"$fl_pending/item"
      printf '%s' "$fl_leg" >"$fl_pending/leg"
      printf '%s' "$fl_lane" >"$fl_pending/lane"
      printf '%s' "$fl_task" >"$fl_pending/task"
      printf '%s' "$fl_subject" >"$fl_pending/subject"
      printf '%s' "$fl_line" >"$fl_pending/line"
      act_line "$fl_item" "$fl_leg" "paused $(iso_of "$pause_until")" "a usage limit"
      ;;
    hold)
      hold "$fl_held" "$fl_reason" "$fl_note" "$fl_leg"
      lane_failed "$fl_lane"
      ;;
    noresult)
      hold "$fl_held" agent-error "no result; claude exited $fl_code" "$fl_leg"
      lane_failed "$fl_lane"
      ;;
    *)
      queue_verdict "$fl_leg" "$fl_item" "$fl_task" "$fl_line"
      if [ -n "$q_reason" ]; then
        hold "$fl_held" "$q_reason" "$q_note" "$fl_leg"
        lane_failed "$fl_lane"
      else
        act_line "$fl_item" "$fl_leg" ok ""
        lane_passed "$fl_lane"
      fi
      ;;
  esac
}

# A leg past `--leg-timeout` is sent TERM; one 30 s past its TERM, KILL;
# each to its whole process group (`signal_leg`).
watch_leg() {
  [ -f "$1/pid" ] || return
  wl_now=$(now_epoch)
  if [ -f "$1/termed" ]; then
    if [ $((wl_now - $(cat "$1/termed"))) -ge 30 ]; then
      signal_leg KILL "$1"
    fi
  elif [ $((wl_now - $(cat "$1/start"))) -ge "$leg_timeout" ]; then
    printf '%s' "$wl_now" >"$1/termed"
    signal_leg TERM "$1"
  fi
}

reap() {
  for rp_dir in "$state"/legs/*; do
    [ -d "$rp_dir" ] || continue
    if [ -f "$rp_dir/exit" ]; then
      finish_leg "$rp_dir"
    else
      watch_leg "$rp_dir"
    fi
  done
}

# The legs a usage limit paused, started again once the pause is over (none
# after `--until`).
restart_pending() {
  for rs_dir in "$state"/pending/*; do
    [ -d "$rs_dir" ] || continue
    rs_item=$(cat "$rs_dir/item")
    rs_leg=$(cat "$rs_dir/leg")
    rs_lane=$(cat "$rs_dir/lane")
    rs_task=$(cat "$rs_dir/task")
    rs_subject=$(cat "$rs_dir/subject")
    rs_line=$(cat "$rs_dir/line")
    rm -rf "${rs_dir:?}"
    if past_until; then
      act_line "$rs_item" "$rs_leg" skipped "until $until_time"
    else
      start_leg "$rs_item" "$rs_leg" "$rs_lane" "$rs_task" "$rs_subject" "$rs_line"
    fi
  done
}

past_until() {
  [ -n "$until_epoch" ] && [ "$(now_epoch)" -ge "$until_epoch" ]
}

# Makes `<worktrees>/<T>` on branch `$2` from `$3` when absent.
ensure_worktree() {
  ew_dir=$worktrees/$1
  [ -d "$ew_dir" ] && return 0
  mkdir -p "$worktrees" || return 1
  if git -C "$root" show-ref --verify --quiet "refs/heads/$2"; then
    git -C "$root" worktree add "$ew_dir" "$2" >"$state/git.out" 2>&1
  else
    git -C "$root" worktree add -b "$2" "$ew_dir" "$3" >"$state/git.out" 2>&1
  fi
}

# How many times in a row an act on an item failed without spec's hold:
# three end the run (a land or approve spec cannot run would loop).
act_failed() {
  af_file=$state/acts/$(hexname "$1-$2")
  af=0
  [ -f "$af_file" ] && af=$(cat "$af_file")
  af=$((af + 1))
  printf '%s' "$af" >"$af_file"
  if [ "$af" -ge 3 ]; then
    wait_legs
    say "spec auto $2 $1 failed 3 times in a row: $3"
    finish 1
  fi
}

act_passed() {
  rm -f "$state/acts/$(hexname "$1-$2")"
}

do_approve() {
  da_task=$1
  if ! ensure_worktree "$da_task" "$2" "$3"; then
    hold "$da_task" agent-error \
      "the runner could not make the worktree $worktrees/$da_task: $(head -n 1 "$state/git.out")" approve
    return
  fi
  da_root=$worktrees/$da_task
  [ -n "$rel" ] && da_root=$da_root/$rel
  da_out=$(spec auto approve --root="$da_root" --json -- "$da_task" 2>"$state/act.err")
  da_code=$?
  da_reason=$(printf '%s' "$da_out" | jq -r '.reason // empty' 2>/dev/null)
  da_detail=$(printf '%s' "$da_out" | jq -r '.detail // empty' 2>/dev/null)
  if [ "$da_code" -eq 0 ]; then
    act_passed "$da_task" approve
    act_line "$da_task" approve ok "$(printf '%s' "$da_out" | jq -r '.status // empty' 2>/dev/null)"
  elif [ "$da_code" -eq 1 ] && [ -n "$da_reason" ]; then
    act_passed "$da_task" approve
    act_line "$da_task" approve "held $da_reason" "$da_detail"
  else
    da_why=$(head -n 1 "$state/act.err")
    act_line "$da_task" approve skipped "exit $da_code: $da_why"
    act_failed "$da_task" approve "$da_why"
    nap "$poll"
  fi
}

# A land refused for the lock, `$1` the task, `$2` spec's words: retried
# after `--poll`, at most `lock_retries` times in a row (a lock whose land
# died stays until the owner removes it); past `--until` with nothing
# running, the run ends.
lock_refused() {
  lr_file=$state/acts/$(hexname "$1-lock")
  lr=0
  [ -f "$lr_file" ] && lr=$(cat "$lr_file")
  lr=$((lr + 1))
  printf '%s' "$lr" >"$lr_file"
  act_line "$1" land skipped "another land holds the lock: retry $lr of $lock_retries"
  if past_until && [ "$(legs_running)" -eq 0 ]; then
    say "until $until_time"
    finish 0
  fi
  if [ "$lr" -ge "$lock_retries" ]; then
    wait_legs
    say "spec auto land $1 found the lock held $lock_retries times in a row: $2"
    finish 1
  fi
  nap "$poll"
}

# `spec auto land`, INT ignored: Ctrl-C reaches the runner, which acts on it
# once the land ends, never a land mid-merge.
do_land() {
  dl_task=$1
  dl_out=$(
    trap '' INT
    exec spec auto land --root="$root" --json -- "$dl_task" 2>"$state/act.err"
  )
  dl_code=$?
  if [ "$dl_code" -ne 1 ] || ! { printf '%s' "$dl_out" | grep -q 'another land holds' ||
    grep -q 'another land holds' "$state/act.err"; }; then
    rm -f "$state/acts/$(hexname "$dl_task-lock")"
  fi
  dl_reason=$(printf '%s' "$dl_out" | jq -r '.reason // empty' 2>/dev/null)
  dl_detail=$(printf '%s' "$dl_out" | jq -r '.detail // empty' 2>/dev/null)
  if [ "$dl_code" -eq 0 ]; then
    act_passed "$dl_task" land
    dl_commit=$(printf '%s' "$dl_out" | jq -r '.commit // "-" | .[0:12]' 2>/dev/null)
    act_line "$dl_task" land "landed $dl_commit" ""
    if [ -d "$worktrees/$dl_task" ]; then
      if ! git -C "$root" worktree remove "$worktrees/$dl_task" >"$state/git.out" 2>&1; then
        act_line "$dl_task" remove skipped "$(head -n 1 "$state/git.out")"
      fi
    fi
  elif [ "$dl_code" -eq 1 ] && { printf '%s' "$dl_out" | grep -q 'another land holds' ||
    grep -q 'another land holds' "$state/act.err"; }; then
    lock_refused "$dl_task" "$( (printf '%s\n' "$dl_out"; cat "$state/act.err") |
      grep 'another land holds' | head -n 1)"
  elif [ "$dl_code" -eq 1 ] && [ -n "$dl_reason" ]; then
    act_passed "$dl_task" land
    act_line "$dl_task" land "held $dl_reason" "$dl_detail"
  else
    dl_why=$(head -n 1 "$state/act.err")
    act_line "$dl_task" land skipped "exit $dl_code: $dl_why"
    act_failed "$dl_task" land "$dl_why"
    nap "$poll"
  fi
}

# Waits for every running leg, judging each; starts nothing.
wait_legs() {
  while [ -n "$(ls "$state/legs" 2>/dev/null)" ]; do
    reap
    [ -n "$(ls "$state/legs" 2>/dev/null)" ] && nap 1
  done
  # Paused legs are not started again.
  for wp_dir in "$state"/pending/*; do
    [ -d "$wp_dir" ] || continue
    act_line "$(cat "$wp_dir/item")" "$(cat "$wp_dir/leg")" skipped "the run ends"
    rm -rf "${wp_dir:?}"
  done
}

finish() {
  if [ -n "$state" ]; then
    rm -rf "${state:?}"
  fi
  exit "$1"
}

# INT, TERM: every running leg's process group sent TERM and waited for
# (KILL 30 s on); items keep their state.
interrupted() {
  trap '' INT TERM
  if [ -n "$nap_pid" ]; then
    kill "$nap_pid" 2>/dev/null
  fi
  if [ -n "$state" ]; then
    for in_dir in "$state"/legs/*; do
      [ -d "$in_dir" ] && signal_leg TERM "$in_dir"
    done
    in_waited=0
    while :; do
      in_alive=0
      for in_dir in "$state"/legs/*; do
        [ -d "$in_dir" ] || continue
        if leg_alive "$in_dir"; then
          in_alive=1
          [ "$in_waited" -ge 30 ] && signal_leg KILL "$in_dir"
        fi
      done
      [ "$in_alive" = 0 ] && break
      sleep 1
      in_waited=$((in_waited + 1))
    done
    for in_dir in "$state"/legs/*; do
      [ -f "$in_dir/wpid" ] && wait "$(cat "$in_dir/wpid")" 2>/dev/null
    done
  fi
  say "interrupted: every running leg ended; items keep their state"
  finish "$1"
}

# `spec auto next` with the running items busy and, by department, the
# busy and stopped lanes skipped; `$1` the free slots.
call_next() {
  set -- auto next --root="$root" --slots="$1"
  for cn_dir in "$state"/legs/* "$state"/pending/*; do
    [ -d "$cn_dir" ] || continue
    set -- "$@" --busy="$(cat "$cn_dir/item")"
  done
  for cn_item in $parked; do
    set -- "$@" --busy="$cn_item"
  done
  if [ "$by_department" = 1 ]; then
    cn_lanes=$(
      for cn_dir in "$state"/legs/* "$state"/pending/*; do
        [ -d "$cn_dir" ] && cat "$cn_dir/lane" && echo
      done
      stopped_lanes
    )
    while IFS= read -r cn_lane; do
      [ -n "$cn_lane" ] && set -- "$@" --skip-lane="$cn_lane"
    done <<EOF
$(printf '%s\n' "$cn_lanes" | sort -u)
EOF
  fi
  spec "$@" 2>"$state/next.err"
}

# `--dry-run`: the act `next` names and what it would run.
dry_run() {
  # No file is written: the error read again when it failed.
  dr_doc=$(spec auto next --root="$root" --slots="$lanes" 2>/dev/null)
  dr_code=$?
  if [ "$dr_code" -ne 0 ]; then
    dr_why=$(spec auto next --root="$root" --slots="$lanes" 2>&1 >/dev/null | head -n 1)
    say "spec auto next exited $dr_code: $dr_why"
    exit 1
  fi
  read_next "$dr_doc"
  case $n_action in
    stop)
      printf 'next: stop (%s): %s\n' "$n_reason" "$n_why"
      ;;
    wait | done)
      printf 'next: %s\n' "$n_action"
      ;;
    land)
      printf 'next: land %s\n' "$n_item"
      printf 'argv: spec auto land --root=%s --json -- %s\n' "$(quote "$root")" "$(quote "$n_task")"
      ;;
    approve)
      printf 'next: approve %s\n' "$n_item"
      if [ ! -d "$worktrees/$n_task" ]; then
        printf 'argv: git -C %s worktree add -b %s %s %s\n' "$(quote "$root")" \
          "$(quote "$n_branch")" "$(quote "$worktrees/$n_task")" "$(quote "$n_base")"
      fi
      dr_root=$worktrees/$n_task
      [ -n "$rel" ] && dr_root=$dr_root/$rel
      printf 'argv: spec auto approve --root=%s --json -- %s\n' "$(quote "$dr_root")" "$(quote "$n_task")"
      ;;
    implement | plan | plan-line)
      printf 'next: %s %s (lane %s)\n' "$n_action" "$n_item" "$n_lane"
      if [ "$n_action" = implement ] && [ ! -d "$worktrees/$n_task" ]; then
        printf 'argv: git -C %s worktree add -b %s %s %s\n' "$(quote "$root")" \
          "$(quote "$n_branch")" "$(quote "$worktrees/$n_task")" "$(quote "$n_base")"
      fi
      if [ "$n_action" = plan-line ]; then
        dr_subject=$n_text
      else
        dr_subject=$n_task
      fi
      printf 'cwd: %s\n' "$(quote "$(leg_dir "$n_action" "$n_task")")"
      printf 'env: CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS=%s\n' "$bg_wait_ms"
      if [ "$n_action" = plan-line ]; then
        printf 'env: SPECENGINE_AUTO_LINE=%s\n' "$(quote "$n_line")"
      fi
      dr_argv=
      while IFS= read -r dr_word; do
        dr_argv="$dr_argv $(quote "$dr_word")"
      done <<EOF
$(leg_argv "$(leg_prompt "$dr_subject")")
EOF
      printf 'argv:%s\n' "$dr_argv"
      ;;
    *)
      say "spec auto next gave no action: $(printf '%s' "$dr_doc" | head -n 1)"
      exit 1
      ;;
  esac
  exit 0
}

if [ "$dry_run" = 1 ]; then
  dry_run
fi

mkdir -p "$worktrees" "$log_dir" || usage_error "cannot make $worktrees or $log_dir"
log_file=$log_dir/autopilot.log
state=$log_dir/.state-$$
mkdir -p "$state/legs" "$state/pending" "$state/lanes" "$state/acts" ||
  usage_error "cannot make $state"
trap 'interrupted 130' INT
trap 'interrupted 143' TERM

say "start: root $root, worktrees $worktrees, log $log_dir, lanes $lanes"
pause_until=0
next_failures=0
implements=0
# Items `next` offered to implement past `--max-tasks`: busy from then on,
# so plans and lands go on.
parked=

while :; do
  reap

  if [ "$(now_epoch)" -lt "$pause_until" ]; then
    pw=$((pause_until - $(now_epoch)))
    [ "$pw" -gt "$poll" ] && pw=$poll
    nap "$pw"
    continue
  fi
  restart_pending

  # One lane without --by-department: its stop ends the run.
  if [ "$by_department" = 0 ] && [ -n "$(stopped_lanes)" ]; then
    wait_legs
    say "two failures in a row on lane all"
    finish 1
  fi

  running=$(legs_running)
  free=$((lanes - running))
  [ "$free" -lt 0 ] && free=0
  doc=$(call_next "$free")
  code=$?
  read_next "$doc"
  if [ "$code" -ne 0 ] || [ -z "$n_action" ]; then
    next_failures=$((next_failures + 1))
    why=$(head -n 1 "$state/next.err")
    say "spec auto next exited $code: $why"
    if [ "$next_failures" -ge 3 ]; then
      wait_legs
      say "spec auto next failed 3 times in a row"
      finish 1
    fi
    nap "$poll"
    continue
  fi
  next_failures=0

  case $n_action in
    stop)
      wait_legs
      say "stopped ($n_reason): $n_why"
      finish 0
      ;;
    land)
      do_land "$n_task"
      ;;
    approve)
      do_approve "$n_task" "$n_branch" "$n_base"
      ;;
    implement | plan | plan-line)
      if past_until; then
        if [ "$(legs_running)" -eq 0 ]; then
          say "until $until_time"
          finish 0
        fi
        nap "$poll"
      elif [ "$free" -eq 0 ]; then
        # `next` offers a leg only with a slot: none is started without one.
        nap "$poll"
      elif [ "$n_action" = implement ] && [ -n "$max_tasks" ] && [ "$implements" -ge "$max_tasks" ]; then
        act_line "$n_item" implement skipped "max-tasks $max_tasks reached"
        parked="$parked $n_item"
      elif [ "$n_action" = implement ] && ! ensure_worktree "$n_task" "$n_branch" "$n_base"; then
        hold "$n_task" agent-error \
          "the runner could not make the worktree $worktrees/$n_task: $(head -n 1 "$state/git.out")" implement
      else
        if [ "$n_action" = implement ]; then
          implements=$((implements + 1))
        fi
        # Without --by-department every leg is on the one lane `all`.
        leg_lane=all
        [ "$by_department" = 1 ] && leg_lane=$n_lane
        if [ "$n_action" = plan-line ]; then
          start_leg "$n_item" "$n_action" "$leg_lane" "" "$n_text" "$n_line"
        else
          start_leg "$n_item" "$n_action" "$leg_lane" "$n_task" "$n_task" ""
        fi
      fi
      ;;
    done)
      if [ "$(legs_running)" -eq 0 ]; then
        lost=$(stopped_lanes | sed '/^$/d' | tr '\n' ',' | sed 's/,$//; s/,/, /g')
        if [ -n "$lost" ]; then
          say "two failures in a row on lane $lost"
          finish 1
        fi
        if [ -n "$parked" ]; then
          say "max-tasks $max_tasks reached"
          finish 0
        fi
        say "done"
        finish 0
      fi
      nap "$poll"
      ;;
    *)
      # `wait`: nothing to start now.
      if [ "$(legs_running)" -eq 0 ]; then
        if [ -n "$parked" ]; then
          say "max-tasks $max_tasks reached"
          finish 0
        fi
        if past_until; then
          say "until $until_time"
          finish 0
        fi
      fi
      nap "$poll"
      ;;
  esac
done
