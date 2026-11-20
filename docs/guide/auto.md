---
class: canon
tier: 2
scope: [guide]
owner: owner
reviewed: 2026-11-20
summarises: [docs/canon/auto-mode.md#rule, docs/canon/auto-mode.md#switches, docs/canon/auto-mode.md#judging-an-item, docs/canon/spec-auto.md#start, docs/canon/auto-runner.md#loop, docs/canon/hub-auto.md#rules, docs/canon/hub-ops.md#auto]
---

# The `[auto]` mode

Auto mode is a committed rule in `specengine.toml` that lets SpecEngine's own code approve and land the tasks of a pack the owner started, so agents can work overnight. The owner reads the result in the morning. This page covers the rule, packs, what is delegated, what never is, and how to stop it.

## What it solves

In manual mode the owner presses `y` for every task approval and every proposal. That is right for a team and too slow for one person running agents overnight. Auto mode moves those keys into a committed rule: one `y` starts a pack, and code judges each later step against the rule, the queue and git. Without an `[auto]` table, everything stays manual, exactly as before.

## When to use it

- **A project with one person deciding**: `[team]` names at most one login. A second login switches it off.
- **Work in well-bounded zones**, such as code and tests, where a clean `spec check` and the agent's own report are enough to land.
- **Overnight or unattended runs**, driven by the plugin's runner.

## How

### CLI

Check the rule, then start a pack on a terminal:

```text
$ spec auto show
$ spec auto start T-0041 T-0042 --lines ideas.txt --until 2026-11-21T07:00:00Z
auto pack: 2 task(s), 1 line(s); rule b3:<64 hex>; on auto (3f2a1c9d0b4e); until 2026-11-21T07:00:00Z
  T-0041 | review | Add the save slot list | apply
  T-0042 | draft | Fix the door timer | hold (excluded-zone): `docs/narrative/intro.md` is in the excluded zone `narrative`
  line 1 | Make the inventory sortable by weight
start this pack: its tasks approved and landed on `auto` by the rule, with no further key? [y/N] y
started pack 812: 2 task(s), 1 line(s) on auto
```

`--lines` gives one-line requests from which the agents create tasks. Then run the plugin's runner, `scripts/autopilot.sh --lanes 2 --until 2026-11-21T07:00:00Z`; `--by-department` runs one lane per department, and `--dry-run` shows the next step without running it. In the morning:

```text
$ spec feed --needs-me
```

`spec auto stop` halts every project of this machine at once; `spec auto resume` lifts it after a `y`. The runner itself calls `spec auto next`, `spec auto approve` and `spec auto land`; you rarely do.

### MCP

There is no auto tool. Agents work through the usual tools (`get_task`, `claim_task`, `submit_plan`, `report_run`, `complete_task`, `propose_change`); the rule judges what they produced. On a hub under `mode = "hub"` or `"both"`, a covered `update` or `create` the sole owner's agent raises with `propose_change` over `/mcp` is applied by the hub's code right after the call.

### Screen

Activity shows the same feed, newest change first. What waits for you, `spec feed --needs-me` groups as held items, open proposals, failed runs, observed gate writes and unlanded tasks.

## Where it lives

The rule is a table in the committed `specengine.toml`:

```toml
[zones]
code      = ["src/**", "Cargo.toml"]
tests     = ["tests/**"]
narrative = ["docs/narrative/**"]
practices = ["docs/spec/practices/**"]

[auto]
mode          = "local"                      # off | local | hub | both
zones         = ["code", "tests"]            # what may be delegated
exclude_zones = ["narrative", "practices"]   # never delegated
exclude_kinds = ["scene"]                    # kinds your [ids] declares, never delegated
branch        = "auto"                       # where a pack lands
max_tasks     = 10                           # items per pack
max_applies   = 20                           # a hub's delegated applies per day
```

Packs live in the queue. A pack lands on the rule's branch, `auto` by default, through a `--no-ff` merge with `spec check` clean; bringing that branch into your main line, and any push, stay yours. Each delegated commit ends `Delegated-by: auto <rule hash>`. The stop file is `auto-stop` in SpecEngine's data directory. The runner's worktrees sit outside the repository, beside it.

### The rule is what is committed {#GD-AUTO-COMMITTED-RULE}

Auto mode reads `[auto]` from `HEAD` of the main worktree, never from your working copy, so an uncommitted edit changes no verdict. A rule that does not read turns auto mode off with a note naming the line. Narrowing the rule acts at the next step; widening it waits for the next pack.

### Only the owner starts a pack {#GD-AUTO-OWNER-STARTS}

`spec auto start` asks once, on a terminal, and shows each task's verdict before you answer. Nothing else starts a pack: no agent, no tool, no hub page. The pack's tasks are then approved and landed by the rule, one by one, each judged anew.

### What is delegated {#GD-AUTO-DELEGATED}

Locally: approving a pack's tasks, and landing each finished one, which commits its code, applies its covered task-bound `update` and `create` proposals and merges it into the rule's branch. On a hub: applying the sole owner's covered `update` and `create` raised over `/mcp`, at most `max_applies` a day. A path must lie in a covered zone and in no excluded one, a node's kind must not be excluded, and the proposal must hold no check finding and still apply cleanly.

### What never is {#GD-AUTO-NEVER-DELEGATED}

Questions, discrepancies, decisions, interpretations, amendments, drift items, a staged choice, `spec.lock`, test evidence, cards, `specengine.toml` itself and any push. These wait for a person exactly as in manual mode. A held item says why, by one reason from a fixed list.

### Two people end auto mode {#GD-AUTO-SOLO}

When `[team]` names two or more logins, every item holds with the reason `team`. With a team, decisions are people's: return to manual mode.

## What not to do

### Never commit `[auto]` before every build reads it {#GD-AUTO-UPGRADE-FIRST}

An older `spec`, plugin or hub refuses a config holding `[auto]`. Upgrade the hub first, then every terminal's `spec` and plugin, and only then commit and push the table.

### Never cover the spec itself {#GD-AUTO-NARROW-ZONES}

Cover code and tests; exclude narrative, practices and anything a department agreed. A delegated apply has no person reading it before it lands. To undo one, `git revert` its commit; the proposal stays applied in the queue.

### Never loosen the runner's permissions {#GD-AUTO-NO-BYPASS}

The runner never passes `--dangerously-skip-permissions` and refuses free arguments to Claude Code; the main checkout's `.claude/settings.json` decides what an agent may run. A denied tool holds the item with the reason `denied-tool`; widen the project's allow rules on purpose, not the runner.
