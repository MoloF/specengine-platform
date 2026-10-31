---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, plugin]
ref: RM-0068; Phase 7 step 7c (08 s2)
---

# flow-authoring: a `write-flow` skill, gap rows that say how, errors that teach, a commented `spec init` (RM-0068)

## Why

In the trial of 2026-10-31 a fresh AI wrote a whole flow with no help, yet every piece of grammar came from validator errors and none from a source: about 30 discovery calls against 13 work calls, 70 %, 17 of them deliberate probes. No skill, description or README says how to author a flow. `cycle-no-repeat` gave no remedy, `unknown-link-type` listed nothing (24 wrong names), `key-missing` named no values, and `[ids] probe = 1` answered with a Rust type name.

`proposal-rehearsal` ships first, since the skill names `dry_run`.

## Description and interactions

**The skill.** A model-invoked skill `write-flow`: when to use it, a five-step path (configure and have the owner commit, rehearse with `dry_run`, propose per document, the owner's `spec approve --open`, read `get_flow`), the link grammar in short, and one complete small example as `json` blocks of `propose_change`. It says a plain `follows` fork declares nothing, what `supplies` feeds, that `[[flow.step]]` is optional and that `paths` needs no key of the project's own. `get_flow`'s description points to it.

**Gap rows.** A row above 0 and a `not_configured` row carry one line in `cause`, why it fired and how to clear it, in text, JSON and the Flow screen. A row at 0 keeps `null`. No key is added: `FlowGaps.tsx` already draws any non-null `cause`.

**Errors.** Five messages say what they want. Nothing newly refuses (ADR-0012).

**`spec init`** appends comments after the two lines it writes today. The parsed config is unchanged.

## Data

**The config block** of the example, verified with the installed `spec` on an empty git project:

```toml
[ids]
NODE = { kind = "node", shape = "name" }
[[flow.start]]
paths = ["docs/spec/open.md"]
[[flow.end]]
paths = ["docs/spec/done.md"]
[[flow.definition]]
paths = ["docs/spec/rule.md"]
```

**The example,** eight `create` blocks, one file each, the fewest that read all ten rows `measured` 0: `definition-*` need a definition with two users, `unjoined-parallel` two branches that meet. Each text holds `id`, `class: canon`, `owner: owner`, `reviewed: 2026-01-01`, then the keys below, then its H1 alone. Under `docs/spec/`:

| File | `parent:` | `stage:` | `links:` |
|---|---|---|---|
| `root.md` | - | - | - |
| `rule.md` | `NODE-root` | - | - |
| `open.md` | `NODE-root` | `a` | - |
| `left.md`, `right.md` | `NODE-root` | `a` | `follows: [{to: NODE-open, kind: parallel}]`, `uses: [NODE-rule]` |
| `join.md` | `NODE-root` | `b` | `follows: [NODE-left, NODE-right, {to: NODE-lost, kind: repeat, when: retried}]`, `supplies: [NODE-done]` |
| `done.md` | `NODE-root` | `b` | `follows: [{to: NODE-join, kind: alternative, when: won}]` |
| `lost.md` | `NODE-root` | `b` | `follows: [{to: NODE-join, kind: alternative, when: lost}]` |

That corpus reads ten rows at 0 and `spec check` gives 0 errors, 0 warnings.

**`cause`** (D11). Core: `measured()` sets `GapCause::Found(code)` above 0, CLI `cause_text` words it. `NoRule` keeps its prefix `` no [[flow.<table>]] rule: ``. `NoStart` and `Config` stay.

| Code | The line |
|---|---|
| `no-start` | no step here is a start: match the first step in [[flow.start]], or add a follows link into this scope |
| `two-starts` | more than one start: narrow [[flow.start]] to one |
| `unreachable` | no path from a start reaches it: add a follows link on it, naming a step before it |
| `no-way-to-end` | no path from it reaches an end: have a later step follow it, or match it in [[flow.end]] |
| `trap-cycle` | a loop with no way out: have a step outside it follow a member, on the way to an end |
| `unjoined-parallel` | a parallel fork with one branch, or branches that never meet: add a branch, or a later step following them all |
| `cycle-no-repeat` | a loop with no repeat edge: write the edge closing it as {to: ID, kind: repeat} |
| `definition-unused` | no document uses it: add a uses link to it, or drop it from [[flow.definition]] |
| `definition-once` | only one document uses it: add a second uses link, or fold it into that document |
| `supply-unfed` | these only supply each other: add a supplies link into them from a node outside |
| absent `start` | no [[flow.start]] rule: add one naming the first step by paths |
| absent `end` | no [[flow.end]] rule: add one naming the last step by paths |
| absent `definition` | no [[flow.definition]] rule: add one naming what steps share, by paths or kinds |

**Messages.**

| Site | Change |
|---|---|
| `unknown-link-type`, `front_matter.rs:627` | ends `; kept; the types: <LINK_TYPES joined by ", ">`, from the const |
| `key-missing`, `engine.rs:484` | class spec, `status`: `: one of draft \| in-progress \| shipped \| abandoned`, from `SPEC_STATUS`; `scope`: `: a list, as [root]`; `reviewed`: `: a date, YYYY-MM-DD`; any other key as today |
| `RawPrefix`, `scheme_toml.rs:135` | ``#[serde(expecting = "a prefix table: `kind`, then `shape` or `width`")]``, as `check/config.rs:1082` |
| `no_selector`, `health_toml.rs:336` | adds ``; `when` is a table of front-matter key = "value"``; shared by `[[health.bound]]` and `[flow]` |
| `id-not-in-scheme`, `front_matter.rs:401` | adds `; add to [ids]: <PREFIX> = { kind = "<kind>", shape = "name" }`, `<PREFIX>` the written ID's part before its first `-`; an ID with no `-` keeps today's text |

**`spec init`** writes, the first two lines as today, the path formatted from `DEFAULT_SPEC` (core `paths_toml.rs:35`), since `genre.rs` bans a `docs/` literal and `STEP-`:

```toml
[project]
slug = "<slug>"

# Commit this file: a proposal needs a commit, and the project's rules live here.
# [ids]
# NODE = { kind = "node", shape = "name" }
# [[flow.start]]
# paths = ["docs/spec/first.md"]
# [[flow.end]]
# paths = ["docs/spec/last.md"]
```

**Budgets,** measured 2026-10-31 against `tools/list` and the files:

| Budget | Now | This slice | Free after |
|---|---|---|---|
| `INSTRUCTIONS`, 2 048 B | 2 048 | +0 | 0 |
| `get_flow` description, 2 048 B | 2 026 | +19, ` Skill: write-flow.` | 3 |
| model-invoked descriptions, 1 200 B | 1 170 | +30, `Write a flow of spec documents` | 0 |
| `write-flow` body, 4 096 B | - | 3 817 (scratch draft) | 279 |
| `propose-spec-change` body, 4 096 B | 4 092 | +0, untouched | 4 |

**What does not fit, and the cut.** A first full draft ran to 4 257 B, 161 B over: the blocks alone took 2 610 B, nearly the 2.7 KB estimated for the whole. Three cuts reached 3 817 B with both checks above still clean: no `base` key on a block (a new file needs none, one prose line says so), an H1 with no body text, and shorter prose. Next cut if the text grows: the `when` labels on the alternatives. Descriptions end at exactly 1 200 B, so the next skill must cut another's.

**Skill text, what the tests hold.** `plugin_skills.rs` AC-08 reads digits and enum values outside `json` fences only, so the `toml` fence holds no digit, and `` `links` `` and `` `create` `` are never backticked outside a block: both are input enum values today. AC-07 needs each block's line above it to name exactly one tool. The precedence sentence stands in the skill.

**Formats.** `INDEX_FORMAT` 14 -> 15: parser messages are stored rows, and an unchanged file re-parses only on a new stamp (`store/lib.rs:185`, `flow-links.md` "Index"). A line `15 <hash>` in `tests/format_history.txt`; `bundle_config.rs:309` moves. No queue step, no key change, no `SCHEMA_PINS` entry. Plugin 0.28.0 (MINOR: a skill added), `PINS`.

## Rules and edge cases

- WHEN a row reads `measured` above 0, `cause` SHALL be one line, and at 0 `null`. WHEN `not_configured`, it SHALL keep its prefix.
- A key of the project's own in `when` still draws `unknown-key`, by design (`front_matter.rs:292`, listed as not done in `spec-check-process.md` "Open"): the skill and the scaffold teach `paths`, which needs none.
- A `specengine.toml` left untracked gets one scaffold comment and one skill line, no check.

**Decision D11.** Remedies stay in `cause`, written as why plus how, so no key, schema or UI change is needed. `FlowGap.cause`'s doc and the pins of `null` above 0 are reworded.

No ADR: ADR-0027 admits a stack-neutral skill, and the closed file set and the skill count are canon, not rules.

## Acceptance criteria

- [ ] AC-01 — A test, so the example cannot rot, takes the skill's config and `json` blocks from the file into an empty git project, commits, and approves each through the library with a scripted consent. `spec flow` reads ten rows `measured` 0 and `spec check` gives 0 errors, 0 warnings. M: `repeat` dropped from the example.
- [ ] AC-02 — On `fixtures/flow-gaps` each row above 0 has a one-line `cause` in text and JSON, and a row at 0 has `null`; a stubbed row with a `cause` draws it in `FlowGaps`. Every `not_configured` cause keeps its prefix. M: a row left blank; a cause on a row at 0. `crates/specengine-cli/tests/flow_read.rs:252` and core `tests/flow_read.rs:360` change.
- [ ] AC-03 — An unknown link type's message lists the 15 types; a missing `status` shows `draft | in-progress | shipped | abandoned`; `[ids] probe = 1` names no Rust type; `no_selector` says what `when` holds; `id: TOPIC-one` under no such prefix shows a `TOPIC = ` line. M: a literal list instead of `LINK_TYPES`.
- [ ] AC-04 — `spec init`: a rerun exits 2; the parsed config equals the bare one; the example with its `# ` removed parses; `genre.rs` passes. M: a `docs/` literal; an active line.
- [ ] AC-05 — Seven skills, `plugin.json` 0.28.0, `INSTRUCTIONS` pins unchanged, every body within 4 096 B, descriptions at most 1 200 B. M: a 31-byte description.

## Out of scope

`--pack` and config proposals: RM-0061. Writers' prompts over the hub: RM-0062. `spec check` validating `[flow]`: "never a finding or a gate", and `get_flow` already answers `cannot` with config, line and message. An eleventh row for a plain fork (`spec-flow.md` "Gap rows"). A typed key for `when`. Defining the ten rows in `get_flow`'s description: 22 B are free, and the rows carry it.

## Implementation

To fill in after implementation.

### Files in scope

New: `plugin/specengine/skills/write-flow/SKILL.md`. Core `flow.rs`, `front_matter.rs`, `check/engine.rs`, `scheme_toml.rs`, `health_toml.rs`. CLI `flow.rs`, `init.rs`. Store `lib.rs`, `tests/format_history.txt`. MCP `read.rs`. Tests that move: `plugin_files.rs` `FILES` 17, `plugin_skills.rs` `MODEL_INVOKED` 6, `crates/specengine-cli/tests/init.rs:13`, `flow_read.rs` `NO_START_CAUSE`.

The 1 200 B limit is `plugin_files.rs:1292`; nothing pins 1 170.

### Canon at ship

- **`plugin-files.md`** (6.2 KB free): sixteen -> seventeen, five -> six model-invoked, "1 170 at 0.6.0" -> 1 200, 0.28.0. **"Content" needs one exception.** It says a skill holds no path or command, and this skill must: the config's `paths`, the blocks' `target`, `spec approve --open`. No test reads that clause, so it would pass silently. The rule's point is that no project's fact leaks, and a made-up example leaks none, so the sentence gains: `write-flow` alone carries a made-up example and names the owner's command.
- **`plugin.md`** (1 KB free): six -> seven skills, a `write-flow` row. Root `README.md` line 41 follows.
- **`spec-flow.md`** (1.4 KB free) "Gap rows", "Output": a row above 0 carries its remedy.
- **`spec-flow-screen.md`**, 165 B free, shared with `screen-fit`: "A row:" adds `<n> found` with its `cause`, about 20 B; the shared cut is named in `screen-fit.md`.
- **Tier 1.** `crates/specengine-cli/README.md` has 3 B free and its `spec init` line must change: reword that line with no growth, cutting `prints ...` if needed. `crates/specengine-store/README.md` line 22: `14` -> `15`, no growth. Core stays as it is (1 B free).
