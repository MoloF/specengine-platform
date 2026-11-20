---
class: canon
tier: 2
scope: [docs, crates/specengine-model, crates/specengine-core, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, ui]
owner: owner
reviewed: 2026-11-20
---

# spec guide: SpecEngine's own guide, served beside a project

ADR-0085, built by `specengine-guide` (RM-0129, plugin 0.49.0). Code: CLI `build.rs` (the embed), `guide.rs`; core `front_matter.rs` (`summarises`), the check's `summary-stale`; model `grammar.rs` (rule (7)); MCP `guide.rs`; http `app.rs` `guide`; UI `src/guide/`, `src/markdown/guide.ts`.

<a id="the-rule"></a>
## The rule

**What the guide is.** SpecEngine's user guide is canon of this repository: one page per topic, `docs/guide/<topic>.md`, in English (ADR-0024).
- **Each page** says what the feature solves, when to use it, how (CLI, MCP, screen), where it lives in a project, and what not to do.
- **Each rule** is a `{#ID}` section under this repository's `GD` prefix.
- **`summarises:`** names the canon pages the page summarises. When one of them was reviewed after the page, `spec check` warns `summary-stale`: a warning, never an error.

**How it is served.** Every build of `spec`, `specengine-mcp` and `specengine-http` embeds the pages at compile time, and serves them read-only, with no project and no turn:
- `spec guide [TOPIC[#ID]]`;
- the MCP tool `get_guide`, on stdio and on a hub, and the resource template `guide://{topic}`;
- `GET /api/guide[/{topic}]`;
- the "SpecEngine guide" shelf in Documents.

**What it never does.**
- It never enters a project: no project file, index row, check, map, library row or ID space holds it.
- `spec init` copies nothing.
- No bundle, card or task read includes it unasked, so W is unchanged (`documentation-system.md` §3).
- A project cites a rule as `guide:<topic>#<ID>`: plain text whatever the project's `[ids]` (model rule (7)), so the citation is no link, edge or finding.

## The pages

Ten topics, in `GUIDE_ORDER` (CLI `build.rs`): `start`, `records`, `features`, `proposals`, `cards`, `practices`, `auto`, `markers`, `hub`, `documents`. A new page joins that order in the same change.

```markdown
---
class: canon
tier: 2
scope: [guide]
owner: owner
reviewed: 2026-11-20
summarises: [docs/canon/proposal-apply.md#consent, docs/canon/hub-confirm.md#rules]
---

# Proposals and the one writing door

One paragraph; its first sentence is the topic's summary.

## What it solves
## When to use it
## How
### CLI
### MCP
### Screen
## Where it lives
## What not to do

### Never write a spec file by hand from an agent {#GD-PROPOSALS-ONE-DOOR}
```

- **Rules**: `{#GD-<TOPIC>-<NAME>}` headings, `[ids] GD = { kind = "guide-rule", shape = "name" }` here, unique across the guide.
- **Parts**: the five `##` in that order; `### CLI`, `### MCP`, `### Screen` where the feature has them. At most 12 288 B, the Tier 2 cap.
- **Names**: every command, tool and `/api/` route a page names exists (eval `tests/guide_pages.rs`, GU9). This repository's cross-citations between pages are checked there too, not by `spec check`.
- **The writer's duty**: a feature that changes what a project does, or where something lives, updates its page in the same change (parts, rules, `summarises`, `reviewed`).

## `summarises:` and `summary-stale`

