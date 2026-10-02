---
class: canon
tier: 2
scope: [crates/specengine-cli, crates/specengine-core, crates/specengine-store]
owner: owner
reviewed: 2026-10-02
---

# spec bundle and bundle_hash

CLI pass 4 (`docs/features/spec-cli-bundle.md`, 05 §6): one call returns the context around named targets within a budget of estimated tokens and names the rest by ID for follow-up reads, so an agent's steps do not grow with the corpus. Kind-agnostic (ADR-0008), deterministic (08 AC-7), never over the output ceiling. One library function `bundle(&Env, &Globals, &BundleRequest) -> BundleOutcome` (`render_text`, `render_json`); MCP `get_context_bundle` wraps it and gets the same body and hash (`docs/canon/mcp-read.md`). As `spec show` (`crates/specengine-cli/README.md`): discovery, `--json`, streams, the one-line rule, writes only in the data directory.

## Command

`spec bundle REF… [--budget N]`, globals `--root`, `--config`, `--json`; no `--task` (Phase 2).

- **REF**: `spec show`'s forms and resolution. Every REF is classified first (a look-alike or mixed-script ID, `project:` → exit 2 whatever the others), then located in order. Several holders → all, one `warning:`; REFs naming one node count once. A target within another merges into the outermost, one ``note: `X` is within `Y`: bundled as part of it``: X shares Y's form, so it is in the body only when Y prints its text; Y outlined or header-only, the not-included list names X (a direct child ID section of Y) or the child section holding it.
- **Budget**, in `tokens_est` (core, uncalibrated): `--budget N` (1..=`u32::MAX`), else `[budgets] bundle_node`, else `DEFAULT_BUNDLE_BUDGET` 2 000. With `--budget` the key is not read. Without, core's `bundle_node_from_toml` reads that key alone from the config's text: a broken `[classes]`, `[check]` or other `[budgets]` key never stops a bundle (`spec check` judges them); a value that is no whole number in 1..=4294967295 → exit 2 at its line, the bound `spec check` applies too (`docs/canon/spec-check.md` "Configuration"). `bundle_task` waits for `--task`.
- **Input**: pass 3's (`docs/canon/spec-cli-graph.md` "Input", "Live sources"): `update`, every used file re-read and re-parsed, one `SpecGraph` per call, so an edit shows in the next bundle without `spec index`. Candidates come from live files only; a named target is always admitted (` | archived`) and the links written in its own file are followed.

## Layers

Core `check::bundle_layers(&SpecGraph, &[NodeAt]) -> BundleLayers`, pure, by link type, direction and `[ids]` scope only, never a kind (no kind literal in the bundle sources). **Linked**: a resolved link with one end on a target or a node within it (`SpecGraph::within`), the other on the candidate, written in a live file or a target's own; the direction is the target's (`out`: target → candidate). Dangling, `skipped`, `unchecked` links are not followed; a candidate lives in a live file, within no target.

| # | JSON key, heading | Candidates | Form |
|---|---|---|---|
| 1 | `targets`, Targets | the resolved REFs by (path, ord) | `spec show`'s header and text; degrades (Fitting) |
| 2 | `open_questions`, Open questions | linked by any type, `mentions` included; holding a `working_answer:` edge (any state); no resolved `answers` written in a live file lands on it or a node within it | header, summary, `working answer: …` |
| 3 | `ancestors`, Ancestors | `SpecGraph::ancestors` per target, nearest first; a non-live one is dropped and the chain goes on past it; a `parent:` cycle ends at its first member (pass 3) | header, summary |
| 4 | `criteria`, Criteria | sources of `verifies` linked in; an ID section whose prefix has `scope = "feature"`, linked in by any type | header, text |
| 5 | `bindings`, Bindings | none until Phase 3 | — |
| 6 | `decisions`, Decisions | `canon` in | header, summary |
| 7 | `neighbours`, Neighbours | `depends_on` out, `constrains` both ways, `derived_from` out | header, summary |
| 8 | `terms`, Terms | `uses_term` out | header, summary |
| 9 | `tests`, Tests | none until Phase 3 | — |

