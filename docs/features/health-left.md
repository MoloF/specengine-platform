---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-http, ui]
ref: health-left analysis 2026-10-09 (W1-W14, A1-A4, criteria; Q1, Q2 decided under the owner's pre-approval, review open); kind-amendment "Out of scope"; 06 s7; 07 s3; 08 s2 Phase 2
adrs: []
---

# Health left: unapplied amendments

## Why

An approved amendment leaves the queue adding only its own file; the node it amends stays as written until two updates absorb it (`docs/canon/proposal-amendment.md` "Unapplied"). Nothing lists such documents: `spec graph` walks from a REF, `spec inbox` reads the queue, the check reports defects, which a live amendment is not. Health's row 3 of "What is left" (06 s7) reads "Not measured yet". This slice adds one read-only corpus query, `spec health [--json]`, served as `GET /api/projects/:p/health`, and measures row 3 from it.

No general "What is left" engine (W1): rows 1-2 (the queue) and 7 (budgets) are measured by existing reads and stay; rows 4-6 (`@assumes`, unbound, drift) need Phase 3's markers and keep "Not measured yet: Phase 3", no Phase 3 slice named. Three states stay apart everywhere: measured (0 too), not measured, could not be measured; 0 only when measured. No ADR: the row is canon already (kind-amendment's W35; ADR-0008, ADR-0012, ADR-0033, ADR-0034).

## Description and interactions

- **Core**: the rule (below) over a walked `CheckInput`, never the index.
- **CLI** `spec health [--json]` (W4): global flags only; one library call, as every daemon read; uncut. Exit 0 answered, zero rows too; 2 could not run (usage, discovery, a config error, one `<config>:<line>: message` per cause, a walk gap); never 1.
- **HTTP** `GET /api/projects/:p/health` (W5): the CLI's JSON ("Data").
- **UI** (W11): "What is left" holds "Proposals in the queue" (rows 1-2, unchanged), then **"Unapplied amendments"** (row 3), then "Not measured yet" (rows 4-6, "Phase 3", no digit); row 7 stays Budgets. Row 3: its own read `getHealth`, boundary, skeleton (`aria-busy`, no digit), error (the message verbatim, a Retry of `getHealth` only). Measured: `.health-value` the row count, "over <documents> documents"; rows in the daemon's order: the path linked `sectionHash(p, "tree", path)`; title, class, status raw (ADR-0031); per link `name` linked `sectionHash(p, "tree", name)`, else `written` as text, with `linkStateLook(state)`'s label and icon and `reason` verbatim. Over 10 rows: 10 rendered, the rest not rendered until "Show all <n>" (as Findings, `ui-health`). Zero: "None: no live document of this tree declares an `amends` link.", never "nothing left". `notes`: the shared `Notes` list.
- **Refresh** (W12), as `useCheck` (`ui/src/api/queries.ts`): on entering Health; while it is on screen, after `proposal.applied` or a stream gap; on "Check again"; never on focus, reconnect, an interval, a stage or `task.*`; no `AbortSignal`. The home reads none.
- **Canon** (W14), at shipping: a new Tier 2 page `docs/canon/health.md`, the read, the route, the document growing by keys in Phase 3.

## Data

**`spec health --json`** (W7), keys in this order, every key present, absent `null`, empty `[]`:

```json
{"documents":41,"amendments":[{"path":"docs/decisions/DEC-0014.md","id":"DEC-0014","title":"Quay lights stay amber","class":"decision","status":"accepted",
"links":[{"type":"amends","origin":"frontmatter","at":null,"name":null,"written":"RULE-QUAY-LIGHTS","path":"docs/decisions/DEC-0014.md","line":8,"state":"dangling","reason":"..."}]}],
"notes":["docs/spec/locks/lock-c.md:4: frontmatter-yaml: <the parser's message>; not judged"]}
```

- `Health {documents, amendments, notes}`: `documents` = `spec check --json`'s `counts.documents` on the same tree (A4); `notes` without `note: `.
- `UnappliedAmendment {path, id, title, class, status, links}`: `path` root-relative; `id` the document node's Latin ID; `title` `title:`, else the first H1; `class`, `status` raw; the four `null` when absent (`class` too, a class-less project: working answer). `links`: its `amends` out links exactly as `spec show <path> --links --json` lists them in `nodes[0].links.outgoing` (A2, A3), the existing `ShownLink` (`docs/canon/spec-cli-graph.md` "spec show --links"), every state, never empty. The key is `links`: no `"amends"` literal outside core and model (`proposal_genre.rs`).

