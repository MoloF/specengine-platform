---
class: spec
status: draft
scope: [docs, crates/specengine-model, crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, crates/specengine-import, crates/specengine-eval, plugin, scripts, ui]
ref: RM-0129; the owner's word of 2026-11-19, wish 3 ("a must") and word 4 (English), and of 2026-11-20 (08 s2 Phase 7, "Owner's word, 2026-11-19"); the analyst's breakdown of 2026-11-19, slice 3 (G1-G7, sharpened here as GU1-GU16); the owner's decisions relayed 2026-11-20 (English, embedded, never copied, a Documents shelf, no project needed, a tool on stdio and the hub, a route, a freshness warning)
adrs: [ADR-0085]
---

# specengine-guide: SpecEngine's own guide, embedded and served beside every project (RM-0129)

## Why

The owner, 2026-11-19: "Instructions on using SpecEngine itself are a must — for example, a description of how this or that feature works and what it solves, so that when it is implemented in some project there is a full knowledge base and an effective distribution during development that one can refer to and keep, instead of keeping in one's head what should go where."

No such guide exists:
- **The canon** is written for SpecEngine's developers. It is dense, by subsystem, and tied to code.
- **The root `README.md`'s Status** was stale for months.
- **The plugin's eight skills** tell an agent how to call tools, not where a project's things go.

So a project adopting SpecEngine has nothing it can cite for "where does a decision go", "how does a department's rule reach a task", or "what may an agent write".

This slice writes the guide, one page per topic. It ships the guide inside every build, so a project reads the guide of the version it runs, and serves it read-only beside the project, never inside it (ADR-0085).

## Description and interactions

**What a person sees.**
- **Documents** gets a second shelf, **SpecEngine guide**, next to the project's documents. It lists the topics, each with a title and a one-line summary.
- **A topic** opens read-only at `#/<p>/documents/guide:<topic>`, its rules anchored. A "Copy citation" act copies `guide:<topic>#<ID>`.
- **A citation link.** A `guide:<topic>#<ID>` written in a card, a document or a proposal links to that rule when the served guide has the topic.
- **The page text** is English as served, never translated; only the chrome is (ADR-0072).

**What an agent sees.**
- **The `get_guide` tool**: `{topic?, rule?}`. With no `topic` it returns the index; with one, the page or one rule. It is on stdio and on a hub.
- **The resource template** `guide://{topic}`.
- **The plugin's `read-spec` skill** names the tool: "where does X go: `get_guide`".
- **No unasked reads.** The guide is never in a bundle, a card's context or a task package unasked, so W is unchanged.

**What a terminal sees.** `spec guide [TOPIC[#ID]] [--json]`. It works in any directory, with or without a project, and writes nothing.

**Neighbours.**
- **The library** (`spec-library.md`) gains the shelf, never library rows: the guide is not the project's corpus.
- **`spec init`** (`presets.md`) still writes exactly one file.
- **This repository's `spec check`** judges the pages as its canon, and adds `summary-stale` (a new code) for freshness.
- **The plugin's skills** stay, and point to the guide's topics.

**Ten people at once.** The guide is static data compiled in. It takes no project turn, holds no cache and never grows, so a page read on a hub never waits behind an agent's write.

