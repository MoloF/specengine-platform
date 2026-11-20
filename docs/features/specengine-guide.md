---
class: spec
status: shipped
shipped: 2026-11-20
scope: [docs, crates/specengine-model, crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, crates/specengine-import, crates/specengine-eval, plugin, scripts, ui]
ref: RM-0129; the owner's word of 2026-11-19, wish 3 ("a must") and word 4 (English), and of 2026-11-20 (08 s2 Phase 7, "Owner's word, 2026-11-19"); the analyst's breakdown of 2026-11-19, slice 3 (G1-G7, sharpened here as GU1-GU16); the owner's decisions relayed 2026-11-20 (English, embedded, never copied, a Documents shelf, no project needed, a tool on stdio and the hub, a route, a freshness warning)
adrs: [ADR-0085]
---

# specengine-guide: SpecEngine's own guide, embedded and served beside every project (RM-0129)

## Why

The owner, 2026-11-19: "Instructions on using SpecEngine itself are a must — for example, a description of how this or that feature works and what it solves, so that when it is implemented in some project there is a full knowledge base and an effective distribution during development that one can refer to and keep, instead of keeping in one's head what should go where."

The canon is written for SpecEngine's developers, and the plugin's skills say how to call tools, not where a project's things go. So a project had nothing to cite for "where does a decision go" or "what may an agent write". This slice wrote the guide, one page per topic, embedded it in every build, so a project reads the guide of the version it runs, and serves it read-only beside the project, never inside it (ADR-0085).

## Data

The shapes are canon now: `docs/canon/spec-guide.md`. The pages: `docs/guide/<topic>.md`, ten topics in `GUIDE_ORDER`.

### Front-matter `summarises`

`spec-guide.md` "`summarises:` and `summary-stale`": `TYPED_KEYS` 34, never counted, `CHECK_CODES` 41, `INDEX_FORMAT` 20; a write reads each entry's file (`spec-check-write.md`).

### The embedded guide (CLI)

