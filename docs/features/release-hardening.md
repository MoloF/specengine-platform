---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-http, ui, docs]
ref: the owner's word of 2026-11-13 ("close the tails and problems so the full release is complete and everything works without errors"); RM-0063; the orchestrator's hardening scan of 2026-11-13 (outside the repository; its findings are restated here) and its rulings
adrs: []
---

# release-hardening: every known flake fixed, the scan's defects closed, one clean full run

## Why

`v5` ends with `delivery-timeline`, and the owner asked to close the tails so the release is complete and runs without errors. No full green workspace run at `HEAD` is on record. Under load about fifteen tests fail on fixed deadlines or starved threads (RM-0063). A read-only scan of 2026-11-13 found thirteen defects; this release takes twelve, six in the UI and six in Rust. Each is small, and most are already written in the canon as known limits.

This release fixes every listed flake in the tests, never in the product, with one exception the owner approved on 2026-11-13: the gate's deadline goes from 3 s to 5 s (R8). It closes the scan's defects 2a.1-6 and 2a.8-13 and brings stale version text up to date. It ends with one full run, Rust and UI, with zero failures and no retries, on a clean committed tree. No ADR: no rule of `docs/canon/architecture.md` changes. `card-config` follows `team-config` (`docs/canon/hub-team.md` "Check"), and the intake refusal reuses the record's own predicate.

## Description and interactions

There are four lanes: T (test-engineer, tests only), U (ui-developer), R (rust-developer) and D (spec-writer, at ship). One gate, RH-G1, closes them.

### Decisions

- **D1. Tests, not the product, but one product bound.** Lane T widens a test's budget or retries only where the test's premise is a wall-clock bound the product does not promise. One product bound moves: the owner approved raising `GATE_DEADLINE` from 3 s to 5 s on 2026-11-13 (R8). It stays under the wrapper's 7 s limit and the hook's 10 s timeout, which stay as they are. No ADR: no rule of `docs/canon/architecture.md` names the deadline, and ADR-0039 does not either. It is canon (`gate.md` "Verdict"), rewritten in place.
- **D2. Only a no verdict by deadline is asked again.** One shared helper does it, as `gate_answer` does today in `cards.rs` and `card_trace.rs`. A test that asserts a no verdict (`gate.rs` `ac01`, `ac06`) never retries.
- **D3. No `.config/nextest.toml`.** No role owns that path, so there is no test group and no nextest retry; retries live in helpers.
- **D4. Dogfood tests by process.** `parity` (×3), `shards_repo` (×2), store `check_parity` (×3) and `check_rules` read the live tree and fail while a lane is mid-edit. They stay unchanged; RH-G1 runs on a clean, committed tree with no lane running.
- **D5. CR is refused at intake (2a.8), not normalised.** Today intake takes any character, while a record refuses `record::is_escaped` (CR, every other C0 but LF and TAB, DEL, C1, the bidi marks), so the approve stages and never applies. Refusing with that same predicate means "accepted ⇒ applies" holds by construction for all of them. Normalising CR would leave U+202E and the rest stalled, and the queue never rewrites agent text, which it escapes on output. It is one pure change in core `intake.rs`, so every face gets it as it gets every `FieldProblem`: CLI, stdio MCP, a hub's `/mcp`. No store or plugin change.
- **D6. `--login` takes no hyphen value (2a.9).** `allow_hyphen_values` goes, and a login starting with `-` is written `--login=-x`. The daemon passes the login in-process (`WorkspaceRequest`), never through argv.
- **D7. An ID-less document pairs by its path, for the card refresh only (2a.11).** An editorial apply vouches for the cards on it. A meaning approve neither raises nor writes a `rev:` on it: no ID, no revision, as today.
- **D8. A bad `[[card.split]]` row turns splitting off, and nothing else (2a.12).** The whole split is off, never a partial set, so a misspelt row cannot split silently short. `spec check` reports any `[card]` that does not read as `card-config`; a bad flow still stops the card surfaces, as today.
- **D9. Delivery reads one snapshot (2a.13).** Reaching the tab without `[roadmap]` is a design question (the Roadmap screen needs a plan), so it stays a known limit.
- **D10. Collision names count every holder the resolver counts (2a.10).** That is cheap: one more query in `collision_input`'s transaction.
- **D11. No plugin bump, queue schema step, backup format or served key.** No MCP description, `INSTRUCTIONS` or skill changes. This release changes no plugin file: `plugin.json` reads 0.45.0, or 0.46.0 once `usage-orphans` has landed. R8 leaves `gate.sh` and `hooks.json` unchanged. A lane-R change that would need one stops and goes back to the orchestrator.

