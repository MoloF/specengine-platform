---
class: spec
status: draft
scope: [crates/specengine-cli, crates/specengine-mcp, plugin]
ref: RM-0067; Phase 7 step 7b (08 s2)
---

# proposal-rehearsal: a dry run, no false `ref-dangling`, one sitting of approvals (RM-0067)

## Why

The queue is the only validator, so learning spends the owner's attention. In the trial of 2026-10-31, 6 of 29 proposals were rejected probes, each one an owner act, and a bad create held its ID until the owner rejected it. A reference to a proposal still open read as an error, so documents had to be approved one at a time while they were being written. The 23 approvals were 23 separate commands.

This slice lets an agent rehearse with nothing stored, stops a false finding, and lets the owner walk the open proposals in one command, one key each. It ships before `flow-authoring`, whose skill names `dry_run`: a backticked property must exist (`plugin_skills.rs` AC-06).

## Description and interactions

**Dry run.** `propose_change {dry_run: true}` and `spec propose update|create --dry-run` run every step of the real call, every refusal and every finding, and stop before storing. Nothing is written to the queue and no ID is held, so a real call with the same ID stores. An agent sends the first document of a kind this way and reads `diagnostics`.

**No false `ref-dangling`.** A `ref-dangling` on an ID a live create or amendment holds (`reserved()`) is left out of the stored findings, and a note names the holder. Findings never refused anything (ADR-0012), so this changes what is read, not what passes. A reference to nothing keeps its finding.

**The walk.** `spec approve --open` goes through this repository's open proposals of applying kinds, oldest first, and for each runs the ordinary approve: its own question, its own `[y/N]`, its own commit with its `Proposal:` trailer. A decline leaves it open and the walk moves on.

## Data

```text
spec propose update ID --base HASH --text-file F|- --rationale T [--dry-run] ...
spec propose create TARGET [--base HASH] --text-file F|- --rationale T [--dry-run] ...
spec approve --open
```

**`propose_change`** gains one optional input property; no other tool changes. Schema text: `true: every refusal and the diagnostics of the real call, nothing stored; update and create only.`

```json
{"kind": "create", "target": "docs/spec/open.md", "text": "...", "rationale": "...", "author_role": "...", "dry_run": true}
```

**The answer** is the real call's document, `ReviewDocument`: `id` and `status` `null` (both are nullable today, so the output schema is unchanged), `kind`, `target_id`, `target_path`, `diagnostics` as a real call stores them, `notes` holding `dry run: nothing stored` first. What only a stored row or its worktree preview can fill (`base_commit`, `preview`, `revisions`, `created_at`, ...) is `null`. Exit 0 with or without findings; a refusal exits 1 with the real reason, as `ProposalOutcome::refused` does.

**The filter.** One function over the findings and `queue.reserved()`, called from both validation sites: `validated` (`create.rs:636`) and the update's step 4 (`propose.rs:826`), the latter after `checked_update` returns, since it holds no queue. A finding is dropped when `code` is `ref-dangling` and `subject`, cut at `#` and made canonical, is a `Reservation.id`. The check gives `ref-dangling` for `parent:` and `links` alike (`engine.rs:873`), and `subject` is the reference as written. One note per holder, built on `reserved_by`: `` `NODE-open` is reserved by `PR-0002` (open), a live create: no ref-dangling stored ``.

**The walk's output.** Text: each item's own approve output in turn, then `open walk: <a> applied, <k> left open`, then the notes. JSON, one document at the end: `{applied: [PR], left: [PR], notes: [...]}`. Exit 0 when nothing stays open, 1 when any does. No open proposal: `open walk: 0 applied, 0 left open`, exit 0.

**Notes,** in `spec round new`'s words (`round.md`):

- `` <n> open deciding item(s) not walked: <IDs>; spec round new lists them ``;
- `` <n> open drift item(s) not walked: <IDs>; spec review <ID>, then spec approve or spec reject ``.

**Schema.** No queue step; `INDEX_FORMAT` stays. Input schema +1, plugin 0.27.0 (MINOR: a key a tool takes), `SCHEMA_PINS` entry. The number assumes `trace-truth` shipped first: slices 7a to 7c each pin the plugin, so they ship in sequence and each takes the next MINOR. `propose_change`'s description has 19 B free of 2 048 and stays as it is; the property's text carries the rule.

## Rules and edge cases