`spec-guide.md` "The embed" (`crates/specengine-cli/build.rs`, the definitions of `text`, `summary`, a rule's section, the build errors) and "`spec guide`" (forms, JSON, exits).

### MCP

`spec-guide.md` "MCP": `get_guide`, `guide://{topic}`; `HUB_TOOLS` 19 (`hub-mcp.md`); the texts (`mcp-read.md`, `plugin-files.md`).

### The daemon

`spec-guide.md` "The daemon": `GET /api/guide[/{topic}]`, no turn, the five key sets.

### The UI

`spec-guide.md` "The UI": the shelf, the topic and rule routes, `guide:` citations.

## Rules and edge cases

- WHEN no project is found, `spec guide`, `get_guide`, `guide://` and the routes SHALL still answer; nothing SHALL be written.
- WHEN any document cites `guide:<topic>#<ID>` or `[[guide:<topic>#<ID>]]`, it SHALL be plain text in every project, this repository included (the reference grammar's rule (7)): no link, edge or finding.
- WHEN a summarised page is reviewed the same day as the guide page, no warning SHALL be given (strictly later only).
- The guide SHALL never be part of a project's walk, index, map, library rows, bundle, card context or task package; one build SHALL give one guide.

## Acceptance criteria

Final run 2026-11-20: 6 541 of 6 543 tests (a moved pin, since fixed, and the known `autopilot rn4_until` load flake); the UI's 6 491; every named mutation red.

- [x] GU1 — **Embedded, any directory.** In an empty directory with a scratch `HOME`, `spec guide` lists the ten topics in order, a topic prints its page byte for byte, `proposals#GD-PROPOSALS-ONE-DOOR` its section; unknown topic or rule exit 1. Mutation: reading `docs/guide` at run time, caught by renaming the source and by a `sandbox-exec` run denying it.
- [x] GU2 — **Writes nothing, needs nothing.** With `HOME` unusable and no `specengine.toml` above, it answers; the tree and data directory unchanged. Mutation: calling `discover`.
- [x] GU3 — **The embed equals the source**, `crates/specengine-cli/build.rs` against `docs/guide/*.md`: order, topics, titles, summaries, `summarises`, `reviewed`, rule IDs and lines, text; a missing directory or order mismatch fails the build. Mutation: a stale embed.
- [x] GU4 — **Never inside a project.** On `fixtures/spec-a`, `check`, `map`, `library`, `tree`, `search guide` equal the base commit's; both citation forms add no finding, also in a project declaring its own `GD`; `spec init --pack` writes one file.
- [x] GU5 — **MCP.** `get_guide` byte-equal to the CLI on stdio with and without a project and over `/mcp/<slug>`; `wait_ms=-` on a hub; `rule` without `topic` refused; `HUB_TOOLS` 19; the description within 2 048 B; the schema pinned; `guide://proposals` reads the page. Mutation: the hub tool taking the turn.
- [x] GU6 — **The daemon.** `/api/guide` and `/api/guide/proposals` equal the CLI's JSON; `…/nope` a 404 with its line; any login on a hub, no turn; the key sets pinned.
- [x] GU7 — **LG-8** green with `docs/guide/**` and the preset practices. Mutation: a Cyrillic line in a page.
- [x] GU8 — **`summarises` and freshness.** Typed, never counted, `INDEX_FORMAT` 20; one `summary-stale` a day later, none the same day, the other message for no canon document; the exit unchanged. Mutations: `>=` for `>`; the finding an error.
- [x] GU9 — **Pages are true** (eval `guide_pages.rs`): five parts, a `GD` rule, `summarises` resolving, within 12 288 B; every `spec` command, tool and `/api/` route named exists; the cross-citations resolve. Mutation: a removed command.
- [x] GU10 — **This repository at ship**: `spec check` clean, no `summary-stale`; `docs/index-guide.md` lists the ten pages.
- [x] GU11 — **W unchanged**: `bundle`, `get_card {context}`, `get_task`, `get_node` on `fixtures/spec-a` byte-equal to the base commit's, no guide text.
- [x] GU12 — **The plugin** 0.49.0; `read-spec` names `get_guide`; "thirty-three named"; `INSTRUCTIONS` within its cap.
- [x] GU13 — **The hub image**: its context carries `docs/guide/` (`tests/deploy.rs`); `image.yml`'s step checks the built image's `spec guide` lists every page, on the real image at the release run.
- [x] GU14 — **UI**: the shelf, the read-only topic, "Copy citation", `guide:` links only for a known topic, the none, failure and 404 states, `en` and `ru` keys, `lang="en"`, a lazy chunk, `policy.test.ts`. First load 838 633 B: 6.4 kB of headroom, so the next UI slice plans lazily. Mutation: linking an unknown topic.
- [x] GU15 — **Determinism**: byte-equal across two directories, two `HOME`s, with and without a project.
- [x] GU16 — **[measure] No turn, no growth from the guide.** Two runs, 10 logins beside 8 agents' writes, ~1.27 M route and 1.27 M tool reads each: worst 118 and 113 ms, every guide line with no turn. RSS slope 1.68 MiB/min over the second half with the 8 writers (0.64-0.73 the whole run), 0.0 with readers alone: the write path's growth, not the guide's, taken up by RM-0132.

## Out of scope

- Copying the guide into projects, or `spec init` writing pages; loading it into contexts unasked; a guide search.
- Replacing the plugin's skills; translating the guide; a project's own pages on the shelf; publishing to crates.io.

## Implementation

| Module | What it does |
|---|---|
| model `grammar.rs` | rule (7): a guide citation is no reference |
| core `front_matter.rs`, the check (`engine.rs`, `spec_graph.rs`) | `summarises` typed; `summary-stale`; `summarised_reads` in a write's reads |
| store, import | `INDEX_FORMAT` 20; the typed key |
| CLI `build.rs`, `guide.rs`, `main.rs` | the embed, `GUIDE_ORDER`, `spec guide` |
| MCP `guide.rs`, `hub.rs`, `server.rs`, `resources.rs` | `get_guide`, `guide://`, the texts |
| http `app.rs` | `/api/guide[/{topic}]` |
| `plugin/`, `scripts/deploy/`, `.github/workflows/image.yml` | 0.49.0, `read-spec`; `docs/guide/` in the image; the GU13 step |
| UI `src/guide/`, `src/markdown/guide.ts`, `documents/`, `i18n/` | the shelf, the topic view, citations |
| `docs/guide/`, `specengine.toml`, `docs/README.md` | the ten pages; `[ids] GD`, `summarises` optional, the shard |

**Readings and deviations** (the orchestrator's):
- The build script is `crates/specengine-cli/build.rs`; it is stricter than written: no `reviewed:`, no H1, no paragraph after it, or a non-list `summarises:` fails the build too.
- Exit 1 prints no JSON, `--json` too.
- A `guide:` citation is plain text, not a "skipped" link: rule (7) holds in every project, so a look-alike inside one gets no ADR-0009 error, and this repository's cross-citations are checked by GU9.
- `INSTRUCTIONS` and `read-spec` were trimmed to name `get_guide` within their budgets.
- `get_guide`'s inputs are `["string", "null"]`.
- UI: the views live in `src/guide/`; a rule's route carries `%23`; rules are reached by route; a citation's rule is not checked in the UI (the page says when it lacks one); rule headings take a scroll margin.

**Release.** Plugin 0.49.0 (MINOR: a tool added), tag `v0.49.0`.

**Text for the owner's `.claude/agents/spec-writer.md`** (G-A4), under "When you are called after implementation":

> 5. If the feature changes what a project does or where something lives, update its page under `docs/guide/` (its parts, its `GD-` rules, `summarises`, `reviewed`) in the same change. A `summary-stale` warning names a page to review.