## Data

### Lane T

| Test | Change and cause |
|---|---|
| CLI `show_bindings.rs` `show_bindings_without_code_read_as_show_and_the_note` | Before its loop: `sleep(PAST_RACY)` (2.1 s, as `store/tests/stat_key.rs`), then one warm `spec show MEC-STAMINA`. The read-trust racy rule (`store/src/write.rs`, `RACY_NS` 2 s) stores `taken_ns` in 14 `file_stats` rows once 2 s have passed since the fixture copy. Backdating mtime fails (`max(mtime, ctime)`). |
| CLI `gate_wrapper.rs` `a_stalled_spec_is_2_within_the_limit` | Ceiling LIMIT + 2.5 s (9.5 s; the hook's timeout is 10 s). Loaded runs took 7.67-7.73 s against 7.5. |
| CLI `gate_wrapper.rs` `a_stopped_hook_leaves_nothing_behind` | `spec` and the watchdog keep 1.1 s; the watchdog's orphaned `sleep 1`s (by design, `gate.sh`) get 3 s. |
| CLI `common/gate.rs` (new) | The helper below. |
| CLI `gate.rs` `ac05`, `ac11`, `ac12`, `ac14`, `a_large_write_is_read_to_its_end_and_decided`, `card_tasks_ct3_the_gate_reads_no_link`; `team_config.rs` `a1_a_bad_team_leaves_show_card_and_gate_as_they_were`; `task_next_roadmap.rs` `unfinished_predecessor_never_holds_a_task`; the `gate_answer` copies in `cards.rs`, `card_trace.rs`, `trace_truth.rs`, `card_mandate.rs` | Each ask of `spec gate` that expects a verdict goes through `until_verdict`. |
| http `spec_ids_hub.rs` `PUSH` | `["--push", "--push-deadline", "30"]` (was 3; the product default is 120). |
| http `tail.rs` `LIVE` | 3 s (was 1 s against a 250 ms poll; one run measured 1.03 s). |
| CLI `round.rs` `a_fifo_swapped_in_after_the_size_check_never_holds_the_open` | Rounds until both outcomes are seen or 60 s pass, then fails naming its counts (one run had 3 000 FIFO, 0 regular, 2 swaps); each round keeps the hang check. |
| CLI `read_trust.rs` `ac6g_…` | A failed premise (a step outlasting the 3 s interval) runs the scenario once more from a fresh place; a state assertion never retries. |
| CLI `verify_tests_lock.rs` `failing_test_removes_evidence` | The time-out half's `[verify] timeout` 5 (was 1; the hanging test still hangs past it). |

```rust
// crates/specengine-cli/tests/common/gate.rs
pub const VERDICT_TRIES: usize = 5;
/// A no verdict by deadline: stdout holds `"reason":"deadline"` or stderr `no answer within`,
/// the binary's and the wrapper's forms (`gate_answer`'s test today); no other no verdict.
pub fn starved(stdout: &str, stderr: &str) -> bool;
/// Calls `ask` until its answer is not `starved`, at most VERDICT_TRIES times; the last answer.
pub fn until_verdict<R>(ask: impl FnMut() -> R, streams: impl Fn(&R) -> (&str, &str)) -> R;
```

**A load run** ([measure] in lane T): the test binary is built first (`--no-run`). Then `sysctl -n hw.ncpu` copies of `perl -e 'alarm 120; exec "yes"'` start, and the named test runs `cargo nextest run -p <crate> --test <file> <filter> --retries 0` repeatedly within those 120 s. Every run must be green. The counts go in "Implementation". Load always ends by itself; no background process is left.

### Lane U

| Surface | Change |
|---|---|
| `styles/app.css` | `.evidence .pairs dd { white-space: pre-wrap; overflow-wrap: anywhere; }`, the quotes only; the eight other `.pairs` lists unchanged |
| `inbox/ProposalCard.tsx` `QuestionLead` | options empty: `AnswerSections`' working answer and price of another answer after why it matters, outside `AskedFold`, in place of `OptionsSection`; the fold keeps the text, the pair and the why |
| `inbox/PairView.tsx` `Side` | `side.path === null`: no `inbox.pair.noEvidence` line |
| `tree/ProposalsPanel.tsx`, `health/LeftRegion.tsx`, `palette/Palette.tsx`, ProposalCard's others list | a held gist (`leadingGist`) as `<bdi>{shownSlot(gist)}</bdi>`, as `inbox/ProposalList.tsx`; else `summaryOf` as before; one shared component |
| `doc/DocBody.tsx` `DocMeta`, `trace/CardsBlock.tsx` | the row of the resolved holder: its canonical ID (`holder.id`), an ID-less holder its path as the row names it; never `node.id === docRef` |
| `roadmap/RoadmapChain.tsx` (`chain.ts`) | the cut's label outside every lane head (`LANE_HEAD`): above the heads or at the rule's foot |

No new catalog key and no served key.

### Lane R

- **Intake (R1).** `question_problem`, `discrepancy_problem` and `decision_problem` judge each field a record carries: `text`, `summary`, `evidence[i].{file, qpath, lines, observed, documented}`, `options[i].{label, effect, price}`, `working_answer`, `price_of_other`, `why`. Each is judged at its place in input order, after its cap, with `record::refused_char`, and is refused exit 1 as `agent-intake.md` "Rules" does: `text: holds U+000D: a decision record never carries it`. `gist` and `matters` keep their own rules. A departing accept's `--summary`, `--observed` and `--documented` follow the same rule.
- **CLI (R2).** `main.rs` `Command::Workspace` `login`: `#[arg(long, value_name = "L")]`. `spec workspace --login --json` prints clap's `a value is required for '--login <L>' but none was supplied` and exits 2.
- **Raise (R3).** When neither parse's document (ord 0) has an `id:`, the editorial card refresh (`Revisions::card_refresh`, `within_moved`) pairs them, named by the file's path. The pair never reaches `meaning_changes`' raise (step 6).
- **Card config (R4).** `CardConfig { flows, splits, split_problem: Option<ProjectError> }`: a split row's error, or the listing check's, sets `split_problem` and leaves `splits` empty. `card_from_toml`'s signature is unchanged and it still returns `Err` for a flow's error, so `delivery.rs`, `card_counts.rs` and `workspace_toml.rs` compile and read unchanged. A split (CLI, route, MCP) given `split_problem` refuses with `<config>:<line>: <message>`, exit 2, 503, or an error result. `spec check`: `CheckSetup.card: Option<Finding>`, beside `team`.

```toml
[[card.split]]
kind = "mechanic"
disciplnes = ["copy"]
```

```text
error  specengine.toml:63: card-config: unknown field `disciplnes`, expected `kind` or `disciplines`
```

Subject `card`, one finding (`card_from_toml`'s first error), whatever `[check] mode` says; the 38th and last of `CHECK_CODES`.

- **Queue (R5).** One deferred read transaction over the queue, as `SqliteIndex::snapshot` (T3) does for the index, holds delivery's `list_existing_cards`, `events_of_types` and the items read. No schema change.
- **Collisions (R6).** `CollisionRows.outside_ids: BTreeSet<String>`: each ID that a node outside the universe holds (a Tier 3 file, a generated file, a node with no kind), read in `collision_input`'s transaction. In `Names::of`, an ID is shared when it has more than one universe holder, or one universe holder and is in `outside_ids`.
- **Gate deadline (R8).** CLI `gate.rs`: `GATE_DEADLINE` is `Duration::from_secs(5)` (was 3). What follows it moves with it: the stdin, git and queue steps; the causes `no answer within 5 s` and `not read to its end within 5 s`; the backstop (`main.rs`, the deadline + 0.4 s), now 5.4 s. That stays below the wrapper's 7 s (`gate.sh` `limit=7`), which stays below the hook's 10 s (`hooks.json`). A gate it cannot judge holds a write up to 5.4 s, not 3.4 s.

## Rules and edge cases

- No test is deleted, `#[ignore]`d, `.skip`ped or given a budget past its product bound (the wrapper's ≤ 9.5 s, under the hook's 10 s).
- `until_verdict` returns any answer that is not `starved` at once, a refusal included. Five `starved` answers return the fifth, and the test then fails on it as before.
- R1: WHEN a field is over its cap and holds a refused character, the cap answers, in input order. LF and TAB pass. An item stored before this release still stages and never applies: `reject:` settles it (known limit, kept).
- R3: a document that gains or loses its `id:` in the change pairs nothing (the names differ). Sections are unchanged.
- R4: a bad flow and a bad split row together give the flow's error (as today, stopping the surfaces) and one finding. `PLAIN_FLOW` with a split row is a `split_problem` (no flow lists a discipline). `--staged` and `--changed` judge `card-config` on the checked side alone, as `team-config`. Being an error, it blocks a commit as a bad `[team]` does; the gate never reads it (ADR-0039).
- R5: with no card, only the card read runs (DA-10's counts: cards 1, events and items 0).
- R8: the deadline + 0.4 s SHALL stay below the wrapper's limit, and that below the hook's timeout; past 6.6 s both move, a plugin change and the owner's call. RH-T3's `until_verdict` stays: a starved answer gets rarer, not impossible.
- R6: an ID held only outside the universe changes nothing. Mark step 4's words stay those of `collision-pass.md` "Mark", with its `spec check` clause kept true: the developer reads whether `id-taken` counts Tier 3 and generated holders and words it to match.
- Lane U touches no file of `delivery-timeline`'s (`roadmap/{RoadmapDelivery,RoadmapTimeline}.tsx`, `roadmap/{timeline,delivery}.ts`, `api/*`, `mocks/delivery.ts`, `i18n/*`) and adds only its own rules to `app.css`.

## Acceptance criteria

A browser look is marked **[look]** (the orchestrator's, in Chrome), a load run or a timing **[measure]**. Every other criterion is a test that the named mutation turns red. Code comments cite `docs/features/release-hardening.md` "Acceptance criteria" with an ID, never "Data", which compaction drops (`doc_pointers`).

**Lane T**

- [ ] RH-T1 — **show_bindings.** The wait and the warm read are in place, and a [measure] load run passes 10 of 10. Mutation, red on every run: the wait and the warm read removed, with a 2.1 s pause between the first case's `spec show` and its `--bindings` read (the test still sees a write).
- [ ] RH-T2 — **gate_wrapper.** The budgets from Data are in place, and a [measure] load run passes 5 of 5 for each test. Mutations, each red: `gate.sh`'s limit raised from 7 s to 10 s; the watchdog left alive after the hook stops.
- [ ] RH-T3 — **No verdict by deadline, asked again.** A test in `gate.rs` drives `until_verdict` with scripted answers: deadline, deadline, verdict gives 3 calls and the verdict; a bad-`[gate]` no verdict gives 1 call; five deadlines give 5 calls and the fifth answer. Every Data user calls it, and `grep` shows that `ac01`, `ac06` and no copy of `gate_answer` keep their own loop. A [measure] load run passes 5 of 5 for each user. Mutations, each red: `starved` true for any exit 2; `VERDICT_TRIES` 1.
- [ ] RH-T4 — **Push deadline.** `PUSH` is 30, and `spec_ids_hub` `ac1_*` and `ac2_*` pass 5 of 5 in a [measure] load run. No mutation: neither test is about the deadline.
- [ ] RH-T5 — **Tail.** `LIVE` is 3 s, and `a_last_event_id_above_the_highest…` passes 5 of 5 in a [measure] load run. Mutation, red: the tail never polls again after its first read.
- [ ] RH-T6 — **FIFO.** The loop from Data, 5 of 5 in a [measure] load run. Mutation, red: the round file opened without `O_NONBLOCK` (the hang check).
- [ ] RH-T7 — **ac6g.** A forced premise failure (a 3.1 s sleep on the first try only, test-local) passes on the second try; a failure on both tries fails naming the premise. Mutation, red: `IndexTrust` walking on every read.
- [ ] RH-T8 — **RM-0063's rest.** `failing_test_removes_evidence` passes 5 of 5 in a [measure] load run. Mutation, red: a timed-out node keeping its block. `drift_dedup` `concurrent_raises_store_one_item` stays unchanged: the store's 5 s busy timeout is a product limit (`agent-intake.md` "Dedup"). `worktree_git` `new_until_kills_a_slow_git_at_the_deadline` and `serve.rs` `d4_…` were each seen once and never reproduced, so they stay unchanged; if one fails in RH-G1, lane T takes it.

**Lane U**

- [ ] RH-U1 — **Quotes keep their line feeds.** A test reading `app.css` finds Data's rule. [look]: a three-line quote shows three lines in the Inbox list and on both pair sides. Mutation, red: the rule removed.
- [ ] RH-U2 — **An option-less gist question shows its working answer.** The working answer, with its price when held, follows why it matters, outside the fold. "No options attached." is gone, and the fold never repeats the working answer. A gist question with options and an item without a gist look as before (PQ12). Mutations, each red: the working answer only in the fold; drawn in both places.
- [ ] RH-U3 — **A fileless pair side.** A side with `path: null` shows no "No evidence cites this file"; a side with a path and no resolving position still shows it. The gone-side tests (the ID heads it once) pass. Mutation, red: the line keyed on `held.length` alone.
- [ ] RH-U4 — **A gist as the lead draws it, everywhere.** On each of the four surfaces, a gist holding U+202E and ESC shows `\u{202e}` and `\u{1b}` inside a `bdi`, and a summary-only entry looks as before. Mutation, red: any one surface back on raw `summaryOf`.
- [ ] RH-U5 — **Opened by an alias.** With `docRef` an alias, the holder `MEC-STAMINA` and the trace row `MEC-STAMINA`, `DocMeta` shows the row's rollup lozenge, not "not known", and `CardsBlock` shows its cards; opened by the canonical ID, it reads the same. Mutation, red: `node.id === docRef` in either.
- [ ] RH-U6 — **The MVP cut's label in its own place.** `RoadmapChain.test.tsx` pins the label outside the lanes' `[0, LANE_HEAD)`. [look]: 1920x1080, dark, Russian, the longest lane title after the cut, and 1100x800; no text overlaps. Mutation, red: the label at `top: 0` again.
- [ ] RH-U7 — **UI gates.** UI lint, `tsc`, `pnpm test` and `pnpm build` pass, as do `policy.test.ts` and `words.test.ts`. Lane U leaves `en.ts` and `ru.ts` unchanged. First-load JS is within 845 kB and at most 0.3 kB over `delivery-timeline`'s recorded figure; the figure is recorded.

**Lane R**

- [ ] RH-R1 — **Refused at intake.** `propose question` with `text` `"a\r\nb"` exits 1 `text: holds U+000D: a decision record never carries it`; nothing is stored and no ID is taken. Refused alike, each at its own field: a question's `options[1].label` holding U+202E, a discrepancy's `evidence[0].observed` holding U+0007, a decision's `why` holding U+000D, and a departing accept's `--summary` holding U+000D. `ask_question` on stdio and on a hub's `/mcp` gives the same line. An LF-only item approves and its record is written. Mutations, each red: `evidence[i].observed` left unchecked; the predicate `== '\r'`.
- [ ] RH-R2 — **`--login` takes no flag.** `spec workspace --login --json` exits 2 naming `--login`, with stdout empty. `--login alice --json` and `--json --login alice` print the JSON with alice's `default`, and `--login=-x` asks for `-x`. Mutation, red: `allow_hyphen_values` back.
- [ ] RH-R3 — **An ID-less document refreshes its cards.** The fixture has `docs/notes/plain.md` (no `id:`) and `C-0001` targeting it by path. An `--editorial` approve of an update to it writes one `card.refreshed` naming the path, and `spec card show C-0001 --marks` reads no suspect. A meaning approve exits 0, writes no `rev:` and leaves the card suspect. Mutations, each red: pairing by ID only; the path pair passed to the raise.
- [ ] RH-R4 — **A bad split row: a finding, splitting off, nothing else.** On Data's example, `spec check` prints the one `card-config` line. A bad `[[card.flow]]` is reported alike, and a good `[card]`, or none, gives none; `CHECK_CODES` has 38 entries. With the row: `spec card list`, `show`, `new` and `move`, `spec gate`, `spec workspace`, `spec delivery`, `spec card counts`, `list_cards` and the Board's card routes answer byte for byte as with the row removed. `spec card split` (dry run too) exits 2 `<config>:<line>: <message>`, the split route answers 503 and `split_card` returns an error result. `spec init --pack` of a preset holding the row still refuses. Mutations, each red: split errors joined to `[card]`'s again; the finding dropped; `init` not judging the split.
- [ ] RH-R5 — **Delivery reads one snapshot.** Store test: after the handle's card read, a second connection commits a `card.accepted` and a question naming the card. The handle's events and items reads show neither, and a new handle shows both. DA-10's seam counts are unchanged; [measure] DT3's figure is re-measured, still ≤ 1.5 s. Mutation, red: each read on its own transaction.
- [ ] RH-R6 — **Collision names count every holder.** The fixture has a live non-feature `R-01` also held in a Tier 3 file, a live non-feature `R-02` also held in a generated file, and a live feature `feat/R-97` also held only in an archived document. `spec collisions --json` makes `R-01` and `R-02` W1 (no name, unpaired, noted) and names `feat/R-97`. For every name the read prints, `spec show <name> --json` returns exactly one node. [measure]: `specengine-eval scale` `reads.collisions` stays under 2 s at 1k, 10k and 100k, the planted twins are as recorded (25, 25, 0), and the medians are recorded. Mutation, red: `outside_ids` not counted.
- [ ] RH-R7 — **Rust gates.** `cargo nextest run` is green for `specengine-core`, `-store`, `-cli`, `-mcp` and `-http`, and eval `doc_pointers`, `ui_policy`, `anonymity` and `workspace_names` pass. This release changes no plugin file: `plugin.json` and `PINS` are as `usage-orphans` leaves them (0.45.0 before it, 0.46.0 after), and `plugin_files.rs` is green. Its lanes leave `fixtures/daemon-keys.json` unchanged.
- [ ] RH-R8 — **The gate's deadline is 5 s.**
  - `GATE_DEADLINE` is 5 s. `spec gate --stdin` with stdin held open exits 2 with `not read to its end within 5 s`, in at least 5.0 s and under 5.5 s.
  - `tests/gate.rs` words its two deadline lines through `GATE_DEADLINE.as_secs()`, no longer the literal `3 s`.
  - A test reads `limit=` from `plugin/specengine/hooks/gate.sh` (7) and `timeout` from `hooks/hooks.json` (10). It asserts `GATE_DEADLINE` + 400 ms < the limit < the timeout.
  - RH-T3's [measure] load runs pass at 5 s.

  Mutations, each red: `GATE_DEADLINE` at 3 s (the 5 s line); at 7 s (the order).

**Lane D**

- [ ] RH-D1 — **Versions.** `hub-deploy.md` "Image" names the version `.env.example` holds at ship. `card-mandate.md` "Known limits" and `CLAUDE.md` describe the hub's upgrade as from 0.41.0 (schema 12, format 6) to the current build (schema 14, format 7), a dump first, pointing at `hub-ops.md` "Upgrade". `grep -n "to 0.38" CLAUDE.md docs/canon/*.md` is empty.
- [ ] RH-D2 — **Closed limits leave the canon.** Every row of "Canon at ship" is done. `grep` finds none of: `pairs nodes with an ID only`, `A typo in a row`, `Intake takes CR`, `are no holders`, `No shared snapshot`, `can overlap the next lane`, `Four surfaces draw a gist`, `No options attached`, `node.id === docRef`, `takes hyphen values`.
- [ ] RH-D3 — **RM-0063 closed** (`status: shipped`, `shipped:`), with each entry's disposition: fixed by RH-T*, a product limit, or not reproduced in RH-G1.
- [ ] RH-D4 — **The check.** `export index`, then `check`: 0 errors. Every edited canon file is within its cap, and `CLAUDE.md` ≤ 16 384 B.

**Gate**

- [ ] RH-G1 — **One clean full run.** On the owner's commit that holds every lane and the canon edits, `git status --porcelain` prints nothing and no `cargo`, `rustc`, `nextest` or `node` process of another lane runs. Then, in order, with nothing else running:
  - `cargo nextest run --workspace --retries 0 --no-fail-fast`: 0 failed, and skipped equals the `#[ignore]` count (86 at the scan; this release adds none);
  - UI lint and `tsc`;
  - `pnpm test`: 0 failed, and `ui/src` holds no `.skip(`, `.only(` or `.todo(`;
  - `pnpm build`;
  - `export index`, then `check`: 0 errors.

  The commit, counts and durations are recorded in "Implementation". A failure goes to its lane (within its three iterations), the owner commits, and RH-G1 runs whole again.

## Out of scope

Each is the owner's decision or a later item:
- **The pair side's found flag** (2a.7): a served field, so a plugin bump and a hub upgrade (`agent-intake-documents.md` "Pair").
- **The collision-pass flood and its absolute cap of 200** (`collision-pass.md` "Known limits").
- **The hub's upgrade from 0.41.0, the tags v0.42.0-v0.45.0 and the push of `main`** (`hub-ops.md` "Upgrade", `card-mandate.md` "Known limits").
- **Gate 11b-3 and the pipeline trial** (`08-roadmap.md` Phase 7).
- **RM-0039 and RM-0001** (`docs/roadmap/`).
- **The Delivery tab without `[roadmap]`** (D9; `spec-delivery.md` "Known limits").
- **The store's 5 s busy timeout:** two fresh index builds can exit 2.
- **The scan's minor items**, not taken:
  - the tile and the bar count cancelled subtasks differently;
  - a split's `name_problem` echo keeps raw line breaks;
  - card-context's "over the room" note names `--budget` unasked;
  - Retry fails after a redeploy until a reload;
  - the hub's blob-compare warning reaches stderr only;
  - `one_line` flattens only CR and LF (`spec-cli-read.md`).
- **Owner's housekeeping:** `stash@{0}` (RM-0055's work in progress) and the never-run 10k differential (`eval/tests/store_reads_scale.rs`, ignored).
- **Already accepted:** Vite's chunk-size warning (`ui-languages.md` "Size").

## Implementation

To be filled in after implementation. The plan follows.

### Roles and files

| Lane | Role | Files |
|---|---|---|
| T | test-engineer | CLI tests: `common/{mod,gate}.rs`, `show_bindings.rs`, `gate_wrapper.rs`, `gate.rs`, `cards.rs`, `card_trace.rs`, `trace_truth.rs`, `card_mandate.rs`, `team_config.rs`, `task_next_roadmap.rs`, `round.rs`, `read_trust.rs`, `verify_tests_lock.rs`; http `tests/{spec_ids_hub,tail}.rs` |
| U | ui-developer | `styles/app.css`, `inbox/{ProposalCard,PairView}.tsx`, the shared summary component (`inbox/`), `tree/ProposalsPanel.tsx`, `health/LeftRegion.tsx`, `palette/Palette.tsx`, `doc/DocBody.tsx`, `trace/CardsBlock.tsx`, `roadmap/{RoadmapChain.tsx,chain.ts}` |
| U | test-engineer | their `*.test.tsx`, the `app.css` rule test; RH-U1-U7's mutations and figure |
| R | rust-developer | R1 core `intake.rs` (and the departing accept's check, if it does not use `discrepancy_problem`); R2 CLI `main.rs`; R3 core `raise.rs`, CLI `revisions.rs`; R4 core `card_toml.rs`, `check/mod.rs`, store `check.rs`, CLI `card.rs`, `init.rs`; R5 store `queue/cards.rs` and the items read, CLI `delivery.rs`; R6 store `read.rs`, core `collisions.rs`; R8 CLI `gate.rs` (the constant alone) |
| R | test-engineer | core, store, CLI, MCP and http tests for RH-R1-R8, their fixtures; R8 in CLI `tests/gate.rs` (lane T's file too: one change, after lane T's) |
| D | spec-writer | "Canon at ship" |

### Work order

1. **Now, beside `delivery-timeline`'s end:**
   - Lane T, which shares no file with it.
   - Lane U (Rules' last line).
   - Lane R: R1, then R3, R6, R4 and R8, one at a time. A broken build stalls every lane's `cargo`. None of them touches `delivery-timeline`'s files: CLI `src/{lib,main,subset,delivery}.rs`, core `src/delivery.rs`, http `src/app.rs`. R8 edits CLI `src/gate.rs` alone, never `main.rs`'s backstop. An item that finds it needs one waits.
2. **After `delivery-timeline` is committed:** R2 (`main.rs`), then R5 (`delivery.rs`).
3. **Tests and reviews:** lane T's pass first, then each U and R item's tests as it lands, overlapping. Each lane gets its review, and at most three iterations.
4. **Lane D:** "Canon at ship", then RM-0063.
5. **The gate:** the owner commits, then RH-G1 runs. When it is green: `status: shipped`, compaction.

### Canon at ship

Most of these files sit within 100 B of their cap: "net ≤ 0" means the file does not grow.

| File | Edit |
|---|---|
| `agent-intake.md` "Rules" step 2 | the record's refused characters, per field (≤ +150 B) |
| `gate.md` "Verdict" | `GATE_DEADLINE` (5 s), the backstop at 5.4 s, and the order 5.4 s < 7 s < 10 s. "Hook" and "Known limits" keep the 7 s and 10 s. Net ≤ +120 B (424 B free) |
| `round.md` "Known limits" | the CR line: stored before this release, `reject:` settles. Net ≤ 0 |
| `hub-department.md` "Read" | `--login=VALUE` for a hyphen login |
| `spec-trace.md` "Suspect", "Known limits" | the path pair; the line removed. Net ≤ 0 |
| `card-split.md` "Config", "Known limits"; `card-flows.md` "Config" | the split off alone, `card-config`; the typo line removed. Net ≤ 0 |
| `spec-check.md` "Rules", `spec-check-process.md`, `hub-team.md` "Check" | 38 codes, `card-config`. Net ≤ 0 but `spec-check-process.md` |
| `spec-delivery.md` "Reads", "Known limits" | one transaction; the line removed. Net ≤ 0 |
| `collision-pass.md` "Universe", "Known limits" | outside holders counted; the line removed. Net ≤ 0 |
| `plain-questions.md` "UI", "Known limits" | the option-less lead; two lines removed |
| `agent-intake-documents.md` "Pair" | quotes keep line feeds; no line on a fileless side |
| `doc-view.md` "Known limits", `spec-roadmap-screen.md` "Layout", "Known limits" | rows by the holder; the label's place; the lines removed |
| `hub-deploy.md` "Image", `card-mandate.md` "Known limits", `CLAUDE.md` | RH-D1 |
| CLI, core and store `README.md` (10 236-10 238 of 10 240 B) | nothing added; a pointer only by replacing text |
| `RM-0063`; `CLAUDE.md` "State", 08 Phase 7 | closed; `release-hardening` shipped, once `delivery-timeline`'s ship has compressed "State" |
| this spec | compacted to ≤ 3 KB |

### Working answers

**The owner's:** R8, `GATE_DEADLINE` from 3 s to 5 s (2026-11-13).

**The orchestrator's:**
- the scope and lanes;
- D1-D4, D9 and D11;
- R5 only after `delivery-timeline` commits;
- the dogfood tests by process;
- RH-G1's form.

**The spec-writer's:**
- D5: refuse, by the record's predicate, for the reasons given there.
- D6: drop `allow_hyphen_values`.
- D7: path pairing for the refresh only, never a raise.
- D8: the whole split off; `card-config` covers any `[card]`, an error as `team-config`.
- D10: count, not a known limit, because it costs one query in a transaction already open.
- RH-U1: a scoped selector.
- RH-U4: one shared component, so a sixth surface cannot drift.
- RH-U6: the label above the heads or at the foot, the developer's pick.
- RH-T8: the verify time-out 5, and the dispositions of RM-0063's other entries.
- R2: waits too, because `main.rs` is `delivery-timeline`'s.
