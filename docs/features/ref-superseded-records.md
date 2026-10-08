---
class: spec
status: draft
scope: [crates/specengine-core]
ref: the one warning of spec check after ADR-0039 (2026-10-08); the orchestrator's working answers under the owner's pre-approval
adrs: []
---

# Exempt records

## Why

ADR-0039 superseded two decisions. Since then `spec check` on this repository gives one warning, `ref-superseded` on `docs/decisions/ADR-0031.md:14` (an inline mention of one of them), `docs/canon/spec-check-graph.md` "This repository ...: `clean`, no warning" is false, and five tests pinning the repository as clean with no finding fail (AC-02).

The finding has no fix at its source. A decision is "never edited, only superseded" (`docs/README.md` "Classes and where they live", ADR-0022); superseding ADR-0031 only to re-cite is absurd; a `.spec-debt.toml` entry would carry a permanent non-defect as debt. A warning asking for an edit the convention forbids is a defect of the rule. An accepted decision that cited Y while Y was live (dated before Y's successor X) joins the sources already exempt because their citation is right by construction (`supersedes:` items, `links.supersedes`, the `status:` value, X's own files). One dated on or after X cited a dead document: it still warns.

## Description and interactions

`ref-superseded` (`docs/canon/spec-check-graph.md` "Graph warnings") skips a reference, declared or inline, of a live source when the source's readable front-matter has `class: decision`, `status: accepted` and a `date:` D, Y's successor X has a `date:` too, and X's date is strictly later than D. Nothing else changes: other sources and references, `mention-dangling` and `depends-cycle` (an accepted decision included), resolution, output, verdict.

Where the rule lives, `crates/specengine-core/src/check/graph.rs`: `fn ref_superseded` (line 118); `supersessions` (line 102) gives per superseded file X as written and X's holders; live sources from `live_sources` (line 67, `is_live` at `check/render.rs`:299); the existing exemptions at lines 135-137 (declared keys) and 160 (X's own files); module doc lines 11-13. `readable_fields` (`check/render.rs`:271) gives `class`, `status`, `date`; `DocClass::Decision` names the class (as `is_tier3` does); `is_date_shaped` (`check/text.rs`:332) is the shape `date-invalid` checks.

**No ADR.** Nothing new is decided: ADR-0022's class table already says an accepted decision is never edited; this applies it to one more rule, and the canon line records it. **Genre** (ADR-0008 holds): `decision`, `accepted` and `date:` are the convention's own class, status and key, which the core already reads; no project literal, no config key.

## Data

No format changes. The inputs are front-matter keys the check already reads, of the citing decision and of X:

```yaml
# docs/decisions/ADR-0031.md (abridged): the citing source
class: decision
status: accepted
date: 2026-10-02
# docs/decisions/ADR-0039.md (abridged): X, the successor of the cited Y
date: 2026-10-08
```

ADR-0031's line 14 cites Y inline; 2026-10-08 > 2026-10-02, so one `ref-superseded` today, none after.

Canon diff at shipping, `docs/canon/spec-check-graph.md` (12 280 of 12 288 B), net +2 B:

```text
"Graph warnings", the ref-superseded paragraph (+54 B, -6 B): the two em dashes around
"declared (...) or inline (with the fallback)" become commas, and
- ... the `status:` value, references from X's own files. Several holders ...
+ ... the `status:` value, references from X's own files or an accepted decision dated before X (never edited). Several holders ...
"Index render", the Tier 3 paragraph (-40 B; "Live source" defines it):
- ... archive filter; `is_live` (neither `class: generated` nor Tier 3) serves the graph rules.
+ ... archive filter; `is_live` serves the graph rules.
"Open nits" (-6 B):
- ... (closures work around it; `\bFile::` would be cleaner).
+ ... (closures work around it; `\bFile::` is cleaner).
```

The module doc of `check/graph.rs` (lines 11-13) names the new exemption likewise.

## Rules and edge cases

