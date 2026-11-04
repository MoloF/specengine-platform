---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, docs]
ref: RM-0055; Phase 7 step 13 (08 s2, row 13, its "ADRs" paragraph, the owner's decisions 3, 6 and 9); the orchestrator's Z1-Z7, A5 and the settlements S1-S12 below are working answers, each awaiting the owner's review; the draft ADR `zone-index` awaits the owner
---

# zone-index: canon under the index entry's `unlisted` globs takes no line, and canon retires by status (RM-0055)

## Why

Writers keep briefs as canon, one document per flow step (the owner's decision 9; a step is a document: `docs/canon/flow-links.md` "Items"). The `write-flow` skill writes them `class: canon`, with no tier and no scope. The brief lives in the game's repository (decision 6), so the developers' index is its index too. Two rules of today meet there:
- **Every walked document but a generated one takes one index line** (`docs/canon/spec-check-graph.md#index-shards`). A brief's line is 55-130 B, so the root's 10 240 B cap holds 75-175 briefs. ADR-0030's shards raise the ceiling to about 130 shards (each listed twice in the config, plus a pointer of about 70 B in the root), about 10 000 briefs, and that index is spent on routing.
- **Canon never reaches Tier 3** (`check::is_tier3`). A cut brief stays in search, tree, graph, flow, bundle candidates and W's pool. Every way out today breaks something: deleting it dangles its citers, an `exclude` glob contradicts the convention's s6 (front-matter, not location, excludes), and reclassifying it as an abandoned spec contradicts decision 9.

This is invisible today only because `spec init` writes no `[[generators]]` and the index rules are opt-in. A project that registers its index fails `spec check` with `budget` at about 100 briefs, and since ADR-0062 every brief's apply rewrites the index. This slice does two things. It gives the index entry `unlisted` globs: canon under them takes no line, and the root names each glob once. It also adds `status: retired`, which makes canon Tier 3. A brief corpus of any size then stays inside the convention: the check stays clean, the index's W term stays constant, and a dead brief leaves an agent's context while staying readable by ID.

The orchestrator decided the following. Each decision is binding here and awaits the owner:
- **Z1.** A zone is the index entry's own globs. They use the `exclude` and `claims` grammar, have no output file, and take no `[zones]` name. The key is named `unlisted` (S1).
- **Z2.** A glob takes canon only, and never Tier 0 or Tier 1 canon. Specs, decisions, class-less files and files whose front-matter failed keep their lines.
- **Z3.** Tier 3 canon under a glob takes no line, because the glob is applied before the archive.
- **Z4.** The root's pointer carries no count.
- **Z5.** The check judges only canon `retired`. Every other canon status stays unjudged.
- **Z6.** No new finding code: `CHECK_CODES` stays at 37.
- **Z7.** The amendments of ADR-0022 and ADR-0030 are a working answer under 08 s2's "ADRs" paragraph: the draft ADR `zone-index` ("Draft ADR"), accepted by the orchestrator at ship, with the owner's review open.
- **A5.** Canon with `tier: 0` or `tier: 1` cannot retire: `status-invalid`.
- **One slice**, with `retired` first in the work order. If iteration 1 fails, the slice splits into 13a `retired` (no ADR, since decision 9 covers it) and 13b `unlisted` (the ADR).

This spec settles the following, each a working answer:
- **S1. The key is `unlisted`**, on the `index = true` entry: a non-empty array of globs, each glob one zone. The name is not `zones`, because `[zones]` and the `zones` keys of `[gate]` and `[team]` already name the gate's controlled paths (`docs/canon/gate.md` "Config", `docs/canon/hub-team.md`). Owner's decision 3 (zone leads) stays theirs: a project may give both the same globs, and nothing ties them.
- **S2. Placement order:** unlisted, then archive, then the first matching claim, then the root.
- **S3. The root's section `## Unlisted`**, after the class sections and before `## Shards`: a fixed paragraph, then one line per glob ("Data").
- **S4. The `retired` line** is ADR-0028's Tier 3 line with the status printed whole: `- [<path>](<link>) retired`.
- **S5. `is_tier3` holds for canon `status: retired` whose `tier:` is neither 0 nor 1.** A tier 0 or tier 1 canon with `retired` stays live: it keeps its line, W counts it at its tier, and it errs `status-invalid`. Render, W (`working_set.rs` reads tier 0 and 1 before Tier 3) and the graph therefore agree.
- **S6. `INDEX_FORMAT` 18.** `files.tier3` is derived, and since 12e an unchanged stat key means a file is never re-read.
- **S7. Exact replacement texts** for the amendment message and `docs/canon/health.md:57` ("Data").
- **S8. A repeated glob is dropped**, keeping the first, so each glob has one pointer line.
- **S9. Canon:** a new Tier 2 page, `docs/canon/spec-check-index.md`, and the ADR's `canon:` is its `#unlisted-globs`. `spec-check-graph.md` has 2 B free: it keeps the `index-shards` anchor with a summary, and its **Shards** paragraph moves to the page ("Canon at ship").
- **S10. The committed fixture is `fixtures/zone-index/tour/`**, nested. The scans of every top-level `specengine.toml` (store-reads' SR-0 and its golden recorded at 12d's base, `edge_keys.rs`, the genre test's kinds) stay as they are.
- **S11. The synthetic corpus lives in `crates/specengine-core/tests/brief_corpus/mod.rs`**, std only. The CLI test takes it by `#[path]`, as CLI tests take the store's `parity_config`.
- **S12. No glob in this repository.** Its canon contract is closed without `status` (`specengine.toml` `[classes]`), so no canon here can retire either.

## Description and interactions

**The owner** adds `unlisted = ["docs/spec/**"]` to the index entry. `spec export index` drops the lines of the canon under that glob, and the root gains `## Unlisted`, which names the glob. Nothing else moves: `writes`, the shards, the caps, `spec sparse-set` and the apply outputs stay as they are. Removing the key gives today's index back, byte for byte.

**An agent** reads `CLAUDE.md`, then the root. The root lists what a task starts from, as before. For a brief, the agent reads the pointer and makes one `spec tree ROOT` or `spec search` call (MCP `get_tree`, `search`; each answer is capped at 40 000 characters, `docs/canon/spec-cli-read.md`). It then reads at most three documents. The root's protocol line stays literal.

**A writer retires a brief** by putting `status: retired` in its front-matter, through `propose_change` like any other edit. The brief then leaves search (except with `--archive`), tree, graph, impact, flow (counted in `left_out`), bundle candidates and ancestors (the chain goes on past it), W's pool, the graph warnings' sources and Health's unapplied amendments. `spec show ID` and `get_node` still print it, with ` | status retired` and ` | archived`. A card's target and `spec trace REF` still resolve. Its citers stay clean, because a Tier 3 document still defines its IDs. Outside a glob, its line moves to the archive shard. Inside one, it has no line.

**Interactions:**
- **ADR-0030:** shards are unchanged. A glob and a claim may overlap: the glob takes the canon and the claim takes the rest.
- **ADR-0028:** the line form is kept, and the decision is not amended: it fixes a Tier 3 line's form, while the one-line-per-document rule this slice amends is ADR-0030's.
- **ADR-0062:** an apply commits the outputs whose render changed, so creating or editing canon under a glob commits that file alone.
- **12e's stat key:** this is why the format is bumped (S6).
- **Not changed:** the `write-flow` skill (RM-0062 teaches it `retired`), importers (they map their own words through the census `value_map`), `worst_w`, the finding codes, every command's output and exit codes, this repository's `specengine.toml`, `docs/canon/architecture.md` and `docs/canon/documentation-system.md`.

## Data

### `specengine.toml`

```toml
[[generators]]
command  = "spec export index"
writes   = ["docs/index.md", "docs/index-archive.md"]
index    = true
unlisted = ["docs/spec/**"]   # optional, index entry only: canon under these globs, Tier 0 and 1 aside, takes no line
shards   = [{ path = "docs/index-archive.md", tier3 = true }]
```

Each error below has the form `specengine.toml:<line>: <message>`, at the line of the key or of the item. The run cannot check, and both `spec check` and `spec export index` exit 2:

| Case | Message |
|---|---|
| on an entry without `index = true` | `` `unlisted` is only for the entry with `index = true` `` |
| `unlisted = []` | `` `unlisted` is empty: give it a glob, or drop the key `` |
| a glob breaking the `exclude` grammar | `` `unlisted`: <the grammar's problem> `` |
| a backtick or a control character | `` `unlisted`: "<glob>" <why>: the index renders it into Markdown `` (Debug-quoted, as `claims`) |
| not an array of strings | the TOML reader's type error, as for `claims` |

The following are accepted: a glob matching nothing; a glob matching an output (outputs are never listed); a glob overlapping a claim or another glob. A repeated glob is dropped (S8). The table stays outside the index fingerprint, so editing it re-parses nothing.

### Front-matter

```yaml
---
id: NODE-lost
class: canon
owner: owner
reviewed: 2026-11-04
parent: NODE-root
status: retired
---
```

`retired` is a fixed English word, canon only. A closed canon contract without `status` reports `key-extra`, and the document is still read as Tier 3, just as a spec's `status:` is read whatever its contract says.

### The render

When `unlisted` is set, the root's class sections are followed by this section, with `## Shards` (if any) after it, its bytes unchanged:

```
## Unlisted

Canon under these globs, Tier 0 and 1 aside, has no line in this index: find it with one `spec tree` or `spec search` answer (MCP `get_tree`, `search`), then read at most three documents.

- `docs/spec/**`
```

The exact bytes are:
- `\n## Unlisted\n\n`;
- the paragraph, then `\n\n`;
- per glob, `` - `<glob>`\n ``, in config order, with repeats dropped.

No count and no document path is ever written. No shard holds the section. Without `unlisted`, it is not written. A retired canon outside every glob has this line, in the archive shard (or, with no archive shard, in its file's Archive section, as today):

```
- [docs/canon/old-route.md](canon/old-route.md) retired
```

### The check

| File | Finding |
|---|---|
| canon, `tier: 0` or `tier: 1`, `status: retired` | `status-invalid` at the `status:` line, subject `status`: `` status `retired` on tier <N> canon: only tier 2 or untiered canon retires `` |
| spec, `status: retired` | `status-invalid`, today's text: `` status `retired` is none of draft \| in-progress \| shipped \| abandoned `` |
| decision, `status: retired` | `status-invalid`, today's text: `` status `retired` is none of accepted \| rejected \| superseded-by <ID> `` |
| canon, any other status | nothing (Z5) |

### Core API

- `Generator.unlisted: Vec<String>`: the checked globs, in config order, repeats dropped; only on the `index = true` entry; empty when there is no key. `RawGenerator.unlisted: Option<Spanned<Vec<Spanned<String>>>>`, checked as `claims` are (`paths_toml::checked_path(_, false)`, `not_markdown_text`).
- `is_tier3(&Fields)` gains `Some(DocClass::Canon) => status == Some("retired") && !matches!(tier, Some(0 | 1))`. `is_tier3_file`, `is_live`, `standing_of` and `worst_w` follow it with no edit.
- `render_index_set`: placement follows S2, with "unlisted" meaning readable fields, class canon, tier neither 0 nor 1, and a path matching any glob. `line`: a Tier 3 line prints `status:` whole for every class.
- `engine.rs`, the canon arm: after `tier_rules`, the A5 rule.
- Literals: `retired` (one constant, shared by `is_tier3` and the rule), `unlisted`, the section title and its paragraph. No path and no glob.

### `INDEX_FORMAT` 18 (store `lib.rs`)

```rust
/// 18: `files.tier3` also true for a canon `status: retired` off tier 0 and
/// 1 (`docs/features/zone-index.md` "Data"); no column. An unchanged stat key
/// never re-reads a file, so only a new stamp derives it again.
pub const INDEX_FORMAT: u32 = 18;
```

`tests/format_history.txt` gains `18 <hash>`. The format dump also reads `fixtures/zone-index/tour`, the corpus that sets a canon's `tier3`.

### Replaced texts

- **CLI `amend.rs` `applied`:** `format!("{amendment} is a {} or a {}, applied when its document goes to Tier 3 by status", Spec, Decision)`. The refusal reads `` `<p>` is `class: canon`: an amendment is a spec or a decision, applied when its document goes to Tier 3 by status; nothing stored `` (no class: `` `<p>` declares no `class:`: ... ``). Behaviour is unchanged: a canon amendment stays refused.
- **`docs/canon/proposal-amendment.md:26`:** the same text.
- **`docs/canon/health.md:57`:** `- A canon declaring `amends` stays listed until edited or `status: retired`.`
- **`docs/canon/health.md:17`:** "Listed: ..., a canon." becomes "a canon not `retired`."

### The fixture `fixtures/zone-index/tour/`

The fixture is an invented museum audio tour, with no game word. Its config:
- `[paths] roots = ["GUIDE.md", "docs"]`, `tier0 = "GUIDE.md"`, `tier1_name = "README.md"`, `index = "docs/index.md"`;
- `[ids] TOUR = { kind = "tour-stop", shape = "name" }`, `TDR = { kind = "decision", width = 4 }`;
- the entry of "`specengine.toml`" above with `unlisted = ["docs/tour/**"]`, writing `docs/index.md` and `docs/index-archive.md`; both outputs are committed.

Canon contracts are the defaults (open: `owner`, `reviewed`).

| Path | Front-matter beyond the contract | Placed |
|---|---|---|
| `GUIDE.md` | canon, `tier: 0` | root, Canon |
| `docs/README.md` | canon, `tier: 1` | root, Canon |
| `docs/canon/route.md` | canon, `tier: 2`, `links: {uses: [TOUR-ANNEX]}`; its body cites `TOUR-WING`; a heading `## Order` | root, Canon |
| `docs/canon/old-route.md` | canon, `tier: 2`, `status: retired` | archive: the line above |
| `docs/tour/README.md` | canon, `tier: 1` | root, Canon |
| `docs/tour/hall.md` | `TOUR-HALL`, canon, the flow's container | none |
| `docs/tour/entry.md`, `gallery.md`, `exit.md` | `TOUR-ENTRY`, `TOUR-GALLERY` (follows ENTRY), `TOUR-EXIT` (follows GALLERY), `parent: TOUR-HALL` | none |
| `docs/tour/annex.md` | `TOUR-ANNEX`, `parent: TOUR-HALL`, follows GALLERY, `status: retired`; the only file holding `sculpture` | none |
| `docs/tour/wing.md` | `TOUR-WING`, `parent: TOUR-HALL`, `status: retired` | none |
| `docs/tour/room.md` | `TOUR-ROOM`, `parent: TOUR-WING`, live | none |
| `docs/tour/plan.md` | spec, `status: draft` | root, Specs |
| `docs/tour/TDR-0001.md` | decision, `accepted`, `canon: docs/canon/route.md#order` | root, Decisions |

`expected.json` lists the paths each output lists, in line order, then the documents with no line and the Tier 3 ones:

```json
{
  "outputs": {
    "docs/index.md": ["GUIDE.md", "docs/README.md", "docs/canon/route.md", "docs/tour/README.md",
                      "docs/tour/TDR-0001.md", "docs/tour/plan.md"],
    "docs/index-archive.md": ["docs/canon/old-route.md"]
  },
  "unlisted": ["docs/tour/annex.md", "docs/tour/entry.md", "docs/tour/exit.md", "docs/tour/gallery.md",
               "docs/tour/hall.md", "docs/tour/room.md", "docs/tour/wing.md"],
  "tier3": ["docs/canon/old-route.md", "docs/tour/annex.md", "docs/tour/wing.md"]
}
```

### The synthetic corpus `crates/specengine-core/tests/brief_corpus/mod.rs`

```rust
pub fn config(unlisted: bool) -> String;                                  // the corpus's specengine.toml
pub fn files(briefs: usize, seed: u64, broken: bool) -> Vec<(String, Vec<u8>)>;  // root-relative path, bytes; path order
```

- **Config:**
  - `[paths] roots = ["HANDBOOK.md", "docs"]`, `tier0 = "HANDBOOK.md"`, `tier1_name = "README.md"`, `index = "docs/index.md"`;
  - `[ids] BRF = { kind = "brief", shape = "name" }`, `DEC = { kind = "decision", width = 4 }`;
  - the entry writes `docs/index.md`, `docs/index-archive.md` and `docs/index-rest.md`, with `unlisted = ["docs/briefs/**"]` (left out when `unlisted` is false) and `shards = [{ path = "docs/index-archive.md", tier3 = true }, { path = "docs/index-rest.md", claims = ["docs/**"] }]`. The catch-all claim is what makes ZI-3's mutation visible.
- **Outside the glob:** `HANDBOOK.md` (tier 0), `docs/README.md` (tier 1), and `docs/canon/c1.md` to `c3.md` (tier 2, about 2 KB each, so they are the pool's three largest; `c1` has a heading `## Rules`).
- **Inside the glob:** `docs/briefs/README.md` (tier 1), `docs/briefs/DEC-0001.md` (accepted, `canon: docs/canon/c1.md#rules`), `docs/briefs/plan.md` (a draft spec), and `docs/briefs/broken.md` (unclosed front-matter, only when `broken`).
- **The briefs:**
  - Groups of 100: `docs/briefs/g<4 digits>.md` (`BRF-G0001`, canon, no tier).
  - In each group, its briefs: `docs/briefs/g0001/b<5 digits>.md` (`BRF-G0001-B00001`), with `parent:` the group and `follows` the previous brief; a group's first brief follows the previous group's last.
  - Each brief has a body of 80-400 B of seeded words.
  - Exactly `briefs / 5` of them are `status: retired`, chosen by the seed.
- **Determinism:** two xorshift64\* streams seeded through splitmix64, as eval's `scale/generate.rs`. No clock and no hash-map order: one seed and one size give one tree.

## Rules and edge cases

- **R1.** WHEN the index entry has `unlisted` and a walked document has readable front-matter, `class: canon`, a `tier:` that is neither 0 nor 1, and a path matching any glob, the render SHALL give it no line in any output, Tier 3 or not.
- **R2.** Every other document SHALL be placed as today: Tier 3 to the archive shard if one exists, else to the first shard with a matching claim, else to the root.
- **R3.** WHEN `unlisted` is set, the root SHALL hold `## Unlisted` as "Data" gives it. It SHALL hold no count and no document path.
- **R4.** WHEN no `unlisted` key exists, every output SHALL be byte for byte what it is today.
- **R5.** WHEN a file's front-matter failed, the file SHALL keep its line under `No class`: it is not canon. A `[paths] tier0` file with no `tier:` under a glob has no line and errs `tier-invalid`, as today.
- **R6.** A canon document with `status: retired` and a `tier:` that is neither 0 nor 1 SHALL be Tier 3. Its line, outside every glob, SHALL be `- [<path>](<link>) retired`.
- **R7.** The check SHALL report the "Data" findings and no others. Statuses are never hashed (`docs/canon/node-revision.md` "The recipe", **Never counted**), so retiring a document SHALL raise no revision, mark no card and move no lock entry.
- **R8.** A retired document SHALL still define its IDs: its citers get no `ref-dangling` and no new warning, and `ref-superseded` stays decisions-only. Its own front-matter references SHALL still be checked. Its inline mentions are no longer warned, since it is no live source (`docs/canon/spec-check-graph.md` "Graph warnings").
- **R9.** Through `is_tier3_file`, a retired document SHALL leave search (except with `--archive`), tree, graph, impact, flow, bundle candidates and ancestors, W's pool and Health's amendments. It SHALL stay readable by `show`, `get_node`, `--archive`, a card's target and `spec trace`.
- **R10.** The genre rule (ADR-0008, `docs/canon/architecture.md#universal`): core SHALL hold no path or glob for a zone, and no default.
- **R11.** Nothing is blocked by an unlisted or a retired document (ADR-0012). Files stay the truth (ADR-0001, ADR-0003).
- **R12.** Placement is a pure function of the entry and the walk, blind to input order. Pointer order is config order.

**Known limits** (these go to the canon page):
- A `**` glob leaves only Tier 0 and 1 canon, specs and decisions listed; the pointer is always printed.
- An agent with neither a shell nor MCP cannot reach canon under a glob.
- A retired container hides its live subtree from the default `spec tree` (`left_out` counts it once); search and `--archive` reach the subtree.
- One tool answer may cost up to 40 000 characters, and `worst_w` does not count it. A flat zone truncates `spec tree` without a ROOT.
- An older binary refuses `unlisted` (unknown key: cannot check), so a team upgrades together. Format 18 recreates each index on its first write, a hub's included.
- A canon document already `status: retired` before the upgrade leaves retrieval on the upgrade.

## Acceptance criteria

Tests run on scratch copies under `std::env::temp_dir()`, with non-ASCII written as escapes. M names the mutations that must turn each criterion red: 25 in all. Commands take the form `cargo nextest run -p <crate> --test <file>`.

- [ ] **ZI-1 [default] No glob, no change.**
  - (a) Here, with the slice's code, `spec export index` leaves `git status --porcelain` empty, and `spec check` gives 0 errors and 0 warnings.
  - (b) The tests over the four fixtures whose config has an `index = true` entry, and every index render test (core `check_shards.rs`, `check_index.rs`; CLI `export.rs`, `export_shards.rs`, `shards_repo.rs`, `apply_outputs.rs`), pass unedited.
  - M: an empty `## Unlisted` written with no glob.
- [ ] **ZI-2 [default, core `check_unlisted.rs`] The cap holds.**
  - (a) Over `files(1_000, 1, true)` and `files(10_000, 1, true)`, every output is at most 10 240 B, and each output is byte-equal at both sizes.
  - (b) No output line links a document under `docs/briefs/`, except that directory's `README.md`, `plan.md`, `DEC-0001.md` and `broken.md` (the root's glob line is no link).
  - (c) The root holds `## Unlisted` once, with one line for the glob. With the glob written twice, it still holds one line.
  - M: the glob ignored; the section dropped; the repeat kept.
- [ ] **ZI-3 [default and ignored, CLI `zone_index.rs`] On disk.**
  - (a) `files(1_000, 1, false)` is written to a scratch directory. `spec export index` then `spec check` exit 0, with 0 errors and 0 warnings.
  - (b) With `config(false)` and `export index` run again, `check` exits 1 with `budget` on `docs/index-rest.md`.
  - (c) At 10 000, the same, under `#[ignore = "cargo nextest run --release -p specengine-cli --test zone_index -- --ignored"]`.
  - M: claims applied before the glob (the catch-all takes the briefs).
- [ ] **ZI-4 [default, core] Placement.**
  - (a) Over the corpus and over the fixture, every walked document is on exactly one line across the outputs. The exceptions are generated documents, the outputs, and R1's canon (`check_shards.rs`'s `listed_once` widened by R1 alone).
  - (b) Under the glob, the draft spec, the accepted decision, the failed front-matter file and the Tier 1 README keep their lines.
  - (c) The fixture's render equals `expected.json`.
  - M: the glob matched on the path alone; Tier 0 and 1 not excepted.
- [ ] **ZI-5 [default, core] Determinism and precedence.**
  - (a) Reversed and seed-shuffled input gives equal bytes for every output.
  - (b) A canon matched by both a claim and a glob has no line, and a spec matched by both lands in the claim's shard.
  - (c) Of two matching claims, the first wins.
  - M: claims applied before the glob; the last claim winning.
- [ ] **ZI-6 [default, core `check_config.rs`; CLI `zone_index.rs`] Config errors.**
  - (a) Each of the five cases in "Data" gives its message at the line of the key or item, and `spec check` and `spec export index` exit 2 on it.
  - (b) The three accepted cases load.
  - M: `unlisted` accepted off the index entry; an empty list accepted; a backtick accepted.
- [ ] **ZI-7 [default, CLI `zone_index.rs`] Apply.** On a committed git copy of the fixture:
  - (a) A confirmed `spec propose create docs/tour/cloak.md` (canon `TOUR-CLOAK`, `parent: TOUR-HALL`) commits exactly `A docs/tour/cloak.md`.
  - (b) A confirmed create of `docs/canon/map.md` commits `A docs/canon/map.md` and `M docs/index.md`.
  - M: a count in the pointer.
- [ ] **ZI-8 [default, core `check_unlisted.rs`] `retired`.**
  - (a) `is_tier3_file` is true on canon `retired` with no tier and with `tier: 2`. It is false with `tier: 0` and `tier: 1` (A5), and on canon `draft`.
  - (b) The check gives exactly the "Data" findings: one `status-invalid` with the tier 1 text; today's two texts on a spec and a decision; nothing on canon `draft`.
  - (c) `CHECK_CODES.len()` is 37.
  - M: canon never Tier 3; a spec accepting `retired`; tier 1 canon becoming Tier 3.
- [ ] **ZI-9 [default, CLI `zone_index.rs`, mcp `mcp_read.rs`] Out of retrieval, readable by ID.** On the fixture:
  - (a) `spec search sculpture` gives 0 hits and `tier3_left_out` 1; `--archive` gives `TOUR-ANNEX` ` | archived`.
  - (b) `spec show TOUR-ANNEX` exits 0 with ` | status retired` and ` | archived`.
  - (c) `spec tree TOUR-HALL` lists ENTRY, GALLERY and EXIT; it does not list ANNEX, WING or ROOM; its `left_out.tier3` is 2.
  - (d) `spec flow TOUR-HALL` has no node of ANNEX, WING or ROOM, and its `left_out.tier3` is 2 (a left-out child's subtree is not walked).
  - (e) `spec bundle TOUR-GALLERY` does not admit ANNEX. `spec bundle TOUR-ROOM`'s Ancestors hold TOUR-HALL and not TOUR-WING.
  - (f) MCP `search`, `get_node`, `get_tree` and `get_flow` return structured content equal to the CLI's `--json`.
  - M: the store's `tier3` computed without the canon arm; the flow's scope ignoring standing.
- [ ] **ZI-10 [default, core and CLI] Its line.**
  - (a) The fixture's archive shard holds `- [docs/canon/old-route.md](canon/old-route.md) retired`, and no line of the retired documents under the glob.
  - (b) In core, a retired canon with no tier, outside every glob, prints `retired` too.
  - M: the canon arm kept for Tier 3 lines (`tier 2`, `tier ?`).
- [ ] **ZI-11 [default, CLI] Links.**
  - (a) `spec check` on the fixture gives no `ref-dangling` for `route.md`'s `uses: [TOUR-ANNEX]`, no `mention-dangling` for its `TOUR-WING`, and 0 warnings.
  - (b) Adding `links: {uses: [TOUR-GONE]}` to `docs/tour/annex.md` gives one `ref-dangling` error there. A body mention of `TOUR-GONE` in that file gives none.
  - M: a Tier 3 canon's IDs left undefined.
- [ ] **ZI-12 [default, CLI] Status is not meaning.** On a scratch copy, setting `status: retired` on `TOUR-GALLERY` leaves unchanged its `norm_hash` and `rev` (`spec show --json`) and the mark of a card targeting it (`spec card show`). M: `status` counted in the norm.
- [ ] **ZI-13 [default, store `format.rs`] The stamp.**
  - (a) `INDEX_FORMAT` is 18, and `format_history.txt` ends with `18 <hash>`.
  - (b) Over the fixture, an index is built, then through the raw connection its `files.tier3` is set to 0 for `docs/tour/annex.md` and its stamp to 17. The next `update` and `search` leave `TOUR-ANNEX` out (`tier3_left_out` 1).
  - (c) `bundle_config.rs`' pins and `read_trust_pins_format_17_...` move mechanically to 18.
  - M: no bump (the stale row is trusted and found).
- [ ] **ZI-14 [default, CLI `kind_amendment.rs`] Texts.**
  - (a) The two pins at `:486` and `:495` read the new message.
  - (b) After the ship's canon, `grep -c 'never reaches Tier 3' docs/canon/*.md` totals 0, and `docs/canon/proposal-amendment.md` no longer holds "which only a spec or a decision can".
  - M: the old message kept.
- [ ] **ZI-15 [default, core `check_genre.rs`] Genre.** `PROJECT_NAMES` gains `briefs/` and `tour/`. The check sources name no path or glob, and their new literals are those listed in "Core API". M: a default `docs/spec/**` in core.
- [ ] **ZI-16 [docs, core] Dogfooding.**
  - (a) Here, `check` is clean; the ADR is at most 1 536 B; `crates/specengine-eval/tests/doc_pointers.rs` passes; `git diff` leaves `documentation-system.md` and `architecture.md` untouched.
  - (b) The corpus's `worst_w`, outputs included, is at 10 000 within 10 % of its value at 1 000.
  - (c) Consumer projects' read-only checks stay equal (the orchestrator's, outside the repository).
  - M: the glob ignored (W grows with N).
- [ ] **F1 [form]** clippy and fmt are clean; no new dependency; `check` gives 0 errors and 0 warnings.

## Out of scope

- A `ref-retired` finding, or a warning on a live child of a retired parent.
- Tree and flow rule changes.
- `superseded-by` for canon; retiring a single section.
- `specengine-eval scale` and `w`; `worst_w` counting tool answers; timing `spec tree` at scale.
- The UI.
- Teaching `write-flow` about `retired` (RM-0062).
- Importers mapping their own words to `retired`.
- Card marks on retired targets.
- A glob in this repository.
- `[zones]`-based unlisting; a default glob.

## Implementation

Deferred on 2026-11-04 to the next release, `v4`, with RM-0055 (08 s2 Phase 7, "Release scope"): this draft stays as committed, and a partial implementation exists outside the tree. To be filled in after implementation. The plan and the ship's canon follow.

### Files in scope

| Crate | Files | What changes |
|---|---|---|
| core | `src/check/config.rs`, `render.rs`, `engine.rs` | `Generator.unlisted` and its errors; R1-R4 and the section; the Tier 3 line; `is_tier3`; A5 |
| store | `src/lib.rs` | `INDEX_FORMAT` 18 |
| CLI | `src/amend.rs` | the message |
| tests (test-engineer) | core `tests/check_unlisted.rs`*, `tests/brief_corpus/mod.rs`*, `check_config.rs` (two `Generator` literals, ZI-6), `check_shards.rs`, `check_genre.rs`; store `tests/format.rs`, `format_history.txt`; CLI `tests/zone_index.rs`*, `kind_amendment.rs`, `bundle_config.rs`; mcp `tests/mcp_read.rs`; `fixtures/zone-index/tour/`* | ZI-1 to ZI-16 |

\* New. Test comments cite this spec, not the canon page, which exists only from ship on (`doc_pointers.rs`).

### Work order

1. `retired`: `is_tier3`, the Tier 3 line, A5, format 18, the CLI text; ZI-8, ZI-13, ZI-14. A split 13a runs ZI-9 to ZI-12 on a copy of the fixture without `unlisted`.
2. `unlisted`: the key, its errors, placement and the section; ZI-1, ZI-2, ZI-4, ZI-5, ZI-6, ZI-15.
3. The fixture, the CLI and the MCP tests: ZI-3, ZI-7, ZI-9 to ZI-12, ZI-16.

### Before

At `dfafc1e`:
- `INDEX_FORMAT` is 17 and `CHECK_CODES` 37.
- Here, `check` reads 425 documents, 0 errors, 0 warnings, worst W 134 099 B.
- The root `docs/index.md` is 2 661 B.

### Canon at ship

Free bytes are as of this spec.
- **New `docs/canon/spec-check-index.md`** (Tier 2, scope `[crates/specengine-core]`, claimed by `docs/index-canon-spec.md`, which has 4 426 B free), about 6 KB. Its sections:
  - an intro: ADR-0030, the new ADR, built by `index-shards` and `zone-index`;
  - `## Shards`: `spec-check-graph.md`'s **Shards** paragraph, moved whole and edited for R1;
  - `## Unlisted globs`, the ADR's anchor: S1-S3, Z2-Z4, R1-R5, the read, W, apply, `[zones]` unrelated;
  - `## Retired canon`: S4, S5, R6-R9, format 18;
  - `## Known limits`; `## Tests`.
- **`docs/canon/spec-check-graph.md`** (2 B free):
  - the API line gains `unlisted: [glob]`;
  - the line rule gains "(a canon's `retired`)";
  - Tier 3 gains ", a canon `retired` off tier 0 and 1";
  - the s11.6 error list gains "`unlisted` off the index entry, empty, or holding a glob a claim could not";
  - Genre gains `retired`, `Unlisted` and its paragraph;
  - the `index-shards` anchor stays (ADR-0030's `canon:`, `docs/README.md` and `spec-check.md` cite it), and so does the heading of `#index-render` (store `tests/parity_config/mod.rs:168`).

  The s11.6 TOML block stays unchanged, because `check_config.rs` reads it verbatim. The **Shards** paragraph (about 1 240 B) becomes, about 590 B:
  > **Shards** (ADR-0030) and **unlisted globs** (the new ADR), in full: `docs/canon/spec-check-index.md`. The index is the root at `[paths] index`, the one entry point, plus the index entry's `shards`. Each walked document but a generated one, an output, or a canon under an `unlisted` glob off tier 0 and 1 has one line across the set, placed: unlisted (no line), Tier 3 to the archive shard, the first matching claim, the root. The root and each live shard are capped at `index_bytes`, the archive none; W's index term is the root + the largest live shard. No shard and no glob: the single file above, byte for byte.
- **`docs/canon/spec-check.md`** (11 B free): `status-invalid`'s list gains "; tier 0-1 canon `retired`" (26 B). Room, about 33 B, comes from shortening "(<= 4294967295, `u32::MAX` as" to "(<= `u32::MAX`, as" and "This repository, root config, no baseline: ... 0 debt" to "This repository, no baseline: ...".
- **`docs/README.md`** (Tier 1, 10 B free):
  - Tiers, the Tier 3 bullet, gains ", canon `retired` (none here: canon takes no `status:`)" (52 B);
  - Generated, "one line per document across them", gains ", canon under an `unlisted` glob excepted (none here; the new ADR)" (about 65 B);
  - room: drop the sentence after the classes table ("Misclassification is the main source of rot...", 143 B with its blank line), which the norm's s2 states verbatim.
- **`docs/canon/health.md`** (6 B free): the `:17` and `:57` texts of "Data" (net about -19 B).
- **`docs/canon/proposal-amendment.md`** (2 545 B free): `:26`'s message.
- **Store README** (9 B free), **`symbol-index.md`** (2 B free, `:23`) and **`layer-c.md`** (5 B free, `:78`): format 17 becomes 18, at the same length.
- **`CLAUDE.md`** (7 465 B free): the state line names `zone-index` and step 13 shipped, the ADR and the page; next is step 14 `change-record`.
- **Plan:** RM-0055 shipped and compressed; 08's Phase 7 heading, row 13 and the "ADRs" paragraph.
- **Generated:** `docs/index-decisions.md` (1 829 B free) gains the ADR, about 140 B.
- **Unchanged:**
  - `docs/canon/architecture.md` (4 B free): `#documentation-convention` says the convention applies, not "in full", and `#universal` already lists config as the project's;
  - `docs/canon/documentation-system.md`;
  - ADR-0022, ADR-0028, ADR-0030;
  - the core README (22 B free; `-graph.md` holds the API), the CLI README (15 B free);
  - `spec-cli-graph.md`, `spec-flow.md`, `spec-cli-bundle.md`, `node-revision.md`.

### Draft ADR

This is a working answer, awaiting the owner. ADR\-0070 went to `spec-ids` (RM-0073, `v3`). This ADR claims no number until this slice resumes in `v4`: then it takes the next free number in place of `NNNN`, and its date, at ship. With four digits there, it is 1 464 B with its front-matter:

```markdown
---
class: decision
id: ADR-NNNN
title: Canon under the index entry's unlisted globs takes no index line, and canon retires by status
status: accepted
date: 2026-11-DD
scope: [docs]
canon: docs/canon/spec-check-index.md#unlisted-globs
ref: zone-index (RM-0055, step 13); the orchestrator's Z1-Z7, the owner's review open
---

# ADR-NNNN

**Decision.** Accepted by the orchestrator in the owner's absence, awaiting the owner's review. Amends ADR-0022 and ADR-0030. A canon document matching a glob of the index entry's `unlisted` (the `exclude` grammar), Tier 0 and 1 aside, has no index line, Tier 3 too; the root names each glob once, no count, and a read there is the root, one capped `spec tree` or `spec search` answer, then at most three documents. Placement: unlisted, archive, first claim, root; no glob: byte for byte. `status: retired` makes canon off tier 0 and 1 Tier 3. ADR-0022's "in full" excepts s9's line per document under such a glob; caps are never raised.

**Why.** Briefs are canon, one per flow step (owner's decision 9): their lines fill the root at about 100 and the shards' pointers at about 130 shards, and no status took a cut brief out of retrieval.

**Cost.** Such canon is found only through a tool answer of up to 40 000 characters, which W does not count; an agent with neither shell nor MCP cannot reach it. A retired container hides its live children from `spec tree`. Older binaries refuse the key; `INDEX_FORMAT` 18 re-indexes.
```