**Language** (the owner's decision): the guide is English (ADR-0024). A consumer's own documents may be in any language.

**Assumptions**, each the owner's to overrule:
- **G-A1. The guide is canon of this repository**, not a new class; its pages count against the 12 288 B Tier 2 cap.
- **G-A2. Freshness compares `reviewed:` dates**: no git, deterministic, a warning in every mode. A page-level comparison also flags edits unrelated to the summary, giving some false warnings, cleared by reviewing the page.
- **G-A3. Only a workspace build carries the guide** (`cargo install --git`, the hub image). A build without `docs/guide/` fails, naming it. Publishing to crates.io is not a goal.
- **G-A4. The pages are written by the spec-writer, in this slice**, once the command, tool and route names are fixed. From then on, a feature's spec-writer updates the page with the feature: the text for `.claude/agents/spec-writer.md` is in "Implementation", for the owner to apply.
- **G-A5. Topics are file stems**, listed in a fixed order (`GUIDE_ORDER`). A new page joins that order in the same change.

## Data

### The pages: `docs/guide/<topic>.md`

There are ten topics, listed in this order (`GUIDE_ORDER`):

| Topic | Title | Covers |
|---|---|---|
| `start` | Adopting SpecEngine | `spec init`, `[paths]`, `[ids]`, the reading protocol, where everything lives |
| `records` | Records, the spec tree and canon | the four classes, IDs, `parent:`, `{#ID}` sections, `rev`, canon "how it works now" |
| `features` | Features, criteria and decisions | feature specs, `scope = "feature"` criteria, decision records, `canon:`, superseding |
| `proposals` | Proposals and the one writing door | propose, review, approve, confirm on a hub, why nothing else writes a spec file |
| `cards` | Cards, departments and flows | `[[card.flow]]`, disciplines, departments, hand-off, split, `T-` tasks |
| `practices` | Department practices | where practices live, `disciplines:`, how they reach a card and a task, leads' zones |
| `auto` | The `[auto]` mode | the rule, packs, what is delegated, what never is |
| `markers` | Code markers and the lock | `@implements`, tiers, `spec.lock`, drift, `spec verify` |
| `hub` | The hub | logins, `[team]`, confirm, the door, `/mcp`, upgrades and dumps |
| `documents` | The documents library | Documents, tags, decisions behind a page, the guide shelf |

**A page:**

```markdown
---
class: canon
tier: 2
scope: [guide]
owner: owner
reviewed: 2026-11-20
summarises: [docs/canon/proposal-apply.md, docs/canon/hub-confirm.md#the-rule]
---

# Proposals and the one writing door

One paragraph: what this solves. (`spec guide`'s index line is this paragraph's first sentence.)

## What it solves
## When to use it
## How
### CLI
### MCP
### Screen
## Where it lives
## What not to do

### Never write a spec file by hand from an agent {#GD-PROPOSALS-ONE-DOOR}
...
```

**Rules for a page:**
- Each rule is a heading `{#GD-<TOPIC>-<NAME>}`, under this repository's new `[ids]` prefix `GD = { kind = "guide-rule", shape = "name" }`.
- The five `##` parts are required, in that order; `### CLI`, `### MCP` and `### Screen` are written when the feature has them.
- At most 12 288 B, the Tier 2 cap.
- Every command, tool and route a page names exists (GU9).

**`[classes] canon`** gains `optional = ["summarises"]`. The index gains a shard, `docs/index-guide.md`, claiming `docs/guide/*.md` (the generator entry's `writes` and `shards`). `docs/README.md` "Classes and where they live" lists `docs/guide/`.

### Front-matter `summarises`

**The key.**
- Core `TYPED_KEYS` 34: `summarises`, `KeyType::List`, appended after RM-0128's `disciplines`.
- Each entry is a root-relative `.md` path with an optional `#anchor`.
- It is never counted (`node-revision.md`); a mistyped value gives `frontmatter-type`.

**The finding.** `summary-stale` is a new check code (`CHECK_CODES` 41, after RM-0128's 40). It is a warning, never an error, and changes no exit. Its subject is `summarises`, at the key's line, one per entry:
- `` summarises `<entry>`: names no walked canon document ``
- `` summarises `<entry>`: reviewed <its date>, after this page's <own date> ``

The second is given only when both dates read as `YYYY-MM-DD` and the named page's is strictly later. An anchor is checked as a `canon:` anchor (`spec-check.md` "Path-form `canon:`"); a missing one is the first message. The rule is generic: any project may use the key.

**The index.** `INDEX_FORMAT` 20, no table: a stored row keeps the key in `extra` until it is re-read. `zone-index`'s draft takes the next number.

### The embedded guide (CLI)

- **`crates/specengine-cli/build.rs`** reads `../../docs/guide/*.md` (each `cargo:rerun-if-changed`, the directory too) and generates `GUIDE: [GuidePage; N]`, in `GUIDE_ORDER`.
  - Each entry: `{topic, title, summary, summarises, reviewed, rules: [{id, title, line}], text}`, with `text` the page without its front-matter.
  - A missing directory, a page with no front-matter, a topic absent from `GUIDE_ORDER` (or the reverse), or a rule ID repeated in the guide is a build error naming the file.
- **`spec guide [TOPIC[#ID]] [--json]`**, CLI `guide.rs`. No discovery, no `HOME`, no data directory, no index, no git: it runs in an empty directory and writes nothing.
  - No `TOPIC`: the index. Text lines `<topic> | <title> | <summary> | rules <n>`; JSON `{topics: [{topic, title, summary, rules}]}`.
  - `TOPIC`: the page. Text: the `text` bytes. JSON `{topic, title, summary, summarises, reviewed, rules, text}`.
  - `TOPIC#ID`: that rule section's bytes, its nested sections included. JSON `{topic, rule: {id, title, line}, text}`.
  - Exit 1 for an unknown topic, `` spec: no guide topic `<t>`; topics: start, records, … ``, and for an unknown rule, `` spec: no rule `<ID>` in guide topic `<t>`; rules: … ``.
  - Exit 2 for usage, or a REF with a space or a control character.
  - The 40 000-character cap (`OUTPUT_CAP_CHARS`) never binds, since a page is at most 12 288 B; the test holds it.

### MCP

- **`get_guide {topic?: string, rule?: string}`**, on stdio and in `HUB_TOOLS`, which goes from 18 to 19. `rule` without `topic` is `isError`. Its content is byte-equal to `spec guide …`, and its `structuredContent` to `--json`.
  - It needs no project: on stdio it answers with no `specengine.toml`.
  - On a hub it takes no turn, like `tools/list`. It is not in `HUB_READ_TOOLS`, since it reads no index.
  - The description stays within 2 048 B and names "where does X go".
- **Resource template** `{"uriTemplate": "guide://{topic}", "name": "guide", "mimeType": "text/markdown"}`, read with or without a project. `resources/list` is unchanged.
- **Instructions.** `INSTRUCTIONS` and `HUB_INSTRUCTIONS` name the tool within their caps; words elsewhere make room.
- **Plugin 0.49.0, MINOR** (`plugin-files.md` "Changing it": a tool added). RM-0128 takes 0.48.0. `read-spec` names `get_guide`, and "all thirty-three named" holds.

### The daemon

- **Routes:** `GET /api/guide` (the index) and `GET /api/guide/:topic` (`:topic` may be `<topic>#<ID>` percent-encoded). They answer `spec guide [REF] --json` less its final LF, with no turn.
  - An unknown topic or rule is a 404 with the CLI's line; another query is a 400.
- **On a hub**, behind the fence, for any login.
- **Key sets:** `GuideIndex` (1), `GuideTopic` (4), `GuidePage` (7), `GuideRule` (3), `GuideRuleRead` (3).

### The UI

- **The Documents section** gets a View group, "Project documents | SpecEngine guide" (`aria-pressed`), kept per project for the page's life.
- **The guide shelf** lists the topics from `GET /api/guide` (title, summary, "N rules"). A topic route `#/<p>/documents/guide:<topic>` renders the page through `src/markdown/`, read-only:
  - its rules are anchors `#GD-…`;
  - each rule heading has "Copy citation", which copies `guide:<topic>#<ID>`;
  - there is no edit or propose act.
- **`src/markdown/`:** a `guide:<topic>[#<ID>]` token whose topic is in the held guide index is a link to that route. Nothing else is linked.
- **States:** a skeleton, an `ErrorPanel` with Retry, and a daemon before this slice (404), shown as the library shows it.
- **Strings:** chrome `guide.*` in `en.ts` and `ru.ts`; the page text as served, `lang="en"`.

## Rules and edge cases

- WHEN no project is found, `spec guide`, `get_guide` and the routes SHALL still answer.
- WHEN the guide is read, nothing SHALL be written: no data directory, index, queue or file.
- WHEN a project document cites `guide:<topic>#<ID>`, `spec check` SHALL give no finding: the form is not in the project's `[ids]`, and a `[[guide:…]]` is `skipped`, as a `project:` reference is (`spec-cli-graph.md` "Parents and links").
- WHEN a summarised page is reviewed the same day as the guide page, no warning SHALL be given (strictly later only).
- WHEN a page names a command, tool or route that does not exist, GU9 SHALL fail. This is a test, not a check finding, since the check knows no command list.
- The guide SHALL never be part of a project's walk, index, map, library rows, bundle, card context or task package.
- One build SHALL give one guide: byte-equal output at any root, `HOME` and directory.

### Known limits

- A crates.io package would carry no guide (G-A3).
- Freshness is by page and date: an unrelated canon edit asks for a review (G-A2).
- A project cannot add its own pages to the shelf. Its own canon serves that role in the library.

## Acceptance criteria

- [ ] GU1 — **Embedded, any directory.**
  - The release `spec`, run in an empty temp directory with a scratch `HOME`: `spec guide` lists the ten topics in `GUIDE_ORDER`, with titles, summaries and rule counts, exit 0.
  - `spec guide proposals` prints `docs/guide/proposals.md` without its front-matter, byte for byte.
  - `spec guide proposals#GD-PROPOSALS-ONE-DOOR` prints that section.
  - An unknown topic or rule exits 1 with the lines of "Data".
  - Mutation: reading `docs/guide` from disk at run time turns it red (the test renames the source directory before running the binary).
- [ ] GU2 — **Writes nothing, needs nothing.** With `HOME` unusable and no `specengine.toml` anywhere above, `spec guide` answers. The directory tree and the data directory are unchanged. Mutation: calling `discover` turns it red (exit 2).
- [ ] GU3 — **The embed equals the source.** A test compares `GUIDE` to `docs/guide/*.md`: order, topics, titles, summaries, `summarises`, `reviewed`, rule IDs and lines, and text. `build.rs` fails on a missing directory or an order mismatch. Mutation: a stale embed (one page edited in the test's copy without a rebuild) turns it red.
- [ ] GU4 — **Never inside a project.**
  - On `fixtures/spec-a`, the outputs of `spec check`, `spec map`, `spec library`, `spec tree` and `spec search guide` equal the base commit's.
  - A document citing `guide:records#GD-RECORDS-ONE-FILE` and `[[guide:records#GD-RECORDS-ONE-FILE]]` gets no new finding.
  - `spec init --pack` still writes one file.
  - Mutation: guide pages added to the project's walk turns it red.
- [ ] GU5 — **MCP.**
  - `get_guide`, with no arguments, with `{topic}` and with `{topic, rule}`: content and `structuredContent` byte-equal to the CLI's, on stdio with and without a project, and on a hub over `/mcp/<slug>`.
  - On a hub its slow line has `wait_ms=-` (no turn).
  - `rule` without `topic` is `isError`.
  - `HUB_TOOLS` is 19, the description within 2 048 B, the output schema pinned, and `guide://proposals` reads the page.
  - Mutation: the hub tool taking the turn turns it red.
- [ ] GU6 — **The daemon.**
  - `GET /api/guide` and `/api/guide/proposals` equal the CLI's JSON; `…/nope` is a 404 with the CLI's line.
  - On a hub any login reads them, and the request line has no turn.
  - The key sets are pinned in `fixtures/daemon-keys.json`.
- [ ] GU7 — **LG-8.** `crates/specengine-eval/tests/anonymity.rs` `cyrillic_exemption_is_exactly_the_two_fixture_directories_and_the_catalog` stays green with `docs/guide/**` and RM-0128's preset example practices present. Mutation: a Cyrillic line planted in a guide page turns it red.
- [ ] GU8 — **`summarises` and freshness.**
  - `summarises` is typed (`TYPED_KEYS` 34) and never counted; `INDEX_FORMAT` is 20.
  - A page whose summarised canon is reviewed a day later gives one `summary-stale`; on the same day, none; an entry naming no canon document gives the other message.
  - `spec check`'s exit is unchanged in `enforce`.
  - Mutations: `>=` for `>` turns it red; so does raising the finding to an error.
- [ ] GU9 — **Pages are true** (eval `tests/guide_pages.rs`).
  - Each of the ten pages has the five parts in order and at least one `GD-<TOPIC>-…` rule; its `summarises` is non-empty and resolves; it is at most 12 288 B.
  - Every backticked `spec <command>` resolves in the CLI's command list, every backticked tool name is in `tools/list`, and every `/api/…` route is in the daemon's route table.
  - Mutation: a page naming a removed command turns it red.
- [ ] GU10 — **This repository at ship.** `spec check` is clean with no `summary-stale`. `docs/index-guide.md` lists the ten pages, and the root index stays under its cap.
- [ ] GU11 — **W unchanged.** No `spec bundle`, `get_card {context}`, `get_task` or `get_node` output on `fixtures/spec-a` contains guide text, and the outputs are byte-equal to the base commit's.
- [ ] GU12 — **The plugin.** 0.49.0; `read-spec` names `get_guide`; `plugin_files.rs` holds "thirty-three named" and the pins; `INSTRUCTIONS` within its cap.
- [ ] GU13 — **The hub image.** Its build context carries `docs/guide/`, pinned in `tests/deploy.rs`, and the built image's `spec guide` lists the ten topics.
- [ ] GU14 — **UI.**
  - The shelf lists the topics, and a topic route renders the page read-only.
  - "Copy citation" copies `guide:<topic>#<ID>`.
  - A `guide:` token in a card's goal links to the route only for a known topic.
  - The none, failure and 404 states; `en` and `ru` chrome keys; the page `lang="en"`; a lazy chunk; `policy.test.ts` green.
  - Mutation: linking an unknown topic turns it red.
- [ ] GU15 — **Determinism.** `spec guide --json` and every topic's text are byte-equal across two directories, two `HOME`s and with or without a project.
- [ ] GU16 — **No turn, no growth.** On a hub, 10 logins reading `get_guide` and `GET /api/guide/<topic>` back to back for 120 s beside 8 agents' writes: every guide read under 1 000 ms, its lines with no turn, and the daemon's RSS slope reported. This is the `library_common/` harness, as `doc-library` DL15.

## Out of scope

- Copying the guide into projects, or `spec init` writing pages.
- Loading the guide into contexts unasked; a guide search.
- Replacing the plugin's skills; translating the guide.
- A project's own pages on the guide shelf.
- Publishing to crates.io.

## Implementation

To be filled in after implementation. The plan:

| Lane | Files |
|---|---|
| `rust-developer` | core `front_matter.rs` (`summarises`, `TYPED_KEYS` 34), the check's `summary-stale` (`CHECK_CODES` 41); model; import's typed keys; store `INDEX_FORMAT` 20; CLI `build.rs`, `guide.rs`, the command; MCP the tool, the template, the texts; http the two routes; `plugin/` (0.49.0, `read-spec`); `scripts/deploy/` (the build context) |
| `spec-writer` | the ten pages under `docs/guide/`, written once the names above are fixed; `specengine.toml` (`[ids] GD`, `[classes] canon optional`, the shard); `docs/README.md`; then the canon at ship |
| `ui-developer` | `documents/` (the shelf, the topic route), `src/markdown/` (`guide:` links), `api/`, `provisional.ts`, mocks, `i18n/` |
| `test-engineer` | CLI (`tests/guide.rs`), MCP, http, eval (`guide_pages.rs`, LG-8, the GU16 run), `fixtures/daemon-keys.json`, `tests/deploy.rs`, UI tests |

**Order:**
1. the Rust lane's command, tool and routes (GU1, GU5, GU6 fix the names);
2. the pages;
3. the UI on the mocks;
4. the measures.

**Text for the owner's `.claude/agents/spec-writer.md`** (G-A4), to add under "When you are called after implementation":

> 5. If the feature changes what a project does or where something lives, update its page under `docs/guide/` (its parts, its `GD-` rules, `summarises`, `reviewed`) in the same change. A `summary-stale` warning names a page to review.

**Canon at ship:**
- `spec-guide.md`, completed from "Data";
- `spec-check.md` (41 codes), `node-revision.md`;
- the core and store READMEs;
- `mcp-read.md`, `hub-mcp.md` (19 tools), `plugin-files.md` (0.49.0);
- `spec-library.md` (the shelf), `docs/README.md`, `architecture.md#distribution` (a pointer to `spec-guide.md`).
