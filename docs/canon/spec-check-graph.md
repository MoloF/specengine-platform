---
class: canon
tier: 2
scope: [crates/specengine-core]
owner: owner
reviewed: 2026-10-06
---

# spec check: index render, generators, graph warnings

§11.5–6 of `docs/canon/documentation-system.md` over a generator registry, and three warnings over the parse: seven of the 35 `CHECK_CODES` (engine, config, verdict, output: `docs/canon/spec-check.md`). Pure and read-only: the render stays in memory, no generator is run (ADR-0013). Errors where the convention says "fail" (§11.5–6), warnings where recognition is heuristic or the finding is a content discrepancy (owner, Q-A; `#control`).

## API (`specengine_core::check`)

- `render_index_set(&CheckInput, index_path: &str, &Generator) -> Vec<IndexOutput {path, bytes: String}>`: the root, then each shard in config order; pure, blind to input order. `render_index` (same arguments) → the root's bytes. `walk_gap(&CheckInput, &Paths) -> Option<WalkGap {Unreadable, MissingRoot, UnlistedDir}>`: the first stop condition of §11.5 by path, shared with `spec export index`.
- `CheckConfig.generators: Option<Vec<Generator {command, writes, index, gate?, shards: [Shard {path, kind: ShardKind {Tier3, Claims(globs)}, line}], line}>>` (`None`: no table, the rules are off); `CheckConfig::index_generator()` → the `index = true` entry; `Generator::gate()` → `gate`, else `DEFAULT_GATE` = `spec check`; `Shard::is_archive()`.
- Module `check::resolve`: `Resolver`, `Resolution`, scope-aware, taking the citing file (`docs/canon/spec-check-links.md` "API"); the store keeps `dst` as written, so `spec refs`, `get_impact` reuse it.
- Normal dependency: `petgraph =0.8.3`, default features off (`Graph`, `tarjan_scc`; owner, Q-C).

<a id="index-render"></a>
## Index render (§9)

```
---
class: generated
generator: <command>
source: front-matter of the repository's documents
---

# Documentation index

<!-- Built by `<command>`. Manual edits are overwritten on rebuild, and `<gate>` rejects them. -->

Reading protocol (§9): this index, then at most three documents. Needing a third step means the index is wrong: fix it rather than reading further.
```

Then each non-empty section as `\n## <name>\n\n` plus one `\n`-ended line per document, paths in byte order: `Canon`, `Decisions`, `Specs` (live documents of the class), `No class — fix` (live, no readable class or one outside the four, failed front-matter included), `Archive — Tier 3, by id only`. The index and every other generated document are not listed.

Line `- [<label>](<link>) <title> · <scope> · <status>`; a Tier 3 line is only `- [<label>](<link>) <status>`, title and scope unread, the status whole (`superseded-by ADR-0026`; ADR-0028). Label = a decision's ID, else the path; link relative to the output's directory (`a/b/index.md` → `../../CLAUDE.md`); title = `title:`, else the first H1's inline text, else `?`; scope items joined by `, `; status = canon `tier N` (`tier ?`), else `status:`, else `?`.

**Tier 3** is decided once, by status, never by folder: a spec `shipped` or `abandoned`, a decision with a `status:` other than `accepted` (no `status:`: live). One public predicate, `check::is_tier3_file(&ParsedFile)` (over `is_tier3(&Fields)`; false when the front-matter failed or the file is not UTF-8), serves this render, the store's `files.tier3` and `spec search`'s archive filter; `is_live` (neither `class: generated` nor Tier 3) serves the graph rules. Front-matter failing strict YAML lists under `No class — fix`; a key of the wrong type renders absent.

