---
class: spec
status: shipped
scope: [spikes]
ref: 'owner request "Phase 0: spikes" 2026-09-28'
shipped: 2026-09-29
adrs: [ADR-0008, ADR-0012, ADR-0016, ADR-0020, ADR-0021, ADR-0022, ADR-0023, ADR-0024]
---

# Phase 0 spikes: turn the engine's unverified claims into numbers on the two pilot corpora

## Why

Spec 05 §5.1–5.2 and 04 §3–4 rested on research claims nobody here had executed: AST-hash traps 2–5, `bevy_dev_tools::schedule_data`, the rust-analyzer cost estimate, the flagged `tree-sitter-ron` 0.2.0, and the Claude Code behaviours the approval flow depends on — elicitation form, `_meta["anthropic/requiresUserInteraction"]`, output limits, 2-minute backgrounding, both protocol eras. If any was wrong, Phases 1–3 would build on it: a hash that flickers under `cargo fmt` teaches the owner to ignore drift, a form that never renders removes the single control point (ADR-0012), a layer needing 4 GB cannot run on the laptop.

Six probe groups delivered as production code (owner decision: no throwaway crate), each yielding a number or a yes/no per pilot, to confirm ADR-0020, ADR-0021 and the stack rows of 05 §9 / 04 §6 or trigger superseding decisions.

## Acceptance criteria

- [x] AC-01 Build graph: default members `xtask`, `-code`, `-mcp`, `-import`, `-eval`; core graphs free of `ra_ap_*`, `syn` 3, Bevy; `--features ra` pulls `ra_ap_*` `=0.0.352` only via `specengine-ra` (`build_graph.rs`).
- [x] AC-02 Read-only guard: `--out` under the corpus → exit 2, nothing written; fixture runs leave `fixtures/` clean.
- [x] AC-03 Hash stability: 100 % on error-free fixture items under default fmt, contrasting fmt and comment stripping; `files_changed > 0`.
- [x] AC-04 `cannot_verify`: items with `has_error()` are never hashed; two broken items never share a hash.
- [x] AC-05 `syn` 3 comparison printed (failures, naive / normalized stability, time ratio); verdict on ADR-0021 recorded.
- [x] AC-06 Pilot `ast-hash` runs finish under the timeout with every field; ≥ 1 000 items each.
- [x] AC-07 RON: nested markers resolve to field paths, orphans to `unanchored`; the 53-case `ADJACENCY` table passes under LF and CRLF; verdict `lexer`.
- [x] AC-08 MCP scripted: legacy, stateless, elicitation in both eras, `requiresUserInteraction`, a ≥ 26 k-token output (`mcp_stdio.rs`).
- [x] AC-09 MCP manual: owner checklist on Claude Code 2.1.283; every row observed except HTTP 405 (not measured → 08 §5).
- [x] AC-10 Bevy: both pilots dumped and compared; `fixtures/bevy-mini` yields exactly the four hand-listed registrations.
- [x] AC-11 rust-analyzer: cold, peak RSS, warm, moniker share on both pilots, with and without the proc-macro server; overruns give `"timeout"`.
- [x] AC-12 Census: `fixtures/corpus-mini` counts equal `expected.json`; pilots appear as counts only.
- [x] AC-13 Anonymity and language: no absolute path or pilot name in `docs/`, `crates/`, `fixtures/`; census stdout carries anonymised keys only; English.
- [x] AC-14 Verdict closure: all 12 claims closed — 9 confirmed, 3 refuted → a spec section, 0 not measured.

## Summary

| Claim | Outcome | Lives now |
|---|---|---|
| Normalized tree-sitter hash is stable (ADR-0021) | confirmed for recipe v2: 100 % on 11 852 + 7 599 items; v1 **refuted** (84.9–85.4 % under contrasting fmt) | 05 §5.2; `crates/specengine-code/README.md` |
| `syn` 3 is not a better digest | confirmed: 94.5–94.9 % after normalisation | 05 §5.2; 04 §6 |
| Parse errors ≈ 1 % of items | confirmed: 0 on both pilots | 05 §5.2 |
| `qpath` names items unambiguously | **refuted**: 16.5–32.2 % ambiguous, bin / examples / tests collide | 05 §5.1; 08 §2 Phase 1 |
| `tree-sitter-ron` 0.2.0 serves markers | **refuted** → own RON lexer (parse-clean 97.5 / 100 %) | 05 §5.3, §9; 04 §6; 08 §5 |
| Layer C needed for registrations | confirmed: detector 100 / 93.4 %, every miss a generic instance | 05 §5.1 |
| `schedule_data` dump via a small patch | confirmed: 6 lines, headless, schema drift 0 | 05 §5.1; 08 §5 |
| rust-analyzer affordable (≤ 3 min, ≤ 4 GiB) | confirmed: cold ≤ 64.7 s, group peak 3.76 GiB, monikers 99.7–100 % | 05 §5.1; 08 §5; `crates/specengine-ra/README.md` |
| Both eras, form, `requiresUserInteraction` | confirmed by script and by the owner | 04 §4; 07 §1.1–1.2; `crates/specengine-mcp/README.md` |
| Output cap 10 k / 25 k tokens, raised by `MAX_MCP_OUTPUT_TOKENS` | **refuted**: the cap counts characters (48 000 inline, 104 000 rejected); the variable does not raise it | 04 §4; 07 §1.1 |
| Calls > 2 min go to the background, form-held calls exempt | confirmed | 04 §4; 07 §1.1 |
| A corpus convention fits a config (ADR-0008) | confirmed on both pilots | 08 §4.3; `crates/specengine-import/README.md` |

No new ADR; ADR-0020 and ADR-0021 stand. Owner rulings made during the work are spec-level: RON adjacency rules 1–5 (05 §5.3), the per-system 95 % threshold (05 §5.1), the whole-process-group RSS measure (`crates/specengine-eval/README.md`), exact dependency pins and rustc ≥ 1.98 for `ra_ap` (04 §6). Deliberate divergences: stable rustfmt rejects `trailing_comma`, so the contrasting format is effectively `max_width = 60`; AC-13's prefix grep does not discriminate and is replaced by a stdout whitelist test. Open minors live in the crate READMEs; the harness contract in `crates/specengine-eval/README.md`. Plans, results tables, the owner checklist and review notes: git `6468f07`.
