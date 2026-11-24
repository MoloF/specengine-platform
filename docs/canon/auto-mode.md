---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-http, plugin, ui]
owner: owner
reviewed: 2026-11-23
---

# Auto mode: a committed rule approves, applies and lands

Decided 2026-11-15, ADR-0081 (local) and ADR-0082 (hub), on the owner's word of that day (08 s2 Phase 7, "Owner's word, 2026-11-15"); working answers, his review open. Shipped that day by `auto-rule`, `hub-auto`, `auto-land`, `auto-runner`, `auto-feed` (RM-0120 to RM-0124, plugin 0.47.0); 0.46.1 and older refuse a file holding `[auto]` (`hub-ops.md` "Auto" orders the upgrade). With no `[auto]` table every output, route body, event and queue row stays 0.46.1's. Commands: `spec-auto.md`, `spec-auto-land.md`; the hub: `hub-auto.md`; the runner: `auto-runner.md`; the feed: `spec-feed.md`.

Delegated, nothing else: locally, approving the tasks of a pack the owner started with one terminal `[y/N]`, and landing each finished one (its code committed, its covered task-bound `update` and `create` applied, `spec check` clean on the merge, a `--no-ff` merge into the rule's branch); on a hub, applying the sole `owner` login's covered `update` and `create` raised over `/mcp`. Agents only report through today's tools; SpecEngine's code alone judges, a function of the rule, the queue and git. Never delegated: a deciding kind, an interpretation, an amendment, a `reference` (a file into git: ADR-0089), a drift item, a stage, `spec.lock`, tests or `verified` (ADR-0041, ADR-0044), a card, a push.

## Rule

```toml
[auto]
mode          = "both"             # off | local | hub | both; required
zones         = ["code", "tests"]  # [zones] keys covered; required, 1-32
exclude_zones = ["narrative"]      # [zones] keys never delegated; default []
exclude_kinds = ["scenario"]       # node kinds never delegated; default []
branch        = "auto"             # local: the branch a pack lands on; default "auto"
max_tasks     = 10                 # local: items per pack, 1-100; default 10
max_applies   = 20                 # hub: delegated applies per project per UTC day, 1-1000; default 20
```

Read apart, core `auto_toml.rs` `AutoRule::from_toml`, from the bytes of `<root_rel>/specengine.toml` at `HEAD` of the repository's main worktree (`git worktree list`'s first entry), whichever worktree runs the command; `[zones]` (`zones_from_toml`), `[ids]` and `[team]` from the same bytes. An uncommitted edit changes no verdict and adds one note, `specengine.toml differs from HEAD: auto reads HEAD's; commit it to change the rule`. `ProjectConfig` only name-checks `auto` (`RawFile`).