- **The key**: core `TYPED_KEYS` 34, `List`, each entry a root-relative `.md` path with an optional `#anchor`; never counted (`node-revision.md`); a mistyped value is `frontmatter-type`. `[classes] canon` here lists it `optional`. Any project may use it.
- **The finding**, `CHECK_CODES` 41, a warning that moves no exit, subject `summarises`, at the key's line, one per entry in written order:
  - `` summarises `<entry>`: names no walked canon document `` (not walked or read, not canon, or an anchor it lacks, judged as a path-form `canon:`'s);
  - `` summarises `<entry>`: reviewed <theirs>, after this page's <own> ``, only when both read as `YYYY-MM-DD` and theirs is strictly later.
- **A write** reads each entry's file (core `summarised_reads`), so a change to a summarised page judges the summarising one again (`spec-check-write.md`).
- **The index**: `INDEX_FORMAT` 20, no table; a row stored by 19 keeps the key in `extra` until re-read.

## The embed

CLI `build.rs` reads `../../docs/guide/*.md` (`cargo:rerun-if-changed` for each and the directory) into `GUIDE: [GuidePage; 10]`, in `GUIDE_ORDER`: `{topic, title, summary, summarises, reviewed, rules: [{id, title, line}], text}`.
- `text` is every byte after the front-matter's closing `---` line; `title` (the first H1) and `summary` as written; `summary` runs to the first `.` followed by whitespace or the paragraph's end; a rule's `line` is its heading's line, its section running to the next heading of its level or above. Headings in fenced code are none.
- **A build error naming the file**: no directory; a page with no front-matter, no `reviewed:`, no H1, no paragraph after it, or a `summarises:` that is no list; a topic absent from `GUIDE_ORDER`, or the reverse; a rule ID repeated in the guide.
- Only a workspace build carries the guide; the hub image's context holds `docs/guide/` (`hub-deploy.md` "Image").

## `spec guide`

`spec guide [TOPIC[#ID]] [--json]`: no discovery, `HOME`, data directory, index, git or clock. It runs in an empty directory and writes nothing; one build gives byte-equal answers anywhere.
- No `TOPIC`: `<topic> | <title> | <summary> | rules <n>` a line; JSON `{topics: [{topic, title, summary, rules}]}`.
- `TOPIC`: the page's `text`; JSON `{topic, title, summary, summarises, reviewed, rules, text}`.
- `TOPIC#ID`: that rule's section; JSON `{topic, rule: {id, title, line}, text}`.
- **Exit 1**, nothing on stdout, `--json` too: `` spec: no guide topic `<t>`; topics: start, records, … ``; `` spec: no rule `<ID>` in guide topic `<t>`; rules: … ``. **Exit 2**: usage, or a REF holding a space or a control character. `OUTPUT_CAP_CHARS` never binds.

## MCP

- **`get_guide {topic?, rule?}`**, both `["string", "null"]`: content byte-equal to `spec guide …`, `structuredContent` to its JSON. `rule` without `topic` is refused, `isError`; an unknown topic or rule is `isError` with the CLI's line. It needs no project.
- **On a hub**, one of the 19 (`hub-mcp.md` "Tools"): no turn, as `tools/list`; not in `HUB_READ_TOOLS`.
- **Template** `{"uriTemplate": "guide://{topic}", "name": "guide", "mimeType": "text/markdown"}`, read with or without a project; `resources/list` unchanged.
- `INSTRUCTIONS` and `HUB_INSTRUCTIONS` name it (`mcp-read.md` "Texts"); `read-spec` says "where does X go in a project: `get_guide`" (`plugin-files.md`).

## The daemon

`GET /api/guide` and `GET /api/guide/{topic}`, `{topic}` one raw segment percent-decoded once (`<topic>%23<ID>` reads a rule): `spec guide [REF] --json` less its final LF. No project, turn or index; on a hub past the fence, any login's. An unknown topic or rule is a 404, a query, a bad `%` sequence or a refused REF a 400, each `{status, message}` with the CLI's line. Key sets `GuideIndex` (1), `GuideTopic` (4), `GuidePage` (7), `GuideRule` (3), `GuideRuleRead` (3), pinned in `fixtures/daemon-keys.json`.

## The UI

- **Documents** has a View group, "Project documents | SpecEngine guide" (`aria-pressed`), kept per project for the page's life. The shelf (`src/guide/GuideShelf.tsx`, a lazy chunk) lists the topics from `GET /api/guide`: title, summary, "N rules".
- **A topic** opens at `#/<p>/documents/guide:<topic>`, a rule at `…guide:<topic>%23<ID>` (one raw `#` in the hash), rendered read-only through `src/markdown/` (`GuideTopicView.tsx`, `lang="en"`): rule headings are anchors with a scroll margin, each with "Copy citation" (`guide:<topic>#<ID>`); a rule the page lacks says so. No edit or propose act.
- **Citations**: a `guide:<topic>[#<ID>]` token, standing alone and outside code, whose topic the served index holds links to that route (`src/markdown/guide.ts`); its rule is not checked there. Nothing else links. Standing alone is the grammar's on the left (rule (5)'s boundary before `guide`: no letter or digit of any script, `_ - . / :`), so where the daemon keeps a reference the UI links no guide; on the right it is `standsAlone`'s, as for cited IDs.
- **States**: a skeleton, an `ErrorPanel` with Retry, a daemon before this slice (404) as the library shows it. Chrome `guide.*`, `documents.shelf.*` in `en.ts` and `ru.ts`; the text as served.

## Citations

Model rule (7) (`crates/specengine-model/README.md` "Reference grammar"): a run right after `guide:<slug>#`, with the qualifiers' left boundary before `guide`, is no reference in any project, this repository included, nor is its tail: a `#ID`, then an `@rev`, joined as a reference's. The ID and the `#ID` are read by shape alone, never by `[ids]`, so any `#ID` of that shape goes, an alias's or a look-alike's too: `guide:x#R-1#OLD-5` drops `OLD-5`, which a plain `R-1#OLD-5` keeps as its own reference; after a space, `guide:x#R-1 S-2`'s `S-2` is one. So `guide:<topic>#<ID>` and `[[guide:<topic>#<ID>]]` make no link, edge or finding, and a look-alike letter inside one gets no ADR-0009 error.

One table of odd texts, 17 rows (`#`, `@`, `.`, `:`, `/`, `-` or `(` before `guide`, and the tail's forms), is checked by both sides: `fixtures/guide-citations.json` by the model's `tests/grammar.rs`, its byte-identical copy `ui/src/markdown/guide-citations.json` by `guide.test.ts`.

## Known limits

- A crates.io package would carry no guide: only a workspace build does.
- Freshness is by page and date: an unrelated edit of a summarised page asks for a review.
- A project cannot add pages to the shelf; its own canon does that in its library.
- A citation's rule is checked by `spec guide` and the page, not where it is written.
- The UI's right side is `standsAlone`'s, which differs from the grammar on two edges no table row holds: a `-` then a non-alphanumeric, and an `@` then a letter, right after the citation.

## Tests

CLI `tests/guide.rs`, `guide_project.rs`, `guide_freshness.rs`; core `guide_keys.rs`; MCP `mcp_guide.rs`; http `tests/guide.rs`; eval `guide_pages.rs` (GU9), `guide_soak.rs` (GU16), `anonymity.rs` (LG-8); UI `src/guide/`, `src/markdown/guide*.test.*`.
