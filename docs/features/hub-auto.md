---
class: spec
status: shipped
shipped: 2026-11-15
scope: [crates/specengine-http, crates/specengine-cli, crates/specengine-mcp]
ref: RM-0121; the owner's word of 2026-11-15 ("update the hub with the new auto setting so that everything works"); release auto-mode, plugin 0.47.0
adrs: [ADR-0082]
---

# hub-auto: a hub applies its sole owner's covered proposals (RM-0121)

## Why

On his hub the owner's agent raises proposals over `/mcp`, and each takes a click plus a second factor that judges nothing he has not already ruled in `[auto]`. ADR-0082 lets the hub's own code apply the covered `update` and `create` of the hub's sole `owner` login and leaves everything else to a click, so a writer joining (a second `[team]` login) brings manual mode back by itself.

## Description and interactions

Canon: `docs/canon/hub-auto.md` (rules, the door, the lines, the stop file, the cap, the screen, known limits) and `docs/canon/auto-mode.md`. Who calls what: mcp `hub.rs` calls `HubSeam::proposed(login, id)` once a stored `update` or `create` has its answer; http `src/mcp.rs` hands it to `src/auto.rs`, which queues one command in that login's line and calls the CLI's `auto_apply`, the only caller of `delegated_approve`. The review document's `delegated` is served by `spec review --json` and the daemon's route; MCP's `get_proposal` and `review_proposal` leave it out (the context-cost rule). The page's half is `auto-feed`'s.

## Data

`AutoApplyRequest {id, login, now, git}`; `AutoApplyOutcome {Off {reason, why, absent}, Gone, Held {reason, detail, ran}, Applied {commit: Option}, Moved}`; the five stderr forms: `hub-auto.md` "Lines". `delegated` `{"by":"rule","rule":"b3:…","commit":"<sha>"}` after `task_id`; its text line `delegated: by rule <hash>, commit <sha12>`.

## Rules and edge cases

As `hub-auto.md` "Rules" and "Known limits": judged once, inside the login's line, the rule read per command from the checkout's `HEAD`; a refused run held `rebases`; a cancelled propose may leave its row stored and unjudged, open; a daemon stopping drops a queued command.

## Acceptance criteria

http `tests/hub_auto.rs` over a scratch `fixtures/auto-mini` served as a hub; `tests/door.rs`; units in `src/auto.rs`; CLI `tests/auto_apply.rs`; mcp `tests/mcp_hub.rs`; `tests/daemon_keys.rs`.