**Judging.** Every key whatever `mode`; the first problem by line turns the rule off with one note, `<path>@HEAD:<line>: <problem>; auto is off` (the switch's why `[auto] does not read: <path>@HEAD:<line>: <problem>`); no read fails. Problems: the TOML reader's (a wrong type too); `` `[auto]` is not a table ``; `` unknown key `<k>` in `[auto]` ``; `` `[auto]` is required to hold `mode` `` (`zones` alike); `` `mode` is `<v>`: one of off, local, hub, both ``; `` `zones` lists no zone ``; `a list holds at most 32 entries`; `` zone `<z>` is not declared in `[zones]` ``; `` zone `<z>` is listed twice `` (a kind alike); `` kind `<k>` is declared by no `[ids]` prefix ``; `` `branch` `<b>` is no branch name (letters, digits, `.`, `_`, `-`, `/`; 1 to 64 bytes) `` (also refused: a leading `-`, `.` or `/`, `..`, `//`, an ending `/`, `.` or `.lock`); `` `max_tasks` is <n>: 1 to 100 `` (`max_applies`: 1 to 1000); `[zones]`' or `[ids]`' own message.

**Kinds.** A node's kind is what its ID's prefix declares in `[ids]` (an alias resolved); an ID-less document has none. Unlike a direction's `kinds`, an undeclared one is a problem: a misspelled exclusion would delegate what it meant to keep.

## Rule hash

`b3:` + lowercase hex BLAKE3 of the canonical form: compact JSON, keys in this order, every array in byte order, strings as `serde_json` writes them:

```json
{"format":1,"mode":"both","zones":{"code":["src/**"],"tests":["tests/**"]},"exclude_zones":{"narrative":["docs/narrative/**"]},"exclude_kinds":{"scenario":["SC"]},"branch":"auto","max_tasks":10,"max_applies":20}
```

`zones`, `exclude_zones`: each named zone, by name, with the globs `[zones]` gives it; `exclude_kinds`: each kind with the `[ids]` prefixes declaring it. A department's practices stay a person's by their zone in `exclude_zones`, as the gamedev preset's comment shows (`card-practices.md`). Key order, comments, spacing and list order change nothing; a zone's glob or a kind's prefix changes the hash. Shown whole in `spec auto show`, each event and each trailer.

## Switches

Judged first, in this order; the first that holds makes every item a hold with its reason, and no item event is logged:
1. `off`: no `[auto]` table at `HEAD`, no commit, `mode` off, a mode not taking this surface (`local` on a hub, `hub` on a terminal), a rule that does not read, or (a pack, core `judge_pack_switches`) a `branch` changed since its start or a frozen rule that does not read.
2. `team`: `[team]` names two or more logins; on a hub also none, or one that is not `owner`.
3. `stopped`: the stop file `<data dir>/auto-stop` exists (`spec auto stop` makes it, `spec auto resume` removes it on a terminal `[y/N]`); it stops every project of that data directory.

`spec auto start`, `approve`, `land`, `hold` exit 1 `auto is off: <why>`, writing nothing; `show` reports; `next` answers `stop`.

## Judging an item

Pure, core `auto.rs`: the checks an act takes, in this table's order, the first failing one its reason (a closed set):

| Reason | Holds when |
|---|---|
| `not-in-pack` | locally, a task or its proposal in no live pack (a task listed at start or bound to a pack's line) |
| `author` | on a hub, a proposal not raised over `/mcp` by the sole `owner` login as an agent |
| `until` | the pack's `--until` passed: approve and implement hold (`spec auto approve` logs it); the pack stays live and its lands run |
| `status` | the task's state does not take the act; `run-outcome`: its last run is not `completed` |
| `proposal-kind` | not `update` or `create` |
| `staged` | a stage on the row: a person is deciding |
| `config-target` | a path named or changed is a `specengine.toml` (any depth or case) or `spec.lock` |
| `excluded-zone` | a path lies in an excluded zone |
| `excluded-kind` | a node named (the target, its document, a create's new ID; a task's targets and affected nodes) has an excluded kind |
| `uncovered` | a path lies in no covered zone (`Zone::covers`, the gate's glob test) |
| `findings` | the row's stored `diagnostics` hold any finding |
| `rebases` | the span hash now is not `base_hash`, or a file-form create's file exists |
| `cap` | on a hub, `max_applies` delegated applies already today (UTC) |

A pack judges against the effective rule, its start's frozen form and `HEAD`'s now: covered in both, excluded in either, the smaller cap. Removing or narrowing acts on the next judging; widening waits for the next start. A land adds `place`, `merge-conflict`, `check`, `branch-checked-out`, `branch-moved`; the runner's `spec auto hold` adds `agent-error`, `denied-tool`, `no-mcp`, `max-turns`, `max-budget`, `not-planned`, `not-done`: 29 in all.

**Details**, `<x>` filled (core `auto.rs`): `off` the why: `no [auto] table in <path>`, `no commit`, `no specengine.toml in HEAD`, `[auto] mode is off`, `[auto] mode is hub: a terminal acts only under local or both`, `[auto] mode is local: a hub acts only under hub or both`, `` the rule's branch changed since pack <n> started (`<a>` -> `<b>`) ``; `team` `[team] names <n> logins: auto stops at two`, a hub's `[team] names no login: a hub delegates only to its sole owner`, `` the sole [team] login `<l>` is a <role>: a hub delegates only to the owner ``, `` `[team]` does not read: <message> ``; `stopped` `the stop file <path> exists: spec auto resume removes it`; `not-in-pack` `` `<T>` is in no live pack ``, `` `<PR>` is bound to no live pack's task ``; `author` `` `<PR>` was not raised over /mcp by `<login>` ``; `until` `pack <n> ended at <until>`; `status` `` `<T>` is <status>: `<act>` needs <state> ``; `run-outcome` `` `<T>`'s run <n> ended <outcome>, not completed ``, `` `<T>` has no run ``; `proposal-kind` `a question waits for a person` (the kind, its article); `staged` `` `<PR>` has <an approve|a reject> staged at <at>: a person is deciding ``; `config-target` `` `<path>` is a project config or the lock: never delegated ``; `excluded-zone` `` `<path>` is in the excluded zone `<z>` ``; `excluded-kind` `` `<ID>` is a <kind>, an excluded kind `` (a land's changed document `` `<ID>` in `<path>` is … ``); `uncovered` `` `<path>` is in no zone the rule covers (<z1>, <z2>) ``, `` `<ID>` resolves to no file: uncovered ``; `findings` `<n> finding(s) stored at creation, the first <code> at <path>:<line>`; `rebases` `its span changed since its base: it would rebase`, `it conflicts with the span now`, `its target is gone`, `` `<path>` exists: a new file's create needs none ``, on a hub a refused run's refusal; `cap` `<n> delegated applies today (UTC), the cap max_applies <m>`. Under a pack, a check failing in one rule alone appends ` (the rule at pack <n>'s start)` or ` (the rule at HEAD)`; the first failing path, ID or zone is named, zones in the rule's order. A path outside the project root reads `../…`, in no zone.

**Lands** (`spec-auto-land.md`): one at a time, by a lock a dead holder's successor takes over atomically; a changed spec document's IDs judged as the merge brings them and as the merge base held them, so removing an excluded kind's content holds too; `spec check` on the merged tree against the branch's old tip, anything but `clean` held, new debt included; a proposal refused at a step, or changed since judged, is a note, never a stop. **Next** (`spec-auto.md`) skips a live pack whose own switches fail, offers a pack past its `until` only lands, and answers `wait` while any `--busy` item runs.

## Events

In the queue's `events`, payload JSON, keys in this order; no schema or backup format step, a dump keeps them as any event:
- `auto.started` `{repository, tasks, lines, until, branch, branch_at, rule, frozen, by}`: a pack, named by its `seq`;
- `auto.bound` `{pack, line, task}`: `spec task new` under `SPECENGINE_AUTO_LINE=<pack>/<n>`;
- `task.approved` by rule `{id, by: "rule", pack, rule}` (a terminal's stays `{id}`);
- `proposal.approved`, `proposal.applied` by rule: today's keys, then `by: "rule"`, `rule`, and `pack` locally;
- `auto.held` `{pack, task, line, proposal, act, reason, detail, rule}`, `act` one of `approve`, `apply`, `land`, `run`; a hub's `pack`, `task`, `line` `null`;
- `auto.landed` `{pack, task, branch, commit, code_commit, applied, left, rule}`.

## Trailers

A delegated apply's commit is `proposal-apply.md`'s, `Delegated-by: auto <rule hash>` last. A land's code commit, then its merge, author and committer the worktree's git identity:

```
auto: T-0042 <title, else the ID>

<the last run's summary>

Task: T-0042
Run: 1
Pack: 812
Delegated-by: auto b3:<64 hex>
```

```
auto: land T-0042 into auto

Task: T-0042
Pack: 812
Delegated-by: auto b3:<64 hex>
```

`git log --grep '^Delegated-by: auto'` lists every delegated commit; `git revert` (`-m 1` for a land's merge) undoes one.

## Runner

A process outside `spec`: the plugin's `scripts/autopilot.sh`, POSIX `sh` with `jq`, looping `spec auto next`, a worktree per task outside the repository, `claude -p "/specengine:feature <T>"` per leg, `spec auto approve` between the plan and implement legs, `spec auto land` after. `spec`, MCP and the daemon start no agent; the skill is unchanged; the runner never skips permissions (`auto-runner.md`).

## Switch-off

Any one: remove the table or set `mode = "off"` and commit (the next judging), a second `[team]` login (a team returns to manual), the stop file, or stopping the runner. In-flight tasks keep their state; nothing delegated is undone.
