---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, scripts, ui]
owner: owner
reviewed: 2026-11-20
---

# Card practices: a department's agreed rules, fed to its cards and tasks

ADR-0084, built by `department-practices` (RM-0128, plugin 0.48.0). Related: the bundle's layer 10 (`spec-cli-bundle.md` "Layers"), `card-context.md`, `task-package.md`, `card-tasks.md`, `spec-library.md`. Code: core `practice.rs` (`judge_disciplines`, `kept_disciplines`, `practice_rules`, `rule_count`, `cited_in`, `declared_disciplines`, the caps), the check's `practice-discipline`; store `practices.rs` (`practices_of`), `queue/cards.rs` (`cards_listing_task`); CLI `practice.rs` (`CardPractice`, `Cited`, `PracticeAsk`, `read`), `bundle.rs`, `card.rs`, `package.rs`, `review.rs`, `library.rs`; MCP `mirror.rs`; http `app.rs`; UI `src/practices/`.

<a id="the-rule"></a>
## The rule

- **A practice** is a live, non-generated document whose typed `disciplines:` lists card disciplines (`card-flows.md` "Config"). Any class may be one; a Tier 3 or generated document is none.
- **Its rules** are its direct child `{#ID}` sections; with none, the document itself is its one rule.
- **Where they live.** In a project, practices sit under the default walk, `docs/spec/practices/<trade>/…`.
- **Who reads them.** A card's context, and a `T-` task's package through the cards whose `tasks` list the task, carry the practices of their disciplines. A card or proposal read that asks for practices also serves the references its texts cite.
- **How.** Read per call from the index after its freshness step, in one snapshot. Nothing is cached, and every list is capped.
- **On a hub**, a page's card read trusts. MCP `get_card` walks at most once a call, for its mark (a cancelled card, or one with no targets and no live parent, has none), which is trace data (`spec-read-freshness.md` "Routes and tools"); its context bundle and practices read run inside `trusting`.
- **Nothing gates.** No card, task or artefact is gated, refused or judged (ADR-0012); the one finding is a warning.
- **Who writes them.** Anyone proposes; a lead confirms inside its `[zones]`; the owner confirms anywhere (`hub-team.md`). `[auto]` delegates none when the practices' zone is in `exclude_zones`.

## `disciplines:`

Typed (`TYPED_KEYS` 33, `List`), never counted (`node-revision.md`). Entries are judged in written order, each by the first rule it breaks; a passing entry is kept while fewer than 16 (`DISCIPLINES_MAX`) are. `practice-discipline` is a warning, subject `disciplines`, at the key's line:

```text
disciplines: entry <n> is no discipline name ([a-z0-9_-], 1 to 64 bytes)
disciplines: entry <n> repeats entry <m>
disciplines: entry <n> is past the 16th kept; a practice keeps 16
disciplines: `<d>` is in no `[[card.flow]] disciplines`; no card receives this practice
```

- **The last message** is given only when `[card]` reads and declares a flow; under `PLAIN_FLOW`, or a bad `[card]` (its `card-config`), none. An unlisted discipline is still kept.
- **Contracts.** A closed `[classes]` contract without the key gives `key-extra`.
- **The index.** `INDEX_FORMAT` 19 adds the table `disciplines (file_id -> files ON DELETE CASCADE, ord, discipline, PRIMARY KEY (file_id, ord)) STRICT`, indexed by `discipline`. `update` writes every file's kept entries.
- **The lookup.** Store `practices_of(&[&str]) -> Vec<PracticeRow {path, discipline}>`: live, non-generated files, by path.

## Caps

| Cap | Value | What |
|---|---|---|
| `DISCIPLINES_MAX` | 16 | entries a document keeps; a bundle's `--discipline`s |
| `PRACTICES_MAX` | 32 | practices listed per card or task, by path |
| `PRACTICE_RULES_MAX` | 128 | candidates per practice; the rest count in the tail's `more` |
| `CITED_MAX` | 64 | distinct references looked up |
| `PRACTICES_CHARS_MAX` | 16 384 | JSON characters a read's practices may take, entries kept in order while they fit |
| `CITED_CHARS_MAX` | 16 384 | the same for its citations |

The notes, alike in a card, a package and a review:
- past `PRACTICES_MAX` or `PRACTICES_CHARS_MAX`: `` practices: <k> more practice documents of <discipline, …>, from <first left out> ``;
- past `CITED_CHARS_MAX`: `` cited: <k> more cited IDs past 16384 characters, from <written> ``.

At typical sizes the counts bind first. At the extremes `get_card` measures 467 556 characters (`mcp-read.md` "Size").

## The bundle

`spec bundle … --discipline D…`, which is `BundleRequest.disciplines` and MCP `get_context_bundle {disciplines}`. The daemon's `/bundle` takes none.
- **Layer 10**, `practices` (`spec-cli-bundle.md` "Layers"), heading `## Practices`, form header and text, `via` `null`. A node that is already a candidate in an earlier layer stays there.
- **Fitting** is unchanged, so a rule that does not fit becomes a tail ID or is counted in `more`.
- **JSON.** `layers` has ten keys; `practices` is `[]` without disciplines, and then the body, the text and `bundle_hash` are unchanged.
- **Refusals**, exit 2: `` `--discipline`: `<d>` is no discipline name ``; `` `--discipline` is given <n> times; a bundle takes at most 16 ``.

## The card