- WHEN a live source's readable front-matter has `class: decision`, `status: accepted` and a `date:` D of shape `YYYY-MM-DD` (`is_date_shaped`), and a reference of it resolves to a superseded Y whose successor X's first holder (path order) has a readable `date:` of that shape later than D, `ref-superseded` SHALL give no finding for that reference. Declared (`refs`, `adrs`, `links.*`, `parent`, `working_answer`, a reference-form `canon:`) and inline (with the fallback) alike.
- WHEN D equals or follows X's date, the reference SHALL warn as today: strictly later only.
- WHEN D or X's date is absent, unreadable (front-matter failed) or not of that shape, the reference SHALL warn as today: no exemption.
- Dates compare as text: for `YYYY-MM-DD` the text order is the calendar's (as `check/base.rs`:74). No clock, no `today`.
- A live decision with no `status:` (`key-missing`), and any non-decision source, SHALL stay checked whatever its dates.
- Any other decision status (`rejected`, `superseded-by X`, an invalid `proposed` with its `status-invalid`) is Tier 3 by `is_tier3`, not a live source: no graph warning before or after. The convention has no draft decision status; this slice adds none.
- The existing exemptions stay.
- **Cost.** A decision dated the same day as X that cited Y before X was written warns with no fix at its source: a `.spec-debt.toml` entry carries it. An undated X (a canon superseding a canon) exempts no citer.

## Acceptance criteria

rust-developer:

- [ ] AC-01 `ref_superseded` reads D per live source: `readable_fields` gives `class` parsing to `DocClass::Decision`, `status` equal to `accepted` and `date` passing `is_date_shaped`, else none. Per supersession it reads X's date the same way from the first of X's holders (e.g. carried by `supersessions`). A reference resolving to a superseded Y is skipped iff both dates exist and X's date > D (text order). No clock, no config key; the module doc names it; `check_genre.rs` stays green.

test-engineer. The new tests use core `check_graph.rs`'s `SUPERSEDED` with its X (the successor of its `ADR-0001`) dated `date: 2026-09-10` (a dated copy or the constant; `MEC-NEW` stays undated). Each new source has `refs: [ADR-0001]`, `links.derived_from: [ADR-0001]` and the body line `Cites ADR-0001 and a MEC-OLD-based rule.`:

- [ ] AC-02 Unchanged, green with AC-01: cli `check_rules` `this_repository_is_clean_under_its_shipping_rules`; cli `parity` `g_from_the_top_is_clean_with_no_finding`; store `check_parity` `this_repository_is_clean_under_the_root_config`, `citing_the_superseded_layout_decision_is_one_ref_superseded` (exactly the seeded README's finding), `each_seed_blocks_exactly_its_target`. Red: AC-01 dropped.
- [ ] AC-03 New: an accepted decision `docs/decisions/ADR-0003.md` dated `2026-09-01`, with `canon: ADR-0001` besides the above, gives exactly one `ref-superseded`: `MEC-OLD-based` on the body line (case (f)); its four `ADR-0001` references none. Red: the exemption dropped (5 findings).
- [ ] AC-04 New, lines pinned:
  - (a) a draft spec: 4 findings (2 declared, 2 inline: `ADR-0001`, `MEC-OLD-based`);
  - (b) a canon: 4;
  - (c) a decision dated `2026-09-01` with no `status:`, with `canon: ADR-0001`: 5;
  - (d) an accepted decision dated `2026-09-01` whose front-matter failed (`title: a: b`): 2, body only;
  - (e) accepted decisions with `canon: ADR-0001`, dated `2026-09-10` (X's day) and `2026-09-20`: 5 each;
  - (f) X without a date: AC-03's `MEC-OLD-based` finding;
  - (g) an accepted decision with `canon: ADR-0001` and no `date:`: 5.
  Red: dates ignored ((e), (g) and AC-03 silent); `>=` for `>` ((e) at X's day down to 1); status ignored ((c) down to 1); every source exempt ((a), (b) silent).
- [ ] AC-05 `every_reference_form_to_a_superseded_document_warns_once` passes unchanged (its accepted `ADR-0003` has no `date:`, so its `canon:` row stays), as does every other `ref-superseded` test (core `check_graph.rs`, `check_scopes.rs`, `check_output.rs`, `check_genre.rs`, eval `check_cli.rs`).
- [ ] AC-06 `cargo nextest run -p specengine-core -p specengine-store -p specengine-eval`, and `-p specengine-cli --test check_rules --test parity`: green.

spec-writer:

- [ ] AC-07 At shipping: the canon diff of "Data" applied, `docs/canon/spec-check-graph.md` <= 12 288 B; `spec check` on this repository: `0 errors, 0 warnings`, `clean`; this spec `shipped`, compacted.

## Out of scope

- A base-aware rule (warn only on references a change introduces).
- A draft status for decisions; any change to `is_tier3` or to the other graph warnings.
- Fix data for `ref-superseded` (the canon's "Next").

## Implementation

Filled in after implementation.