<a id="index-shards"></a>
**Shards** (ADR-0030). The index is the root at `[paths] index` plus the `shards` of the `index = true` entry (config order, each in its `writes`); the root is the one entry point. Each walked document but a generated one has one line across the set. Placement: Tier 3 → the archive shard (`tier3 = true`) if any; else the first shard with a `claims` glob (the `exclude` grammar) matching the path; else the root. So a claimed failed front-matter lists under that shard's `No class — fix`; with no archive shard, Tier 3 follows the claims into that file's Archive section. Sections and lines as above. The root ends with `## Shards`: per shard, empty or not, `- [<path>](<link>) <label>`, no count; label `Archive — Tier 3, by id only`, else the claims in backticks joined by `, `. A shard: the header above, H1 `# Documentation index: <label>`, and `A shard of [<root path>](<link>), the index's one entry point.` for the protocol line. Caps: the root and each live shard `index_bytes`, the archive none (read by id, §3). W's index term: the root + the largest live shard. The cap, not §9's "~500 entries", decides when to shard; `scope:` never does. No shard (`shards = []` too): the single file above, byte for byte.

## §11.5: index drift

WHEN the registry has an `index = true` entry, each output's render SHALL equal its walked bytes, byte for byte, each output judged on its own. Any difference (a trailing space, CRLF, a BOM, a stale line) → `index-drift` (error, subject `""`) on its path, line 1 + the count of `\n` before the first differing byte; an output not walked (absent, outside the roots, excluded) → `index-missing` (error, line 1) on its path. Both messages name `<command>`. No `index = true` entry → neither rule runs: the comparison is opt-in.

None compared on a `walk_gap` (a `read_error`, an `UnreadableDir`, a written `MissingRoot`: each already "cannot check", and the render would lack documents). A skipped non-UTF-8 name does not stop it: its lossy line shows as `index-drift`, fixed by the rename `name-skipped` asks for. Causes of "cannot check" here: an output's bytes not supplied (`size` > 0, empty `bytes`); an `index = true` entry without `[paths] index` (only a config built in code: the TOML reader rejects it).

## §11.6: the generator registry

This repository's root `specengine.toml` has:

```toml
[[generators]]
command = "cargo run -q -p specengine-cli -- export index"  # a `generator:` value, compared byte for byte
writes  = ["docs/index.md", "docs/index-archive.md",  # root-relative, the [paths] path rules
  "docs/index-decisions.md", "docs/index-crates.md"]
index   = true  # optional: SpecEngine renders this output (§11.5)
gate    = "cargo run -q -p specengine-cli -- check"  # optional, index entry only; default "spec check"
shards  = [  # optional, index entry only (#index-shards)
  { path = "docs/index-archive.md", tier3 = true },
  { path = "docs/index-decisions.md", claims = ["docs/decisions/*.md"] },
  { path = "docs/index-crates.md", claims = ["crates/*/README.md"] }]
```

`specengine.toml:<line>: message`, and the run cannot check: an unknown key; a wrong type, a `[generators]` table included; `command` missing, blank or repeated across entries; `writes` missing, empty, a path breaking the `[paths]` rules, or a path shared with another entry; `index = true` twice; `gate` without `index = true`, or blank; `index = true` while `[paths] index` is absent or not in `writes`; an index entry `writes` path neither `[paths] index` nor a shard; `shards` off the index entry; a shard with an unknown key, no `path`, both or neither of `tier3` and `claims`, `tier3` not `true`, `claims` empty; a shard `path` or claim breaking the `[paths]` or `exclude` rules or holding a control character or a backtick (rendered into Markdown); a shard `path` equal to `[paths] index` or another's, or not in `writes`; `tier3 = true` twice (the first shard error only; claims matching nothing or overlapping: fine); `command` or `gate` not a plain YAML scalar — a newline or control character, leading or trailing whitespace, a leading YAML indicator (19: `YAML_INDICATORS`), `: ` or ` #` inside, a trailing `:`, `-->` (it would close the header comment) — or not reading back as the same string through the crate's own front-matter reader (`read_back`): null, booleans, numbers, and `.inf`, `-.inf`, `.nan`, the texts the reader gives non-finite floats. The table is outside the index fingerprint (`[ids]` only): editing it re-parses nothing.