- [x] HA1 — **A covered update applies with no click** (`ha1_a_covered_update_and_create_apply_with_no_click`, `ha1_ha2_the_daemons_lines`): an `update` of R-02 and a `create` by `ann` applied, author and committer `ann <ann@example.org>`, `Delegated-by:` last, events `by: "rule"`, the route's `delegated`. Mutation: `proposed` never called.
- [x] HA2 — **Each hold writes nothing but its event** (`ha2_each_hold_writes_nothing_but_its_event`, `ha2_ha5_the_cap_and_the_switches_act_on_the_next_proposal`, `ha2_a_refused_run_is_held_rebases_once`, `ha2_a_hold_leaves_the_index_trusted`): `excluded-zone`, `excluded-kind`, `uncovered`, `findings`, `cap`, `author`, `team` and `stopped` (no event) over a served hub; a refused run held `rebases` once. Deviation: questions, decisions, amendments and interpretations raised over `/mcp` get no `auto.held` (the seam never hears them: they stay open, as the rule wants); `staged` and a stale base's `rebases` are covered at unit and predicate level only (`auto_predicate.rs`). Mutation: any one predicate dropped.
- [x] HA3 — **A local daemon applies nothing** (`ha3_a_local_daemon_applies_nothing`). Mutation: `auto_apply` reachable locally.
- [x] HA4 — **One door** (`door.rs` `hub_auto_ha4_the_scan_knows_the_auto_file`, `hub_auto_ha4_the_daemon_applies_from_one_file_with_no_closure`, `hub_auto_ha4_proposed_is_no_cli_call`): positive controls each; `proposed` the one admitted `propose*` name. Mutations: an `approve` in `app.rs`; `auto_apply` from `mcp.rs`; the CLI's `auto_apply.rs` answering `|_| true`; a CLI `fn proposed`.
- [x] HA5 — **Off with no restart** (`ha2_ha5_the_cap_and_the_switches_act_on_the_next_proposal`): the table removed and the stop file act on the next proposal, restored the next applies. Mutation: the rule read once at `App::new`.
- [x] HA6 — **The page stays served** (`ha6_the_page_stays_served_during_a_burst_of_applies`, `#[ignore]`d, the `probes` build, as TF1-TF8). Mutation: the command outside the login's line.
- [x] HA7 — **The review document** (`ha7_the_review_document_marks_the_delegated_row_alone`; `daemon_keys.rs` `Delegated` its own set): `spec review --json` and `GET .../proposals/<id>` hold `delegated` on a delegated row alone. Deviation: `get_proposal` does not serve it. Mutation: the key `null` on every row.
- [x] HA8 — **The seam stays one call** (`mcp_hub.rs` `hub_auto_ha8_proposed_is_called_once_per_stored_update_or_create`). Mutation: called for `ask_question`.
- [ ] HA9 — **[owner, live hub, gate G3]** After the upgrade (`hub-ops.md` "Auto"): the owner's agent raises a covered update over `/mcp`; it is applied and pushed by the next sync with no click; a proposal on the `narrative` zone waits for Confirm with the step-up; `git log --grep '^Delegated-by: auto'` on the remote lists the first.

## Out of scope

A `T-` flow on a hub; a veto window or a scheduler; judging a held row again; delegating any kind but `update` and `create`; a per-login rule; the feed and the page (S5).

## Implementation

| Module | What it does |
|---|---|
| http `src/auto.rs` (new) | `queue` (one poll joins the login's FIFO before the answer leaves), `run` in the `walking` scope, the stderr `line`, the sync's signal, doubting the index only after a run that may have written |
| http `src/mcp.rs`, `turn.rs`, `main.rs`, `app.rs` | `HubSeam::proposed` -> `auto::queue`; `Holder::delegated` (`by=<login>`, `tool=auto_apply`, class `mcp`) |
| mcp `src/hub.rs`, `review.rs`, `intake.rs` | `proposed` called once per stored `update` or `create`; `mcp_review` drops `delegated` from `get_proposal` and `review_proposal` |
| CLI `auto_apply.rs`, `review.rs`, `proposals.rs`, `lib.rs` | `auto_apply`, `AutoApplyRequest`, `AutoApplyOutcome` (`summary`, `doubts`); `delegated` on the review document and its text line |
| store `queue/auto.rs` | `applied_payload`, `rule_applies_on` |
| tests | the files above; `fixtures/daemon-keys.json` `Delegated` |

**Decisions** (the orchestrator's, the owner's to revise): only `auto_apply` calls `delegated_approve`, and `src/auto.rs` alone names `auto_apply`; `door.rs` admits exactly `proposed`; MCP reviews drop `delegated`, so `tools/list` and `SCHEMA_PINS` stay 0.46.1's; no stderr line without a table (`EffectiveRule::absent`), every other `off` printed; `cannot run: …` a fifth form; the login printed on the slow turn line when it matches `[A-Za-z0-9._-]{1,64}`, else `?` (`serve-slow.md` "Hygiene" amended).

**Divergences.** A hub without `[auto]` takes one more turn hold per stored update or create (about six git calls, 100-300 ms), which may print slow-turn lines at a low `--slow-ms`: accepted. A propose cancelled after its turn started still stores its row, never judged (fail-safe), against this spec's "nothing is queued, it stored nothing". A `SIGTERM` during a running apply can leave the row `approved` with a dirty checkout, as a confirm's would.