Two keys, after `context` and before `notes` (`Card` 38, `cards.md`). Both are `null` unless asked: by `spec card show C --practices`, by `--context` (which implies it), by MCP `get_card {context: true}`, or by the daemon's `GET …/cards/:id?practices=true`, which every card read of the UI asks.
- **`practices`**: `[{name, title, path, disciplines, rules}]` (`CardPractice`), the practices of the card's discipline by path, at most 32. `name` is the map's canonical name and `rules` the rule count. It is `[]` when none matches or the card has no discipline.
- **`cited`**: `[{written, name, title, path}]` (`Cited`), from core's body-reference scan.
  - It scans `title`, `goal` and `criteria`; each item's `summary`, `gist`, `matters`, `working_answer`, `why` and `decision_note`; and, with `--history`, each event's `payload.note`.
  - The first 64 distinct references are looked up in first-seen order, unresolved ones counted among them. Each resolves as `spec show` resolves a REF, to its first holder; an unresolved one is left out.
  - `written` is the occurrence's exact bytes.

**Text**, printed before `context.parent:`: `practices: <name>, …` or `-`; then `cited: <name> | <title or -> | <path>`, a line each, or `cited: -`. Both count in a context's room.

**Index reads.** Plain `show` reads no index. When asked, a card with no discipline and nothing cited opens none. A failed read is the note `practices: not read: <cause>`, with both keys `null`, never exit 2.

**Context.** `--context` asks its bundle with the card's discipline, so `context.bundle` equals `spec bundle <REFs> --budget N --discipline <d> --json`.

## The task and the proposal

- **The package** (`task-package.md`):
  - `practices`, its 28th key, after `bundle`: the practices of the disciplines of every card whose `tasks` list the task (store `cards_listing_task`, cancelled cards too), by path, each once, at most 32; `[]` when no card lists it.
  - `bundle.disciplines`: those disciplines, sorted, at most 16; `bundle_hash` is made with them.
  - The brief's `Practices: <n>` section, before Bindings, printed only when not empty.
  - `schema_version` stays 1. A hub runs no `T-` flow.
- **The proposal.** `cited` is the last key of every proposal document, since MCP shares one schema.
  - It is filled by `spec review --json`, `get_proposal`, `review_proposal` and the daemon's `proposals/:id`, and is `null` elsewhere.
  - It scans `rationale`, `summary`, `gist`, `matters`, `working_answer`, `why`, the option labels and the answer, by the card's rules.
  - The text prints no `cited:` line.

## Library, presets, MCP

- **The library.** Rows gain `disciplines`, their 23rd key (`spec-library.md`), and a Discipline facet.
- **Presets.** Four English examples with `PRA` rules, copied or proposed by a person and never written by `spec init`: `scripts/presets/practices/gamedev/{art-3d-models,development,sound,production}.md`. This repository does not walk them (`[paths] exclude`). `gamedev.toml` comments `[ids] PRA`, `[zones] practices = ["docs/spec/practices/**"]` and `[auto] exclude_zones = ["practices"]` (`presets.md`).
- **MCP.** `get_card`, `get_task` and `get_context_bundle` name practices within 2 048 B; `get_card`'s first paragraph is shortened to fit, 2 042 B. The schemas gain `CardPracticeShape` and `CitedShape`; `HUB_TOOLS` is unchanged. The plugin is 0.48.0, and so is `.env.example`.

## The UI

- **`src/practices/PracticesPart.tsx`**: the Practices part of the card modal and page, after Targets. Each practice is a `DocLink` with its title and "N rules"; the part has none and failure states.
- **`src/markdown/`'s `cited` kind**: a raw slice equal to a served `written`, outside code, becomes a `DocLink` to its `name`. An escaped ID never links.
- **Documents' Discipline facet**, strings `documents.*.disciplines`.
- **First load** is 835 195 B of the 845 kB bar (`ui-languages.md` "Size").

## Known limits

- **Tight budgets.** Rules shrink to IDs under a tight budget, and a reader fetches them.
- **Links age.** A link is resolved when read: a rule deleted later reads as text on the next read.
- **Hubs and tasks.** A hub gives a `T-` task no practices.
- **The mark's walk.** A hub's MCP `get_card` walk grows with the corpus, about 1.3 s at 10k nodes (`spec-library.md` "Known limits"). Trusting the mark is a read-trust decision for the planned concurrent-trusted-reads ADR.
- **A doubled warning.** A card linking a task prints a walk warning twice on `--context`: once for card-tasks' read, once for the bundle's.
- **Cost** (DP16, release, warm, medians): at 10k nodes with 20 practices × 30 rules, `show --context` takes 31 ms, 2 ms more than a card that matches nothing, and a hub's `get_card` takes 76 ms of `run_ms`. At 1k nodes: 19 ms, 2 ms more, and 25 ms. The soak (DP17: 2×300 s, 10 logins): slopes 0.0 and 0.0 MiB/min (RSS 93.0→102.2 and 92.6→102.5 MiB); one walked line per `/mcp` `get_card` call in both runs (3 714 and 3 346 calls); the page route 0 walked of 372 and 335 reads (cap 2); client worst 1 912 and 2 245 ms, `run_ms` worst 430 and 410 ms; no hang.

## Tests

- CLI `practices_{base,bundle,card,check,flow,presets}.rs`, with the scene `practices_scene/`;
- MCP `practices.rs`, `mcp_size`;
- http `practices.rs`;
- eval `practices_scale.rs`, `practices_soak.rs`, `practices_common/`;
- UI `practices/PracticesPart.test.tsx`.
