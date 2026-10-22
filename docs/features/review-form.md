---
class: spec
status: shipped
scope: [crates/specengine-mcp, crates/specengine-cli, plugin]
ref: RM-0012 slice 1; requirement analysis 2026-10-22 against 962457a, Q1-Q7 decided under the owner's pre-approval, review open; 07 s1.2; 08 s2 Phase 5
shipped: 2026-10-22
adrs: [ADR-0054, ADR-0035]
---

# review-form (RM-0012 slice 1): the owner stages a decision from the session

## Why

To pick an option or write a note on screen the owner must start `spec serve` and the UI, or type every flag on a terminal. He wants to choose in the session and only confirm on a terminal. On his request the agent opens his form, Claude Code prompts and renders it, his answer is staged, and `spec approve|reject PR` confirms it. The agent never supplies the choice: an agent-written stage adds nothing (`spec approve PR` with no flag already takes the working answer) and would pass the agent's pick off as the owner's (ADR-0054).

## Summary

`review_proposal {proposal_id}` in the default build, both eras: input `proposal_id` only, `_meta` `maxResultSizeChars` and `requiresUserInteraction`, four `false` annotations, `get_proposal`'s output schema. Before any form one read-only CLI call, `stage_form`, refuses in `stage`'s words (ID, unknown, another repository or an orphan, not `open`, a drift item) and gives the brief document and the kind family; a client without form elicitation gets the no-form line. The form's message marks agent-declared provenance and agent text as data, escaped and cut; `choice` follows the family, never a kind name; `note`. Only `accept` stages, through `stage()` against the `updated_at` read before the form (the stateless era carries it in an unsealed `requestState`); the result is the outcome lines and `spec review PR --brief` read after. The terminal's `spec approve|reject PR` confirms. The `probes` demo of the same name, its sealed state, rmcp's `request-state` and `getrandom` are gone. Contract, byte for byte, and the owner's check (R8: titled-enum labels, open): `docs/canon/review-form.md`.

Decisions: Q1 ADR-0054 supplements ADR-0035; Q2 `approve_task` to RM-0038 `task-stage`, its own ADR; Q3 form mode, URL mode later if ever; Q4 the demo retired, its code reused; Q5 no `via` key; Q6 default build; Q7 only the main session, when the owner asks, never after a decline. Out of scope: a `spec stage` command, unstaging over MCP, `--answer`/`--canon` in the form, URL mode, `approve_task`, any daemon, UI, store, queue-schema or event change.

## Acceptance criteria

rust-developer:

- [x] D1 -- both eras: default `tools/list` 22 tools; `review_proposal` input exactly required `proposal_id`, `additionalProperties: false`; `_meta` exactly both keys; four `false` annotations; output schema byte-equal to `get_proposal`'s; `probes` adds only `probe_output`, `probe_sleep`. Tests: `mcp_default` (exact list, `d1`), `mcp_stdio` (`probes`), `TOOLS` 22.
- [x] D2 -- a look-alike ID (U+0420 for `P`), `PR-9999`, an applied, another repository's, an orphan, a drift item: no form, `staged` NULL, no event, content the CLI's bytes. Tests: `mcp_review_form` `d2`.
- [x] D3 -- `accept` per family (option + note; working answer; editorial; reject + reason) -> one `proposal.staged`; `staged` equals `stage()` of the same body; outcome lines; `structuredContent` = `review --brief --json` after, schema met. Tests: `mcp_review_form` three `d3`, `mcp_door`.
- [x] D4 -- decline, dismissed form, a client without form elicitation, a cancelled legacy call: `staged` NULL, no event; the cancelled call gets `notifications/cancelled` for its form and no response; a late answer stages nothing. Tests: `mcp_review_form` `d4`.
- [x] D5 -- proposal changed between form and answer (legacy) or between rounds (stateless): "changed since it was read", nothing staged. Tests: `mcp_review_form` `d5`, both eras.
- [x] D6 -- `INSTRUCTIONS` (2 040 B) and the description (1 977 B) each <= 2 048 B, ASCII, the determinism sentence, name `review_proposal` with its twin and "only when the owner asks", no stack word, no kind literal. Tests: the `INSTRUCTIONS` pins (`mcp_decision`, `mcp_path`), `mcp_genre`.
- [x] D7 -- `ask-owner` names `review_proposal` (main session, the owner asks, once, never after a decline, a stage no answer), body 4 095 B; `plugin.json` 0.19.0; `getrandom` gone, `probes = []`. Tests: `plugin_files`, `plugin_skills`, `plugin_roles` (0.19.0), eval `build_graph` (6).

test-engineer: new `mcp_review_form.rs` (14 tests, both eras); `mcp_default`; `mcp_stage` (a recursive walk: no tool input carries a choice); `mcp_door`; `mcp_stdio` retargeted (also mending three tests red at HEAD under `probes`, a missing `get_impact`); `mcp_genre`; `plugin_files`, `plugin_skills`, `plugin_roles`; `mcp_tasks` ac14 narrowed; the `INSTRUCTIONS` pins; eval `build_graph`; the new shard in `shards_repo`, `parity`, `check_parity`:

