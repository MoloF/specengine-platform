---
class: canon
tier: 2
scope: [crates/specengine-eval]
owner: owner
reviewed: 2026-10-05
---

# W measurement: every task, before and after

`specengine-eval w` (eval `src/w.rs`) measures task W (`docs/canon/documentation-system.md` §1, §3) by replaying **every task document** twice on the same targets: the pilot's reading protocol on the untouched corpus (**W_before**) and the shipped `spec bundle` on the after-tree (**W_after**). The bundle caps itself and follows `mentions` only into layers 2 and 4 (`docs/canon/spec-cli-bundle.md`), so the reads an agent still has to make count too (**W_after_followups**). 40 KB is recorded, never enforced (ADR-0012). Pilot figures: `docs/features/pilot-w.md` AC-10, 08 §3 AC-1.

Read-only on the corpus: refusals; `layout::prepare`; `layout::write_tree` (the pre-verifier step of `layout`) writes the after-tree to `<out>/w/<label>/tree/` (files, emitted `specengine.toml`, index outputs; no baseline); core reads it (`check_input`, `SpecGraph::new`); `Layout.emission` (in memory) maps after paths to sources; per task the library's `specengine_cli::bundle` and `show`, as MCP calls them.

## Tasks config

Read only by `w`: `--tasks`, else `SPECENGINE_TASKS_A` / `_B` under `--label pilot-a` / `pilot-b`, else `tasks.toml` at the corpus root; `deny_unknown_fields`.

```toml
[tasks]
include = ["docs/features/**/*.md"] # required, >= 1; globs over source paths
exclude = ["docs/features/drafts/**"] # default []
[tier1] # optional: no Tier 1 slot without it
key = "scope" # optional; front-matter key, read in the after-tree
default = "docs/README.md" # optional; corpus file
[tier1.paths] # needs `key`; value -> corpus file
"area-a" = "crates/a/README.md"
```

Globs: core's matcher (`WalkScope::is_excluded`). Exit 2 `<tasks>:<line>: message` (line 1 without a span), nothing written: unknown key, wrong type, empty `include`, an `include` or `exclude` glob matching no walked source document, `[tier1.paths]` without `key`, a path not clean-relative or not a corpus file.

**Recipe** (no pilot named in the repository): `include` = the documents the pilot hands agents as tasks; `exclude` = its draft or archive directories; an area key after the key map → `key`, per value the Tier 1 README its Tier 0 routes to; `default` = the Tier 1 README over the tasks. Re-run until no refusal; a zero `slots.tier0` or `.index` is a finding.

## CLI