**Text**, uncut, one-lined, no colour or timing: per amendment `<path>[ | id <id>][ | class <class>][ | status <status>][ | <title>]` (absent parts omitted), then each link as `spec show --links` prints an out line, two spaces in; last, always: `spec health: <d> documents, <n> unapplied amendments`. stderr `note: <note>` each. Exit 2: stdout empty; a walk gap `spec: the walk is incomplete: <gap>: unapplied amendments not measured`, `<gap>` as `spec export index` words it.

```text
docs/decisions/DEC-0014.md | id DEC-0014 | class decision | status accepted | Quay lights stay amber
  out amends RULE-QUAY-LIGHTS | docs/decisions/DEC-0014.md:8 | dangling: ...
spec health: 41 documents, 1 unapplied amendments
```

**Route** `GET /api/projects/:p/health` (W5), past the fence; every answer `Cache-Control: no-store`, no `Access-Control-*`. 200: `spec --root R health --json`'s stdout less its final LF. Error bodies, two keys (`crates/specengine-http/README.md` "Answers and statuses"): 400 any query name (it takes none) or a bad `%`, naming it; 403 the fence; 404 an unknown slug or route (the route list gains `health`); 405 any method but GET, HEAD too, `Allow: GET`; 500 a caught panic; 503 exit 2 (a config `discover` refuses, a changed slug, a walk gap), `message` the CLI's stderr line(s) verbatim. It runs in the project's turn (`docs/canon/serve.md` "Concurrency"), writes nothing, emits no event, refreshes no index.

