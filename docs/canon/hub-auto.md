---
class: canon
tier: 2
scope: [crates/specengine-http, crates/specengine-cli, crates/specengine-mcp, ui]
owner: owner
reviewed: 2026-11-15
---

# Hub auto: the hub's code applies its sole owner's covered proposals

ADR-0082, narrowing ADR-0061 (`hub-confirm.md`) on a hub alone; decided 2026-11-15, shipped that day by `hub-auto` (RM-0121, plugin 0.47.0), working answers, the owner's review open. The rule, its hash, switches, reasons and events: `auto-mode.md`. With no `[auto]` table a hub's answers, events, rows and stderr stay 0.46.1's, but each `update` or `create` stored over `/mcp` takes one more turn hold, the rule's read (about six git calls, 100-300 ms), which a low `--slow-ms` may print; a mode taking no hub logs its `off` line. Code: http `src/auto.rs`, `mcp.rs` (`HubSeam::proposed`); CLI `auto_apply.rs`; mcp `hub.rs`, `review.rs` (`mcp_review`).

<a id="rules"></a>
## Rules

- **What.** An `update` or `create` stored by `propose_change` over `/mcp/<slug>` (no dry run, an `id` given) is judged once, after its call answers; a covered one is applied, nothing else is: questions, discrepancies, decisions, interpretations, amendments, a drift item, a stage, a rejection, a card's `items` raise and every page act wait for a person as `hub-confirm.md` says. A hold is never judged again: Confirm applies it. The rest raise no `auto.held`: the seam never hears them.
- **Who.** `[team]` names exactly one login, role `owner`, and the proposal's `author` is `{type: agent, login: <that login>}` (`hub-mcp.md` "Identity"); else the switch `team` or the reason `author`.
- **When, where.** mcp `hub.rs`, once the propose's answer is built, calls `HubSeam::proposed(login, id)`; the daemon queues one command in that login's FIFO line (`hub-mcp.md` "Turn"), so it runs in the project's turn after the propose and before that login's next call, and a page request still waits for at most one call per login. A failed propose queues nothing; one cancelled after its turn started may still store its row, never judged (fail-safe: it stays open). A daemon stopping drops a queued command, the row open; a `SIGTERM` during a running apply can leave it `approved` with a dirty checkout, as a confirm's would.
- **Read per command.** Inside the turn the rule, `[zones]`, `[ids]` and `[team]` are read from the bytes of the checkout's `HEAD`, as `[team]` is read per request with no cache; a commit synced in, or the stop file, acts on the next proposal with no restart.
- **One call.** `auto_apply(&Env, &Globals, &AutoApplyRequest {id, login, now, git}) -> AutoApplyOutcome {Off {reason, why, absent}, Gone, Held {reason, detail, ran}, Applied {commit: Option}, Moved}` judges the switches (`Off`, no event) and the row read once (not `open`: `Gone`; `auto-mode.md` "Judging an item", hub surface, `cap` last), then calls `delegated_approve` (it alone does), which runs `spec approve`'s steps as a confirm does: the row compared with the one judged (`expected`) before the rehearsal and before the run, a rehearsal returning the question, the run's one consent `asked == question`, byte for byte; a row moved meanwhile is `Moved`, nothing written. A run refusing at a step is held `rebases`, the refusal its detail, logged once beside its `proposal.apply_failed`. `Decided-by`, author and committer are the login and its `[team]` `email`, as a confirm's (`hub-confirm.md` "Identity"); the commit ends `Delegated-by: auto <rule hash>`; `proposal.approved` and `proposal.applied` carry `by: "rule"` and `rule`. A hold logs one `auto.held` (`act: "apply"`), a switch none. A `--push` sync pushes the commit (`hub-git.md`).
- **The door.** `tests/door.rs` keeps `approve`, `approve_with`, `reject` caught in every http `src` file; `HUB_AUTO_CALLS` (`auto_apply`) are caught in any file but `src/auto.rs`, which holds no `|_|` closure and no `use ... as`; `delegated_approve` is caught in every file; the always-yes scan also reads the CLI's `auto_apply.rs`. Of the `propose*` names it admits exactly `proposed`, the seam's method, and the CLI library defines no `fn proposed`.
- **A local daemon** never applies: `HubSeam` exists on a hub alone; stdio MCP has none.

## Lines

One stderr line per command, controls escaped, none when `HEAD` holds no `[auto]` table (`EffectiveRule::absent`); every other `off` is printed:

```text
specengine-http: auto <slug> <id> by <login>: applied <sha12>
specengine-http: auto <slug> <id> by <login>: held (<reason>): <detail>
specengine-http: auto <slug> <id> by <login>: off (<reason>): <why>
specengine-http: auto <slug> <id> by <login>: <gone|moved>
specengine-http: auto <slug> <id> by <login>: cannot run: <message>
```

An applied document naming no commit prints `cannot run: applied, but its document names no commit`. The command's turn hold writes the slow lines `slow turn … by=<login> id=<PR> tool=auto_apply state=held|released … class=mcp` (`serve-slow.md` "Lines"). An apply signals a `--push` sync. Only a run that may have written the tree doubts the project's index (`spec-read-freshness.md`): one refused at a step, one naming no commit, one that cannot run or panics; a predicate's hold, which writes one event, leaves it trusted.

## Stop file

`<data dir>/auto-stop`: on the hub image `/data/specengine/auto-stop` (`XDG_DATA_HOME=/data`). Its presence holds every project of the hub (`stopped`), read per command; `spec auto stop` writes it, `spec auto resume` removes it after a terminal `[y/N]` (`docker compose exec -it specengine spec auto resume`).

## Cap

`max_applies` delegated applies per project per UTC calendar day: the count of that project's `proposal.applied` events with `by: "rule"` whose `at` shares `now`'s date (store `rule_applies_on`, in SQL). At the cap the row is held `cap` and waits for Confirm; the next day starts at 0.

## Screen

A delegated apply's review document (`spec review --json`, the daemon's route) carries `delegated` `{by: "rule", rule, commit}` after `task_id`, its key skipped on every other row (as `Author.login`), read in one query (store `applied_payload`); the text form `delegated: by rule <hash>, commit <sha12>`. MCP's `get_proposal` and `review_proposal`, stdio and hub, leave it and its line out (`mcp_review`, the context cost): `tools/list` is 0.46.1's. A decided proposal opened by link (`proposal-review.md` "Inbox") shows `Applied by rule` with the commit and the rule hash, read-only; the Inbox lists only open rows, so a held one keeps its Confirm. The feed (`spec-feed.md`) lists `proposal.applied` by rule and each `auto.held` with its reason; Activity's head says `Auto is off: <why>` when off.

## Known limits

- The `/mcp` secret applies within the cover past the step-up (`hub.md` "Trust", item 5): ADR-0082's cost; rotate it from the host (`hub-deploy.md#accounts`).
- No integration branch on a hub: a `--push` sync carries a delegated commit to the remote within its interval; the cap, the stop file and `git revert` bound it (`hub-ops.md` "Auto").
- Judged once: a row held `rebases` or `findings` stays held after the cause is gone.
- `staged` and a stale base's `rebases` are tested at unit and predicate level alone, not over a served hub (HA2).
- A forged `Remote-User` as the owner is applied within the cover (`hub-mcp.md` "Known limits"), as it would be confirmed.