`specengine-eval w … [--tasks <toml>] [--budget <n>]`, other flags as `layout` (`--timeout` counts after `prepare`), fixture `pilot-w/one`. `--budget` 1..=4294967295 (negative: clap's usage error), default 10 000 (`bundle_task`, 07 §5), passed as `BundleRequest.budget` (the emitted `bundle_node` unread).

Exit 0 measured (`"timeout"` on overrun); 2 refused, nothing written: the tasks config, `--budget`, `layout`'s refusals, `--out` and the corpus nested either way, a symlinked `<out>/w[/<label>]`, a before scheme without `[project] slug`; 1: a CLI error other than a frame refusal, I/O.

stdout: `layout`'s envelope, keys sorted; `result` exactly (a W statistic: bytes, or `"refused"` / `"failed"`):

```json
{"tasks":4,"budget":{"tokens":10000,"source":"default"},
"w_before":{"median":0,"p90":0,"max":0},"w_after":{…},"w_after_followups":{…},"docs_needed":{…},
"third_step":0,"worst":{"w_before":"task-3",…},
"slots":{"tier0":0,"index":0,"shard":0,"tier1":{"key":0,"default":0,"none":0},"unmapped":0},
"targets":{"refs":0,"text":0,"outline":0,"header":0,"not_included":0},
"citations":{"unresolved":0,"unchecked":0,"wiki_links":0},
"followups":{"refs":0,"bytes":0,"truncated":0,"incomplete":0},"refused":0,"failed":0,
"nondeterministic":0,"fill_percent":{"median":0.0,"max":0.0},"bytes_per_token":0.0,"bundle_ms":{"median":0,"p90":0}}
```

`source` `flag` | `default`; `slots.tier0`, `.index` corpus bytes (0: unset, missing); `.shard`, `.tier1.*` task counts. No path, ID, glob or key on stdout.

Detail in `<out>/w/<label>/` (emptied first): `tree/`, `home-1|2/`, `bundles/task-N.txt` (pass 1 body per answered task), `tasks.json`, per task: `task`, `source`, `after`, `refs`, `targets` (with `sources`), `citations` (`written`, `line`, `path` when in a record file), `wiki_links` (`line`, or `{path, line}`), `slots` (`slot`, `side`, `path`, `bytes`, `read`: corpus | tree), the figures, `bundle` (`Bundle`'s counts, `bundle_hash`, `forms`) or `message`, `followups` (per REF, `not_answered`), `bundle_hash_2`, `{bundle,bundle_2,show}_ms`.

## Tasks

`Layout.emission.documents` entries whose source matches `include` and no `exclude`, their after file not `Standing::Generated` (no other filter): Tier 3 (shipped, abandoned) and a failed or absent front-matter are tasks, a Tier 3 D bundled ` | archived` (`docs/canon/spec-cli-bundle.md` "Command"); `task-1…n` in source-path byte order. An after file unwalked or without a document node → `failed`, with a message.

## Targets N

For task D, every target maps to its REF once, in (path, ord) order: `<slug>/ID` for a feature-scoped ID in a feature document (bare, it names every feature's holder: `docs/canon/spec-check-links.md` "Resolution"), else `SpecGraph::name` (ID, else path); never the written form.

- **D** and **each record layout moved out of D**: an `emission.definitions` entry with D's source and another after file → the first node with its ID in that file, else that file's document.
- **Links**: the written end (`Edge::written_end`) of each resolved link written (`Edge::file`) in D's after file or those record files, of every kind (ID, alias, `<slug>/ID`, file link, `canon:`).
- **Tree-dangling or tree-unchecked file links** (inline Markdown) are read on the corpus from D's source as core would (`/` from the root, relative, `link_base`, percent-decoded, above the root dropped): a walked source document → its `emission.documents` after document, a target; no candidate resolves but one lies in the before walk scope → `unresolved`; none in scope, not `.md`, or resolving with no tree document → `unchecked`. Any other dangling reference → `unresolved`.
- **Anchors to moved records**: an inline file link or path-form `canon:` landing on one document node whose decoded anchor names nothing there but equals the ID or an import alias of a definition layout moved out of a source of that document → that record's node. A heading-slug anchor stays on the document.
- **Wiki links**: `[[…]]` pairs on one line of D's or its records' after files, fences included → `wiki_links`, one per line; none followed.

**Sources** of a node: the `emission.definitions` entries with its after path and ID (every holder of a duplicate), else the `emission.documents` entry with that after path, else none (`unmapped`).

## W_before

Corpus bytes of the distinct paths: Tier 0 (`[paths] tier0`, before scheme), Tier 1, index root (`[paths] index`), the first shard (path order) whose core parse links D's source, the sources of D and N; unset or missing: 0. Shard links: inline, resolved from the shard's directory (`/` from the root), then `link_base`, percent-decoded (explicit hex). Shards: the `index = true` generator's `shards`; else the generator whose `writes` holds `[paths] index` supplies its other `writes` that are `.md` and exist in the corpus. `docs_needed` = those sources outside the four slots; `third_step` = tasks with > 3.

**Tier 1**: `key`'s value in D's after-tree front-matter (a string, or each string of a list; `Node.fields` / `.extra`) in `[tier1.paths]`; several → the largest in corpus bytes (path order on a tie); none → `default`, else 0. One path, both sides.

## W_after

Tier 0 + Tier 1 + `Bundle.bytes` (`Globals{root: tree}`, the REFs, the budget), not post-processed; Tier 0 / 1 from the tree when it holds the path, else the corpus. No index, no de-duplication.

## Follow-ups and outcomes

- **Outcomes**: `reason` (exit 1) → `failed`; exit 2 naming the minimum or the ceiling (`below this bundle's minimum`, `-character ceiling`) → `refused`; any other CLI error ends the run (exit 1).
- **Forms**: `text` / `outline` / `header` = the targets layer by `form` (merged targets inside) of every answered bundle; `not_included` its tail.
- **Follow-ups** (all in `refs`): the outline and header items (the canonical REF at the item's (path, line, name), else its name); a refused task's every REF. `bytes` = Σ `render_text(show(REF))` bytes, per task each distinct REF once (answers cached per REF; `show`'s 40 000-character cap applied); `truncated` = those answered `[truncated: …]`; `incomplete` = tasks with > 3 (05 §6). A `show` not answering → the task `failed` (its message names the REF), the REF in `refs` (`not_answered`, 0 B), forms still counted.
- **W_after_followups** = W_after + the follow-up bytes (refused: Tier 0 + Tier 1 + them).

## Aggregates

Nearest rank over all tasks: ascending, p = v[⌈p·n/100⌉], median = p50, max = v[n]; refused and failed rank above every number (task order), a statistic landing there prints the word; `worst` = each W statistic's max task. `fill_percent` (tokens × 100 / budget) and `bytes_per_token` (Σ bytes / Σ tokens): one decimal, over tasks whose bundle answered; `bundle_ms` from pass 1.

## Determinism and isolation

- Pass 2 recomputes every bundle in reverse task order on a fresh `home-2/`; `nondeterministic` = tasks whose hash or outcome differs (08 AC-7). Sorted orders only: two runs give identical detail but `*_ms`.
- `Env {cwd: tree, home: <out>/w/<label>/home-1|2, xdg_data_home: None}`, never `Env::from_process`: nothing under the corpus, `HOME` or the repository.
- Pilot runs (`w_cli.rs` `mod pilots`, `#[ignore]`) as the crate README's, plus `SPECENGINE_TASKS_*` for the child.

## Known limits

- Tree resolution comes first: a relative link inside a moved record can name an existing tree file by chance, a target on both sides though the corpus reader could not follow it.
- A relocated link loses its anchor: the whole document is bundled (overstates W_after); a heading-slug anchor to a moved section lands on the residue (may understate it).
- Wiki links and dangling references are counted, not followed: N is understated on both sides. Truncated follow-ups understate W_after_followups.
- `refused` is recognised by the CLI's wording; a reworded message fails AC-05.
- Each call re-parses the tree: ~2 s a bundle in debug at 1 600 files.
- Not exercised by a fixture: a relocated link through `link_base` or leaving the root, an alias inside `canon:`, a `show` not answering.