**UI** (`ui/src/api/`): provisional types citing this section (as `daemon-read`'s; the shipped spec keeps the heading); query key `["health", p]` (`useHealth`).

```ts
/** GET /api/projects/:p/health (= spec health --json); no AbortSignal, as getCheck */
getHealth(project: string): Promise<HealthReport>;
HealthReport { documents: number; amendments: UnappliedAmendment[]; notes: string[] }            // 3 keys
UnappliedAmendment { path: string; id, title, class, status: string | null; links: ShownLink[] } // 6
```

`fixtures/daemon-keys.json` gains `HealthReport` (3), `UnappliedAmendment` (6), `ShownLink` (9): 44 sets.

**Mocks** (W13; no clock, core's order): `normal` harbor-sim two (the decision above; a `spec` `draft` amending `RULE-BERTH-DRAFT`, resolved), ledger-api none; `empty` none; `large` 3 000 by path; `cannot-check` a 503 with a walk-gap message (working answer: both reads walk one tree); `error`, `slow` as every read.

## Rules and edge cases

- **Rule** (W2, kind-amendment's W35): WHEN a walked document passes `check::is_live` and `create::amended_refs` is non-empty, the read SHALL list it, whatever raised it (by hand too), its class or its links' states. Not listed: Tier 3 by status (a spec `shipped` or `abandoned`, a decision no longer `accepted`), `class: generated`, a document with only other link types, an amendment still a queue proposal (rows 1-2). No kind, class, status or link-type literal: `AMENDS` only through `amended_refs` (ADR-0008). Never blocks: exit 0 whatever is listed, no check finding, rule or gate (ADR-0012).
- **Input** (W6): `load_config`, `check_input` over `WorkingTree` (as `spec check` walks), `SpecGraph::new(&CheckInput, ..)` resolving as `show --links` does (`docs/canon/spec-cli-graph.md` "Input"). No database, index refresh, queue, git, clock or baseline; the data directory never resolved or created. Every Health region walks one tree the same way.
- **Walk gap** (W6): WHEN `check::walk_gap` names a gap, the command SHALL exit 2 naming the first by path, stdout empty, the route 503: never a partial list, never 0.
- **Broken front-matter** (W9): WHEN `front_matter_failure` names a finding for a walked file, the file SHALL not be judged: no row, one note `<path>:<line>: <code>: <message>; not judged`, exit 0, never refused (ADR-0012). A1: `amended_refs` of such a file is empty: unnoted, it would read as none. A mistyped `links:` reads as none too (`proposal-amendment.md` "Known limits").
- **Order** (W8): amendments by path, byte order; links in `show --links` order (written order here); notes by path; byte-identical for one tree and config, whatever the root or walk order.
- **"Old"** (Q1, W3): row 2 stays the queue's order (`ui/src/inbox/order.ts`: severity, `created_at` oldest first, ID); no age threshold, config key or clock.
- **Scope**: the served root's working tree only. Pilot B's own amendment vocabulary reads none until its migration maps it (Q2, Phase 5; ADR-0008 forbids a core fix).
- **Assumed**, tests pin them: A1 above; A2 `show --links` lists every `amends` value whatever its target's standing; A3 `SpecGraph` over `check_input` resolves as `show --links` over the index on a readable tree; A4 `counts.documents` comes from one `CheckInput`.

## Acceptance criteria

Tests on git copies of `fixtures/spec-a` (`-b`, `-c` where named), amendments written into the copies. M: a mutation that must turn it red.

rust-developer
- [ ] AC-R1 -- listed: a `spec` `draft` with `links: {amends: [RULE-STAM-REGEN]}`, a decision `accepted`, one without `status:`. Not listed: the spec `shipped`, `abandoned`; a decision `superseded-by`, `proposed`; a `class: generated` one; a document with only `depends_on` and a body `[[RULE-STAM-REGEN]]`; an amendment only proposed. M: `is_live` -> `!is_tier3_file` (generated listed); the live filter removed (shipped listed); any strong link type taken (`depends_on` listed).
- [ ] AC-R2 -- `links` equals, as JSON and in order, the `amends` entries of `spec show <path> --links --json` `nodes[0].links.outgoing`: a resolved ID, a hand-written alias (`written` raw), a dangling ID, a node in a Tier 3 file. M: unresolved dropped; `written` canonicalised.
- [ ] AC-R3 -- exit 0 with none and several; keys exactly and in order as "Data", absent `null`, empty `[]`; `documents` = `spec check --json` `counts.documents`; uncut, by path; the text of "Data", its last line with zero too. M: `notes` omitted when empty; rows sorted by title.
- [ ] AC-R4 -- an unreadable `.md` (mode 000) or a missing written root: exit 2, stderr naming the first gap, stdout empty; a broken front-matter: no row, one note `path:line: code: ...; not judged`, exit 0. M: the gap ignored and a list printed; the broken file skipped without a note.
- [ ] AC-R5 -- a fresh `HOME` holds no data directory after; `git status --porcelain` empty; queue events unchanged; two runs, and copies written in opposite orders elsewhere, byte-identical. M: an index refresh (creates `<slug>.db`); a `HashMap` order.
- [ ] AC-R6 -- 200 = `spec --root R health --json` less the final LF on spec-a (none), spec-a with amendments, spec-b, spec-c; a query name 400 naming it; HEAD, POST 405 `Allow: GET`; an unknown slug 404; an unknown route's 404 lists `health`; a walk gap 503, `message` the CLI's line; `no-store`; the fence's 403s. M: `limit` accepted; the route at `/left`; 200 `{"amendments":[]}` on a gap.
- [ ] AC-R7 -- `proposal_genre.rs`, http `no_domain.rs`, `door.rs` green. M: an `"amends"` literal in new CLI code.

test-engineer
- [ ] AC-T1 -- tests for AC-R1 to AC-R7, every mutation red.
- [ ] AC-T2 -- `daemon_keys.rs` regenerates the fixture from a spec-a copy holding an amendment, adding the three sets of "Data"; the three 41 pins (Risks 2) move to 44. M: a key dropped from the CLI struct, red in Rust and Vitest.
- [ ] AC-T3 -- `turn.rs`: a health request behind another read of its project waits; another project's does not. M: the handler outside the turn.
- [ ] AC-T4 -- `git diff --stat -- fixtures/spec-*` empty.
- [ ] AC-T5 -- in "Implementation", pilots unnamed: release build, both read-only, median of 5 wall times of `spec health --json`, `spec check --json`.

ui-developer
- [ ] AC-U1 -- `getHealth(project)` on `SpecEngineClient`; `HttpClient` GETs `/api/projects/<p>/health`; no `MISSING ENDPOINT` (`client.test.ts`); no `AbortSignal`. M: a signal passed.
- [ ] AC-U2 -- the types cite `docs/features/health-left.md` "Data" (`doc_pointers`); key records equal the fixture (`daemonKeys.test.ts`, "forty-four"). M: a key renamed.
- [ ] AC-U3 -- row 3 per "Description" in the daemon's order; `large` renders at most 10 rows until expanded. M: a client sort; all 3 000 rendered.
- [ ] AC-U4 -- loading: skeleton, `aria-busy`, no digit; failure (503, an older daemon's 404, status 0): message verbatim, Retry making exactly one `getHealth`, no digit in `.health-value`; zero its own text; a failing `getHealth` leaves the queue part and other regions; rows 4-6 no digit. M: `?? 0`; one boundary for both reads.
- [ ] AC-U5 -- refresh per W12; entering Health makes one `getCheck`, one `getInbox`, one `getHealth`; the home none. M: `refetchInterval`; a re-read on `proposal.staged`; the home calling it.
- [ ] AC-U6 -- mocks per W13; no product code compares a kind, class, status, state or link-type literal (`ui_policy.rs`, `policy.test.ts`). M: `link.type === "amends"` in `src/health/`.
- [ ] AC-U7 -- narrowed: "rows 3 to 6" -> rows 4-6; the two `kind-amendment` AC-U2 tests replaced; `ui-health` AC-01's counting stub adds `getHealth`; `pnpm lint` 0, `build`, `test` twice; still 17 packages. M: an 18th package.

spec-writer
- [ ] AC-S1 -- this draft; `proposal-amendment.md` "Unapplied" points at it.
- [ ] AC-S2 -- at shipping: `docs/canon/health.md` (W14); `proposal-amendment.md` "Elsewhere", "Unapplied" point at it; http README (endpoint row, route list, 44 sets, "Tests"), CLI README "Commands"; 07 s2, s3, 06 s7, 08 s2, `CLAUDE.md` State; dated amendment notes on `ui-health` AC-01, AC-04; no cap raised; `export index`, `check` clean. M: a type citing a missing heading turns `doc_pointers` red.

## Risks

1. Superseded working answer: `ui-health` "Why" kept `health` for Phase 3's composite; this slice opens it with one key, the canon page says it grows by keys.
2. Red by design, updated deliberately: http `daemon_keys.rs`'s 41-set pins (`ac09_the_daemons_key_sets_are_fixtures_daemon_keys_json`, `kind_decision_ac_t4_*`, `kind_amendment_ac_t4_*`); `daemonKeys.test.ts` "names exactly the forty-one types"; `HealthView.test.tsx` "holds no digit in the value of rows 3 to 6 and of task W" and the two "unapplied amendments (AC-U2 of kind-amendment)" tests; `ui-health` AC-01's counting stub. The unknown-route text (`app.rs` `unknown_route_of`) changes; `methods.rs`, `tasks.rs` match only its start.
3. Latency: entering Health runs two whole-tree walks and the inbox, serialized by the turn (AC-T5).
4. Caps: CLI README 10 208 of 10 240 B, http README 10 053 (its "40 key sets" stale): the shipping pass trims as it adds.
5. Also: an older daemon's 404 is a failure, never 0; a canon declaring `amends` never reaches Tier 3 and stays listed; a hand edit fires no event.

## Out of scope

Rows 4-6 producers; an age threshold or clock; rows 1-2 server-side, inbox or `queueOrder` changes; 06 s7's metrics; `spec compact`; an absorb action, command or link from the row; an MCP tool or resource, plugin, skill, SessionStart summary; the task package listing them; pilot B's relation; the Home summary; other worktrees; a check finding; `null` keys for rows 4-6; the live check. W10: no ADR, `specengine.toml` key, index or queue schema change, dependency, package, or `fixtures/spec-*` change.

## Open

1. Q1 (W3), Q2 decided under the owner's pre-approval; the owner reviews them, W1-W14 and this draft's working answers: `class` `null` when absent; the types citing "Data"; the `cannot-check` mock's 503; the text form.
2. The owner's manual check, written at shipping.

## Implementation

Not started. Planned: core, CLI, http, UI, `fixtures/daemon-keys.json`; the store's loaders reused; MCP, model, import, eval, plugin, hook, CI untouched.
