---
class: spec
status: shipped
scope: [crates/specengine-core]
ref: the one warning of spec check after ADR-0039 (2026-10-08); the orchestrator's working answers under the owner's pre-approval
shipped: 2026-10-08
adrs: []
---

# Exempt records

## Why

ADR-0039 superseded two decisions; `spec check` then warned `ref-superseded` on `docs/decisions/ADR-0031.md:14` (an inline mention of one of them), the canon's "This repository ...: `clean`, no warning" was false, and five clean pins failed. The finding had no fix at its source: a decision is "never edited, only superseded" (`docs/README.md`, ADR-0022), re-superseding ADR-0031 to re-cite is absurd, and a debt entry would carry a permanent non-defect. An accepted decision that cited Y while Y was live (dated before Y's successor X) is now exempt, like the other sources right by construction; one dated on or after X cited a dead document and still warns. No ADR: this applies ADR-0022's class table to one more rule. Genre (ADR-0008): `decision`, `accepted`, `date:` are the convention's own words.

## Acceptance criteria

Tests: core `check_graph.rs` "dated decisions" (11), X dated 2026-09-10, `MEC-NEW` undated; each source has `refs`, `links.derived_from` to `ADR-0001` and the body `Cites ADR-0001 and a MEC-OLD-based rule.`

- [x] AC-01 `ref_superseded` skips a reference when the source's readable front-matter is `class: decision`, `status: accepted`, `date:` D of `YYYY-MM-DD` shape, and X's date (same reading, X's first holder in path order) is later than D as text. No clock, no config key; `check_genre.rs` green.
- [x] AC-02 Unchanged and green: cli `check_rules` `this_repository_is_clean_under_its_shipping_rules`, `parity` `g_from_the_top_is_clean_with_no_finding`; store `check_parity` `this_repository_is_clean_under_the_root_config`, `citing_the_superseded_layout_decision_is_one_ref_superseded`, `each_seed_blocks_exactly_its_target`. M1 (exemption dropped): red.
- [x] AC-03 `an_accepted_decision_dated_before_x_keeps_only_the_undated_successor_s_finding`: dated 2026-09-01, with `canon: ADR-0001`: one finding, `MEC-OLD-based`. M1: 5.
- [x] AC-04 Lines pinned: (a) `a_draft_spec_dated_before_x_still_warns` 4; (b) `a_canon_dated_before_x_still_warns` 4; (c) `a_decision_without_status_dated_before_x_still_warns` 5; (d) `an_accepted_decision_with_a_failed_front_matter_warns_for_its_body` 2; (e) `accepted_decisions_dated_on_or_after_x_s_day_still_warn` 5 each; (f) `an_undated_x_exempts_no_accepted_decision`; (g) `an_undated_accepted_decision_still_warns` 5; also `unshaped_dates_exempt_nothing`, `x_s_date_is_its_first_holder_s_in_path_order`, `the_dated_exemption_is_deterministic`. Red: M2 dates ignored ((e), (g), AC-03 silent); M3 `>=` ((e) at X's day down to 1); M4 status ignored ((c) down to 1); M5 every source exempt ((a), (b) down to 1: the undated `MEC-OLD-based` stays); M6 X's last holder; M7 shape filter dropped.
- [x] AC-05 Every existing `ref-superseded` test passes unchanged (`every_reference_form_to_a_superseded_document_warns_once` keeps its undated accepted `ADR-0003` row).
- [x] AC-06 core + eval 847, store `check_parity` 10, cli `check_rules` + `parity` 13: green; fmt, clippy clean.
- [x] AC-07 Canon applied; `spec check`: 0 warnings, `clean`. Measured: "Implementation".

## Implementation

One iteration, accepted. Canon: `docs/canon/spec-check-graph.md` "Graph warnings" (the exemption; the dashes around the reference forms made commas), "Index render" (the duplicate `is_live` definition dropped), "Open nits" (one word): 12 282 B. AC-07 measured: 0 errors, 0 warnings, `clean`, worst W 108 320 B.

| Module | What it does |
|---|---|
| core `check/graph.rs` `supersessions` | per superseded file a `Successor`: X as written, its holders, the date of the first holder |
| `shaped_date`, `accepted_decision_date` | a readable `date:` of `is_date_shaped` shape; D only for `class: decision` + `status: accepted` |
| `ref_superseded` | after the X's-own-files exemption, skips when X's date > D (text order); module doc states the rule |

**Cost** (review): an accepted decision dated before X but edited afterwards to cite Y (against the convention) goes silent, the check reads no history; a placeholder or back-dated D (`date: 0000-00-00` passes the shape check) exempts everything that decision cites, `is_calendar_date` for D being a possible later change; a decision dated on X's day that cited Y before X was written warns with no fix at its source (a debt entry carries it); an undated X exempts no citer.