Rules 4, 6–8: `BUNDLE_LINK_TYPES`. A node is a candidate once, in its first layer (2, 3, 4, 6, 7, 8); within a layer by (path, ord), ancestors target by target. Open means open by links, not by `status:`; an answered question's decision arrives in layer 6 through `canon:`. Incoming `depends_on` is impact (`spec graph --impact`), not context. Keys and headings live in the CLI (`layer_key`, `layer_heading`): core's genre scan bans layer 6's word.

Item header `<name> | <kind or -> | <title or -> | <path>:<line>`, + ` | status <s>` verbatim (layer 2), + ` | via <type> <in|out>, …` by type, `in` first (2, 4, 6–8); no token count, so an edit elsewhere in its file leaves it alone. Summary: a document's `summary` as written; a section or a summary-less document prints its header alone (`form: header`). `working answer:` the resolved answer's header, else `<written> | <path>:<line>` plus `--links`'s state suffix.

## Fitting

Order: layer 1 → 8, then the not-included list. One empty line before each `##` heading and between items; a text gains `\n` if missing; an empty layer prints no heading. **Fits**: the body as it would print, with the frame parts not yet placed and the reserve, has `tokens_est` ≤ the budget and ≤ `OUTPUT_CAP_CHARS` (40 000, one ceiling for every interface) characters. The CLI never cuts a bundle.