- WHEN `dry_run` is true with kind `interpretation` or `amendment`, `propose_change` SHALL return a form error naming the field, as its other form errors do, and store nothing. The CLI has no such flag on those commands: a clap usage error, exit 2.
- WHEN a dry run names a taken ID, a stale base or a `class: generated` document, it SHALL give the real reason, exit 1. `--task` is checked once the place is bound, as in the real call.
- WHEN `spec approve --open` runs with a stdin that is no terminal, it SHALL exit 2 before anything is read, by `require_terminal`. No `--yes`.
- WHEN `--open` stands beside a `PR` or any of `--note`, `--option`, `--answer`, `--canon`, `--editorial`, it SHALL exit 2 as a usage error. An item needing one takes its own `spec approve`.
- The walk lists `open` rows once, at its start, by ID number, the queue's order. An `approved` row is not walked: `spec approve <ID>` completes it, as today. A staged item shows its stage as `spec approve` does (`decision-staging.md` "Terminal").
- WHEN one item's approve is refused (a stale base, a dirty place, a conflict), it SHALL print that refusal, count as left open and move on. WHEN approve fails with an exit-2 error (a queue that cannot be read, no identity), the walk SHALL stop there with exit 2: each earlier commit stands.
- WHEN a holder is rejected after its dependant applied, the `ref-dangling` returns: `spec check` reports it. A project's hook at commit may refuse an out-of-order approve; the walk's order avoids this.
- A dry run is no consent and takes no ID: two agents may rehearse one ID, and the second real call is refused as today.

**Decisions.**

- **D9.** `dry_run` covers `update` and `create` only.
- **D10.** The `ref-dangling` filter covers any live create or amendment, not only the agent's own.
- **Working answer, the owner's (2026-10-31).** `--open` asks one `[y/N]` per proposal. One consent over a shown set is out of this slice and stays with the owner: ADR-0038 says "never one for a round", `architecture.md#apply` says "one `[y/N]` each", and 08's decision 12 is about a baseline, built by `changeset` in RM-0056. If the keys still hurt after the gate, that is a new ADR amending ADR-0038.

No ADR: findings still never refuse (ADR-0012), and one key per item keeps ADR-0035, ADR-0038 and `architecture.md#apply`.

**Assumption A1.** Fixing the grammar removes the 6 updates and 6 rejections, leaving about 17 approvals plus 4 card acts per first flow. That is an estimate from the trial report; the gate measures it.

## Acceptance criteria

- [ ] AC-01 — A dry run of a new file gives the real call's `diagnostics`; `spec inbox` stays empty, no `proposal.created` event, `git status` empty, and a real call with the same ID then stores. M: the row stored; validation skipped.
- [ ] AC-02 — A taken ID, a stale base and `class: generated` each give the real reason, exit 1, on both forms. M: returning before step 4.
- [ ] AC-03 — `dry_run` with `interpretation` or `amendment` is an error naming it, and `spec inbox` stays empty. M: ignored, a row stored.
- [ ] AC-04 — Create A, then create B `follows: [A]`: no `ref-dangling` in B's `diagnostics`, a note names A's PR. The same for `parent: A` and for an update adding the link. A reference to nothing keeps its finding. A rejected, B applied: `spec check` reports it. M: filtering on the code alone.
- [ ] AC-05 — Through the library with a scripted consent: three open creates give three questions in order and three commits, each with its `Proposal:`. `n` on the second leaves it open, the third is asked, the outcome holds it in `left` and the exit is 1. M: one answer reused; stopping at the decline.
- [ ] AC-06 — Through the binary, a piped stdin carrying `y`: exit 2, no event, no commit. `--open` beside a `PR` or `--editorial`: exit 2. M: the terminal check after the list read.
- [ ] AC-07 — A stale item between two good ones: its refusal printed, the other two applied, exit 1. A question and a drift item in the queue are not asked and each is named in its note. M: stopping at a refusal; walking `Decides`.

## Out of scope

One consent over a set (above). A multi-file proposal: one row and one path per proposal, a queue step and a rewrite of the apply door, 6-10 h. Revisable drafts: RM-0057. `spec check` validating `[flow]`: `spec-flow.md` "`[flow]`" says "never a finding or a gate". A `--yes` of any kind.

If time presses, `--open` is the first thing to cut: a shell loop over `spec approve` does the same today.

## Implementation

To fill in after implementation.

### Files in scope

CLI `propose.rs`, `create.rs`, `apply.rs`, `inbox.rs` (the repository's listing, shared), `main.rs`. MCP `intake.rs` (`ChangeArgs.dry_run`, the form error). Plugin version, `SCHEMA_PINS`. No `mirror.rs` change: `ReviewDocument.id` is already an `Option`, and `ChangeArgs` lives in `intake.rs`.

### Canon at ship

`proposal-kinds.md` (2.2 KB free) "Propose", "MCP and plugin"; `proposal-apply.md` (4.5 KB free) "Consent"; `plugin-files.md` "Version"; `round.md` needs no edit. `crates/specengine-cli/README.md` has 3 B free and lists no approve flag, so it stays as it is. `mcp-read.md` (5 B free) is not touched: the tool count stays twenty-nine.