WHEN the table is present (even empty), each `class: generated` document whose front-matter was read SHALL name a registered `command` in `generator:`, else `generator-unknown` (subject: the value, `""` if absent), and its path SHALL be in that entry's `writes`, else `generator-path` (the message names `writes`). Both errors sit on the `generator:` line, else line 1. A failed front-matter gives only its parser findings. Project generators are registered, never run.

## Graph warnings

**Live source**, for all three: neither `class: generated` nor Tier 3. A document whose front-matter failed is live; with `frontmatter-unclosed` the whole file is body, so its `id:` line can give `mention-dangling`: accepted noise. Warnings never block; a baseline entry (code, path, subject) turns one into debt.

**Resolution** (`Resolver`) as for front-matter references (`docs/canon/spec-check.md` "Rules"; feature-scoped IDs: `docs/canon/spec-check-links.md`). `project:` → `Skipped`, no finding. Width is never checked on recognition: a five-digit number is a real mention.

**`mention-dangling`**: one per distinct (line, subject) of a live source's unresolved inline mentions (`mentions`, origin `inline`): the report keeps one of equal findings, so a citation repeated on one line is one finding, as for `link-dangling`; subject as written, a wiki link's `[[…]]` included; the message as `ref-dangling`'s. Front-matter keeps `ref-dangling` (error). The lexer joins `#Y` to an ID only when `Y` is ID-shaped. Fallback, inline only: WHEN a mention of a `shape = "name"` prefix does not resolve, the check drops its last `-segment` and retries while prefix + one segment remain; the first resolving ID wins, its `#section` checked there (`MEC-STAMINA-based` → `MEC-STAMINA`). Never for front-matter or number shapes; case cannot decide (`TERM-exhausted` is a real ID).

**`depends-cycle`**: nodes are documents; an edge A → B for each `links.depends_on` item of a live A that resolves, from A, to an ID B holds (no fallback, `#section` ignored; several holders → an edge to each). `petgraph::algo::tarjan_scc`: one finding per strongly connected component of ≥ 2 documents, or of one with a self-loop. Subject: the member names (ID, else path) sorted by name then path, joined by `, `; placed on the first member's first `depends_on` item into the cycle; message "`depends_on` forms a cycle through <subject>".

**`ref-superseded`**: WHEN a live source references Y — declared (`refs`, `adrs`, `links.*`, `parent`, `working_answer`, a reference-form `canon:`) or inline (with the fallback) — and Y resolves to a document whose `status:` is `superseded-by X`, the check warns once per (line, subject), subject as written: "`Y` is superseded by X". Exempt: `supersedes:` items, `links.supersedes`, the `status:` value, references from X's own files. Several holders of Y: the first superseded one in path order.

**Determinism**: render, findings and cycle subjects do not depend on input order. **Genre** (ADR-0008): no project command (`cargo run -q -p specengine-cli`), `docs/`, `ADR` or `index.md` literal in the sources, no shard name; the ADR-0022 convention text (header, H1, `source:`, protocol line, section names, `Shards`, `Documentation index: `, `A shard of `) and `spec check` are literal.

This repository (root config, `enforce`, no baseline): `clean`, no warning.

## Next

- The root `[ids]` must not configure prefixes that collide with prose labels (Q-1…, AC-01…).
- Elsewhere: drift in project generators' output (ADR-0013); `@rev`, `project:`; citations of rejected decisions; `supersedes` ↔ `superseded-by` consistency; mentions in code (ADR-0016, Phase 3); fix data for `ref-superseded` (Phase 2).

## Open nits

- `Resolution::Resolved(Vec<usize>)` allocates per resolved reference; linear, left.
- Eval's `build_graph.rs` greps `File::`, also matching `ParsedFile::`, `CheckFile::` (closures work around it; `\bFile::` would be cleaner).
- The plain-scalar rule rejects a leading `-`, `?` or `:` though YAML reads `-x` as a string: conservative, accepted.
- `NotAString`'s message ("a null, a boolean or a number") is imprecise if serde-saphyr's `properties` is ever enabled.