- [x] TE -- each mutation red (phase B, iteration 1; M6b too): M1 drop `requiresUserInteraction` (`mcp_default` exact list, `d1`); M2 add an `option` argument (those two, `mcp_stage` ac15); M3 stage on decline (`d4` decline, dismiss); M4 re-read `updated_at` at round 2 (`d5` stateless); M5 skip the capability check (`d4` no form elicitation); M6, M6b send the form before the not-open/drift check (`d2`); M7 call `approve` instead of `stage` (the three `d3`, `mcp_door`: the apply and its commit); M8 omit `notifications/cancelled` (`d4` legacy cancel).

spec-writer:

- [x] W1-W6 -- this spec; ADR-0054 (1 311 B); canon `review-form.md`, `decision-staging.md` "Rule", "Threat model", `mcp-read.md`, `architecture.md#apply`, `plugin.md` "Skills", `plugin-files.md`; 07 s1.2, 08 Phase 2 and 5, 06 s3.3; MCP README, RM-0012, RM-0038, shard `docs/index-canon-proposal.md`; export and check clean.
- [x] W7 -- `INSTRUCTIONS`' bytes in `mcp-read.md` **Texts**, 0.19.0 in `plugin-files.md`, the owner's check with R8 in canon `review-form.md`, `CLAUDE.md` State, RM-0012 shipped; iteration 2's canon (`review-form.md` "Form", "Answer", "Result", "Eras"; `stack.md`; both READMEs).

## Verification

Smoke (developer, iteration 2): `tools/list` 22 tools, `INSTRUCTIONS` 2 040 B, `review_proposal` with exactly the two `_meta` keys and four `false` annotations; accept, decline, dismiss as the canon says; refusals send no form (`PR-9999` exit 1 with the document; the look-alike exit 2, its Latin-fix line); stateless round 1 deterministic, round 2 staged a reject; a legacy cancel sent `notifications/cancelled`, the late answer staged nothing; a discrepancy offered `option-0` (recommended), `option-1`, `reject`; post-form exit 1 (a 5 000 B note): the CLI line and the brief document; exit 2 (a blank reason; `option-5` on a question): the line only; `note: null` staged a bare approve; U+2028, U+2029 escaped. Iteration 2's tests: `d3` pins the exit per case (exit 2 the line only, exit 1 the brief document against the full one); `d5` legacy runs on an update; escaping covers U+2028, U+2029; new `d4_a_call_cancelled_before_its_form_sends_none` (10 of 10 runs); `build_graph` pins rmcp `["server","macros","transport-io"]` and `getrandom` gone; re-checks R1-R4 all red; `mcp_review_form` 16 of 16. Final gates: fmt, clippy (`probes` too) clean; nextest workspace 3 889 of 3 889, 70 skipped; `-p specengine-mcp --features probes` 97 of 97; UI lint 0, `pnpm test` 2 546 of 2 546, build clean; export, check, parity clean; eval quartet 86 of 86.

## Implementation

| Module | What it does |
|---|---|
| CLI `stage.rs` | `stage_form` (read-only: `stage`'s ID, repository, `open` and drift refusals, else the brief document and `StageForm {decides, editorial}`), `StageFormOutcome`; `StageOutcome::brief`; `staged_command` public |
| CLI `lib.rs` | re-exports them; `escape_controls` public |
| MCP `review.rs` | the tool and its description (1 977 B): `review` (pre-check, capability, era), `resume` (round 2), `answered` (answer -> `StageBody`), `after` (`stage`, the read after, outcome lines), `refusal` (the read tools' rule before and after the form), `form_of`, `body_of`, `value` |
| MCP `server.rs` | `review_tools` in the default router; `INSTRUCTIONS` names it: 2 040 B, 1 902 less the tasks line, 8 left, after cutting ` as Markdown`, ` beside the code`, report_drift's ` for the owner`, `, with a tail on narrowing`; no `requestState` key |
| MCP `read.rs` | `answer_of`, the read tools' rendering shared with the form |
| MCP `lib.rs`, `Cargo.toml`; workspace `Cargo.toml`, `Cargo.lock` | the module unconditional, the demo's re-exports gone; `probes = []`; rmcp `["server", "macros", "transport-io"]`, `getrandom` gone; the lock lost `base64`, `cmov`, `ctutils`, `hmac`, `zeroize` |
| `plugin/` | `ask-owner` names `review_proposal` (cut to 4 095 B); `plugin.json` 0.19.0 |
| tests | the criteria's |

Deviations from the analysis: `stage_form` in the CLI (`review --brief` answers an applied or drift proposal at exit 0 and another repository at exit 2; `KindFamily` is the store's); `staged_command` public; `getrandom` removed, not left optional; `PINS`, `SCHEMA_PINS` the tester's; an orphan refused before the form; the CLI's ID check, the demo's `check_proposal_id` gone; 06 s3.3 edited too; M7's library `approve` exits 2 without a terminal, red as no event.

Iteration 2 (canon first; delta review accepted): `StageOutcome::brief` and MCP `refusal()` apply the read tools' rule before and after the form; the cancel checked before the form; U+2028, U+2029 escaped; the not-shown error escaped; `note: null` read as absent; `request-state` and `getrandom` dropped. Last fix: an exit-2 usage refusal after the form is not briefed, so it stays the CLI's single `spec:` line even when the stored findings exceed the tail (`d3_a_usage_refusal_of_an_item_with_many_findings_is_its_line_alone`, both eras: an update citing 21 undeclared IDs, one over the cap of 20; red with the fix reverted). Later: refused documents built without the diff (an accepted form previews three times).