1. **Frame**: `# Bundle: <target names, ", ">`, `## Targets`, each target's header marked ` | outline`, then the **reserve** `## Not included` + `- <n> more`, n = every candidate plus the targets' direct child ID sections; n = 0 → no reserve. `tokens_est(frame)` is the **minimum**: a budget below it → exit 2 naming the minimum and the budget's source (`--budget N`, `` `[budgets] bundle_node = N` (<config>:<line>) ``, the default); a frame over 40 000 characters → exit 2 whatever the budget.
2. **Targets degrade, never cut**: each takes its first form that fits: `text` (`spec show`'s header and bytes), `outline` (the marked header and its document's summary; a section or a summary-less document has none), `header` (the marked header alone; always fits, it is the frame's). An outlined or header-only target's direct child ID sections lead the not-included list. 05 §6's "always" means placed first, not over budget (ADR-0027).
3. **Greedy**: each candidate, in order, enters iff the body with its heading or blank line, its text and the reserve fits; else it joins the not-included list in order.
4. **Tail**: `- <name> | <title or -> | <tokens_est> tokens` (the node's own, as `spec show --json`), at most `BUNDLE_TAIL_LINES` 20, each only while the body with it and the reserved more line fits (the last entry: without it); from the first that does not, the rest count in `- <k> more`. The tail gets only the room the items left: 20 lines when it allows.

## Output

Text: the body, then two lines outside it.

```text
# Bundle: MEC-STAMINA

## Targets
MEC-STAMINA | mechanic | Stamina | docs/spec/movement/stamina.md:1 | <t> tokens | status accepted
<the file>

## Ancestors
DOM-MOVEMENT | domain | Movement | docs/spec/movement/README.md:1
<its summary>
<…>

## Not included
- TERM-exhausted | Exhausted | <t> tokens
bundle_hash b3:<64 hex digits>
tokens <t> of 2000, chars <c>, bytes <b>, not included 1
```

`tokens` = `tokens_est(body)`, `chars` its Unicode scalar values, `bytes` its UTF-8 bytes (08 AC-1's unit); `not included` = tail lines + `more`.

`--json`, one document, every key present, absent = `null`: `{refs, reason, notes, task, budget, tokens, chars, bytes, bundle_hash, body, layers, tail, more}`; `task` always `null`; `body` the text's, once; `layers` the nine keys in print order, arrays, `[]` when empty; `tail` the listed entries `{name, title, path, line, tokens_est, layer}` (`layer` the key it stands in, `targets` for an outlined target's sections); `more` the `k`. An item `{name, kind, title, path, line, form, status, via, working_answer, tokens_est, archived}`, no text: `form` `text | outline | header | summary`; `status` a target's (as `spec show`) or an open question's, else `null`; `via` `[{type, direction}]`, `null` in layers 1, 3; `working_answer` (layer 2, else `null`) `{name, written, path, line, state}`: `name` the resolved answer or `null`, `path:line` where `working_answer:` is written in every state, `state` `--links`'s. Exit 1: `refs`, `reason`, `notes` set, the rest `null`.

**`bundle_hash`** = `b3:` + lowercase hex BLAKE3 of the body's UTF-8 bytes as printed (`specengine_store::b3_hash`, the `spec.lock` form, 05 §3.5), the same in text and JSON. It names the output, not the input: 08 AC-7, the Phase 2 package's reference (07 §1.2, ADR-0027), `runs.bundle_hash` (05 §3.3); "context changed" is a recomputation. Not staleness (`spec_snapshot`, Phase 2). Nothing is stored.

## Exit, determinism, writes

Exit 0 answered: dangling links and parents, cycles (each node once), empty layers. 1 only a REF that does not resolve (`show`'s reasons): no bundle, `spec: <reason>`, JSON `reason`. 2, no JSON, as `show`: usage (no REF, `--budget` not in 1..=`u32::MAX`), a look-alike or mixed-script ID (naming the Latin fix), `project:`, config (`bundle_node` included), `HOME`, `StoreError`, the minimum, the frame over the ceiling.

Determinism: the orders above, never rowid, insertion, time, `HashMap` order or the absolute root; the title names the resolved targets, not the REFs as typed; copies in opposite file orders, at different roots and `HOME`s give byte-identical text, JSON and hash. Records print verbatim (`docs/canon/architecture.md#ui`); labels are English and stack-neutral (no P2-3 word, 07 §1.2). Writes: the data directory only (`update`); no table or column, `INDEX_FORMAT` stays 6.

## API

- core `check`: `bundle_layers`; `BundleLayer` (`ALL`, print order); `BUNDLE_LINK_TYPES: [(BundleLayer, &str, Direction); 7]`; `BundleCandidate {node, layer, via, working_answer}` (`via` sorted pairs, `working_answer` the index of its first `working_answer` edge); `BundleLayers {targets, merged: [(inner, outer)], candidates}`; `bundle_node_from_toml(&str) -> Result<Option<BundleNode {tokens: u32, line}>, ConfigError>`; `SpecGraph::file` (a file's bytes and parse), `SpecGraph::scheme`.
- store: `b3_hash(&[u8]) -> String`. The CLI gains no dependency edge (eval `build_graph.rs`).
- CLI: `bundle`; `BundleRequest {references, budget: Option<i64>}`, `BundleOutcome {references, reason, messages, bundle}`, `Bundle {budget, tokens, chars, bytes, bundle_hash, body, layers, tail, more}` (`not_included`), `BundleItem`, `ItemForm`, `WorkingAnswer`, `TailEntry`; `layer_key`, `layer_heading`, `DEFAULT_BUNDLE_BUDGET`, `BUNDLE_TAIL_LINES`; `Outcome::Bundle`.

Tests: CLI `bundle.rs` (layers, REFs, keys), `bundle_fit.rs` (fitting, the hash against a spec-derived BLAKE3, determinism), `bundle_config.rs` (budget source, read-only, format, stack words), `common/bundle.rs`, over scratch copies of `fixtures/spec-a`, `-b` with their own `HOME`; core `bundle_layers.rs`, `check_config.rs`.

## Not yet

`--task`, task bundles, `bundle_task`, the package (Phase 2); proposals in layer 2; layers 5 and 9 (Phase 3; the keys stay, empty); the `bundles` log, `runs`, the follow-up-reads signal, `rusqlite_migration` (with the first operational table, Phase 2); the "changed since last time" header (06), 07 §1.1 Delta and Hints; MCP `compact`; a tokenizer crate.

## Open

- Fitting re-estimates the whole body per candidate (`fits`): 3 000 candidates 1.13 s against `spec graph`'s 0.62 s (debug), on top of pass 3's per-call re-parse; MCP inherits both (886 ms, 1 821 ms at budget 10 000: `docs/canon/mcp-read.md` "Latency"). Running counts next.
- Calibration (Q6): no reference token counts yet (model, date, five `count_tokens` figures); `tokens_est` stays uncalibrated, its calibration test `#[ignore]`d (the task's AC-17); recalibrating is an `INDEX_FORMAT` change.
- A `working_answer:` written as a Cyrillic legacy alias makes no link (`unparsed-reference`): the question holds no `working_answer` edge and drops out of open questions. Existing front-matter behaviour (core), noted only.
