---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-http]
ref: RM-0072; Phase 7 step 10a (08 s2); the hub-git analysis and the orchestrator's decisions of 2026-11-03
---

# apply-outputs: the index in every apply commit, and a hub's own `PR-` range (RM-0072)

## Why

Since `hub-confirm` a confirm commits in the hub's checkout and nothing pushes it (`docs/canon/hub-confirm.md` "Rules"); RM-0051 (10b) will. Two defects would then reach everyone. **The index:** a create, or a title or status change, leaves `index-drift`, an error (`docs/canon/spec-check-graph.md` "§11.5: index drift"), because an apply commits its path alone (`docs/canon/proposal-apply.md` "Apply steps" 9-10); once pushed, an `enforce` hook stops another person's next Markdown commit. On a terminal this is N12 today (`docs/features/roadmap-items.md`), which parks RM-0019. **`Proposal:` numbers:** store `next_id` is the highest stored number + 1 (`crates/specengine-store/src/queue.rs:3110-3128`), and `spec import-state` goes on from the highest restored (`docs/canon/queue-backup.md` "Import" 5), so a hub filled from a terminal's dump and that terminal mint the same IDs, and "another proposal's same-ID commit on the branch blocks reject" (`docs/canon/proposal-completion.md` "Known limits").

This slice fixes both with no network verb, so 10b pushes only commits that are safe for others. Decided by the orchestrator, 2026-11-03:

- **The cut.** Step 10 splits at the first network verb: 10a this, 10b `hub-git` (RM-0051), which depends on it.
- **Terminals and hubs alike.** One apply library, steps unchanged on both (ADR-0061); RM-0019 is unparked.
- **Render from the committed tree** (`HEAD` plus the apply's paths), never the disk walk.
- **Only SpecEngine-rendered outputs:** a project's own generator commands never run (ADR-0013).
- **A walk gap** commits the target alone with a warning: never a partial render, never a refused apply (ADR-0012).
- **A separate stored `PR-` floor** from `PR-100001`, written once in a hub's data directory at its first start.
- **Spec-ID collisions** (records, created nodes) are a known limit, stated in canon; the cure waits for RM-0053.

## Description and interactions

### The outputs in an apply commit

Every caller of apply steps 8-10 (`spec approve`, `spec round answer`, a hub's `POST .../proposals/:id/confirm`) commits a spec change as it does today (`proposal-apply.md`, `proposal-create-apply.md`, `decision-record.md`), and also the outputs of the `[[generators]]` entry with `index = true` whose bytes the change alters: the root index, each shard, the roadmap view (core `render_index_set`, `spec-check-graph.md` "API"). Still one commit, the same subject, body and trailers (ADR-0005); the consent question is unchanged.

**The committed tree.** The render's input is `HEAD` at step 2's commit (step 8 re-checks it), the recorded root's subtree, its `specengine.toml` included, with each path the apply writes (the target, a new file or record; `--editorial` also `spec.lock`) taken at the bytes step 8 writes. That is the tree `git commit --only` records, and the tree a pre-commit hook's `check --staged` judges, since git names its temporary index in `GIT_INDEX_FILE` (`docs/canon/spec-check-git.md` "The staged check"). Not the disk walk: an uncommitted document would add a line the commit lacks, and a staged hook would refuse step 9; on a sparse checkout a root outside the cone is a written `MissingRoot`. Not the git index either: `--only` leaves other staged paths out.

**Which outputs are committed:** those whose rendered bytes differ from `HEAD`'s blob at that path, or that `HEAD` lacks. An update that changes no index line commits its path alone. No `index = true` entry in `HEAD`'s config: today's commit, byte for byte, no render. A drift item, `lock accept`, `lock rebase` and `verify --tests` write `spec.lock` alone and never render.

**An output is never a reason to refuse** (iteration 2, the orchestrator's decision on the review's REWORK). A gap, an output git will not stage, or an output the config may not name ("Never an output" below) narrows the commit to today's and adds a warning; the apply goes on and ends `applied`. Iteration 1 shows why: there a project that ignores its local `docs/index.md` has every create refused at step 9. A terminal prints the warning on stderr; on a hub, where no stderr reaches the page, the confirm also appends its text to the POST's review document's `notes`.

### A hub's `PR-` floor

`specengine-http --public-origin` stores the floor once in its data directory, beside `<slug>.db`, `daemon.lock` and `daemon.json` (`docs/canon/serve.md` "Files"). Every queue in that data directory then mints `PR-` no lower: a hub's departing accept, the operator's `spec propose` and MCP on the host alike. A data directory where no hub ever started holds no floor and mints as today. Nothing else changes: `proposal_id` already writes more than four digits (`crates/specengine-core/src/proposal.rs:39-55`, `crates/specengine-core/tests/patch.rs:242-244`), the UI takes `^PR-[0-9]{4,}$`, and the trailer, its lookup and every older `spec` read `PR-100001` as they read `PR-0001`.

## Data

**A synthetic project config** (the tests' shape; a pilot-shaped one is a synthetic config with roots beyond `docs/`):

```toml
[paths]
roots = ["docs"]
index = "docs/index.md"

[[generators]]
command = "spec export index"            # named in messages, never run
writes  = ["docs/index.md", "docs/index-archive.md"]
index   = true
shards  = [{ path = "docs/index-archive.md", tier3 = true }]
```

**The commit of a file-form create** in it, `git show --name-status --format= HEAD`:

```text
A	docs/features/stamina.md
M	docs/index.md
```

**Messages**, root-relative paths in config order; stdout and `--json` unchanged:

```text
note: the commit carries the index's outputs too: docs/index.md
warning: the commit carries `docs/features/stamina.md` alone: the index's outputs were not rendered: <cause>; `spec export index` writes them
warning: the committed `docs/index.md` is not the bytes this apply rendered; `spec export index` writes them
```

`<cause>`, the first met: `HEAD's specengine.toml is not a valid config (line N): ...`; `` `<p>` can never be an output (the apply's target | spec.lock | specengine.toml | .spec-debt.toml | outside the walk) ``; `walk_gap`'s first gap as `spec export index` words it (`docs/canon/spec-check-cli.md` "spec export index"); a blob missing from the object database; an output's inspection refusal in its words; `` `<p>` is ignored by git ``; `` `<p>` is `<q>` on disk (a case variant) ``; `` `<p>` is skip-worktree ``; `` staging `<p>` failed: <git's error> ``. The command is the entry's `command`, byte for byte. The second warning is step 10's blob compare; the apply stays `applied`.

**The floor file,** `<data dir>/proposal-floor`, a regular file: the decimal floor, no sign or leading zero, then LF; nothing else. Written atomically, so neither a reader nor a crash ever sees it empty or partial: a dot-named sibling `.proposal-floor.specengine-<pid>-<n>.tmp` written and synced, hard-linked to `proposal-floor` (`link` never replaces: an existing floor is read instead), the sibling removed, the directory synced, as store `create_file` writes a record.

```text
100001
```

Core: `HUB_PROPOSAL_FLOOR: u64 = 100_001`, the value a hub's first start writes; readers use the stored number. Store: the mint is `max(highest stored + 1, floor)`, the floor 1 when no file exists, in the inserting transaction as today.

**Bumps:** none. No queue schema, index format, `spec.lock`, event, `daemon.json`, daemon key, MCP schema or plugin change; the floor is a file, not a row, and no dump carries it.

## Rules and edge cases

- WHEN an apply reaches step 8 THEN it SHALL render from the committed tree before writing anything, then judge each changed output: never an output (next rule); inspected as `spec export index` inspects (each component below the root a non-symlink directory, the file absent or a non-symlink regular file, its parent existing, no two outputs one file); one `HEAD` lacks not ignored by git (store `is_ignored`); one on disk named byte for byte as configured (its directory entry read, so a case variant on a case-insensitive file system is caught); none skip-worktree. Only then the target is written, then each changed output atomically as the target is (a dot-named sibling, synced, renamed).
- **Never an output.** WHEN `HEAD`'s config names as an output the apply's own target (a new file or record included), a path named `spec.lock`, `specengine.toml` or `.spec-debt.toml`, or a path outside `HEAD`'s walk scope (`Paths::in_walk_scope`) THEN the apply SHALL narrow to the target alone, the warning naming that path. An apply writes only its target (`--editorial`: and `spec.lock`, by its own rule) and SpecEngine-rendered outputs, never a spec file or `spec.lock` by way of the config: the one writing door holds.
- **A gap.** WHEN `HEAD`'s config is absent, unreadable or invalid, it names a path that is never an output, `walk_gap` finds a gap over the committed tree, a listed blob is missing from the object database (a partial clone), an output fails a judge of step 8, or staging an output fails THEN the commit SHALL be today's, and the warning SHALL name the cause and the entry's `command`. No fetch: every git child sets `GIT_NO_LAZY_FETCH=1`, as the staged check's do.
- **Step 9** SHALL stage the outputs in a step of their own before the commit (one `HEAD` lacks: `git add --intent-to-add`, as a record's path). WHEN that staging fails THEN the outputs' index entries this run made SHALL be dropped (`git rm --cached --quiet --ignore-unmatch`), the outputs put back (each only while it holds this run's bytes; one absent before removed), and the commit made of the target alone with the warning: never step 9's refusal. Then `git commit --only` of the target and the staged outputs. WHEN the commit itself fails with no commit of this ID (a hook's refusal) THEN as today: the target restored, each output put back only while it holds this run's bytes (one absent before removed), this run's index entries dropped, reopened, exit 1.
- **Step 10** SHALL require the target as today (`--editorial`: and `spec.lock`) and no other changed path but written outputs, then compare each written output's blob in the new commit's tree with this run's bytes. WHEN one differs (two applies in one worktree, a hook rewriting it; a path the commit left unchanged holds `HEAD`'s blob, so it differs too) THEN the warning of "Data" SHALL name it; the proposal stays `applied` and nothing is refused. Then `update_paths` over the target and the outputs.
- **Completion** (`trailer_commit`, `file_commit`, `record_commit`) SHALL accept, beside what it accepts today, paths that are outputs of the `index = true` entry in the **parent's** root config, the tree the apply rendered from, never the commit's own config: added or modified, never deleted, their bytes unjudged (`spec check` judges them). Never accepted as outputs: what an apply may never write as one, the target, `spec.lock`, `specengine.toml`, `.spec-debt.toml`, a path outside the parent's walk scope; so a hand commit cannot authorise its own extra paths. The parent's config blob is read with `GIT_NO_LAZY_FETCH=1` (10b sets it on every worktree git child); none or invalid: no output accepted. Any other extra path is still `changes a, b`. `drift_commit` and the hint are unchanged.
- The render uses `HEAD`'s `[paths]`, scheme and generator entry, not the disk config step 3 read: the committed tree is what a hook judges. An index stale at `HEAD` (a document committed without its line) is brought up to date by the next apply's commit.
- The create refusal of an index path (`docs/canon/proposal-kinds.md` "Propose", 2; CLI `create.rs:554`) keeps its kind: no proposal targets an output. Its words "only `spec export index` writes it" become "SpecEngine renders it (`spec export index`, an apply's commit)".
- WHEN `specengine-http` starts with `--public-origin` THEN, holding `daemon.lock` and before listening, it SHALL write `proposal-floor` atomically when absent ("Data") and otherwise read it, never rewrite it; a leftover sibling from a crash is never read as the floor and does not stop a start. An invalid file: exit 2 before listening, `specengine-http: <data dir>/proposal-floor: not a proposal floor (a decimal number from 1, then LF)`. A local daemon and `spec serve` neither write nor read it at start.
- WHEN a mint reads an invalid floor THEN it SHALL store nothing: exit 2 for `spec propose`, the MCP tool's error, the same text. Refusing is not a verdict on content (ADR-0012): a guessed floor would mint the colliding IDs this slice removes.
- `spec import-state` step 3 judges tables alone, so a floor stored before the import does not refuse it (O9). `spec export state` never writes the floor.
- **Known limits, stated in canon at ship:** spec IDs stay per queue (`decision-record.md` "ID": the greater of the root's index and the queue's issued `record_id`; created IDs by `proposal-kinds.md` "Reservation", across one queue): a hub and a terminal can mint one, and an add/add conflict or `id-taken` after a merge shows it, a person renumbering; two developers' queues still share `PR-` numbers; a mint in a data directory before its hub's first start is below the floor; a hub's dump restored on a terminal mints in the hub's range; an older `spec` on the host ignores the floor; under a clean or eol filter the committed target differs from the bytes rendered (`proposal-completion.md` "Known limits"); after a failed commit an update's target is put back unconditionally (`replace_file`), not only while it holds this run's bytes as `--editorial`'s files and the outputs are, so an edit made in that window is lost: pre-existing, out of scope here.

## Acceptance criteria

Every test uses a local fixture repository and, where a remote is needed, a bare repository on disk.

- [ ] O1 — Terminal, the synthetic config: `spec propose create docs/features/stamina.md --text-file - --rationale R` and `spec approve PR-0001` (`y`): exit 0; name-status as in "Data"; stderr's note names `docs/index.md`; stdout byte for byte as before; afterwards `spec check` has no `index-drift` or `index-missing`, and `spec export index` prints `unchanged` for every output. M: skip the render.
- [ ] O2 — The same create raised on the hub's host (its data directory, so PR-100001) and confirmed by `POST .../proposals/PR-100001/confirm` as the `[team]` `owner`: 200; the commit's name-status equals O1's. M: render only on the terminal path.
- [ ] O3 — O1 with an untracked `docs/features/draft.md` and an unstaged edit of a document's `title:` in the worktree, and a `pre-commit` hook running the built `spec check --staged --root .`: exit 0; the committed `docs/index.md` equals `spec export index --stdout` in a clean clone of the commit and holds neither the draft's line nor the new title; the worktree's draft and edit are untouched. With the hook exiting 1 instead: the target and `docs/index.md` back to their prior bytes, `open`, exit 1. M: render from the disk walk; restore only the target.
- [ ] O4 — No `index = true` entry; a registered `command = "touch marker.txt"`: O1's apply commits `A docs/features/stamina.md` alone, as today, no note, and no `marker.txt` exists anywhere. M: render whenever `[[generators]]` exists; run the command.
- [ ] O5 — An update of a section's body in the synthetic project: the commit changes only the target path, no note. M: stage every output whatever its bytes.
- [ ] O6 — O1's commit made, `approved` left (a run killed between steps 9 and 10, or the same commit made by hand): `spec approve PR-0001` prints `` `PR-0001` completed by its commit <sha> on `<branch>`; no new commit `` and `HEAD` does not move; an update's commit with an output completes alike; a commit adding one more path beside them reads `changes ...` and completes nothing. M: keep "only the target's path changed"; accept any extra path.
- [ ] O7 — A deciding kind approved in the synthetic project: `A <record>` beside `M docs/index.md`; an interrupted one completes from that commit. M: keep "name-status exactly `A <path>`".
- [ ] O8 — `HEAD`'s config lists the root `notes`, present on disk and absent from `HEAD`'s tree: O1's apply commits the target alone, exit 0, `applied`, stderr's warning names the `MissingRoot` gap and `spec export index`. A symlinked `docs/index.md`: the same, naming it. On a hub the POST's review document holds the warning's text in `notes`. M: render the partial walk; refuse the apply.
- [ ] O9 — A dump of PR-0001..PR-0040 imported into a data directory where `specengine-http --public-origin https://hub.example.org` has started: `proposal-floor` reads `100001` and LF; a departing accept through that hub stores PR-100001, then `spec propose` on the host, same data directory, PR-100002; the dump in a fresh data directory with no hub: the next `spec propose` is PR-0041. A second hub start leaves the file's bytes and mtime; a local start writes none. M: `next_id` without the floor; the floor on every queue.
- [ ] O10 — Both queues from that dump: the hub confirms PR-100001 in clone A, the terminal raises PR-0041 in clone B, A's branch is merged into B's through a bare repository; `spec reject PR-0041` on the terminal: exit 0, `rejected`. M: no floor (both mint PR-0041 and the hub's commit blocks the reject).

Iteration 2, from the review's REWORK (the orchestrator's decisions):

- [ ] O11 — `docs/index.md` untracked and listed in `.gitignore`: O1's apply commits `A docs/features/stamina.md` alone, exit 0, `applied`, the warning reads `` `docs/index.md` is ignored by git ``, `docs/index.md` on disk untouched and not in `git ls-files`. On a case-insensitive file system, `HEAD`'s config naming `docs/Index.md` while `docs/index.md` is on disk: the target alone, the warning naming `docs/Index.md` as a case variant of `docs/index.md`, whose bytes are unchanged (skipped where the file system is case-sensitive). A staging that fails (the test makes `git add --intent-to-add` of an output fail): the target alone, the warning `` staging `<p>` failed: ... ``, no intent-to-add entry left, the outputs back to their prior bytes. M: refuse at step 9.
- [ ] O12 — `HEAD`'s config names the update's own target as a shard (the disk config does not), then, in a second case, `spec.lock` as a shard: each apply commits the target alone, the warning names that path, and the target holds the proposal's text while `spec.lock` keeps its bytes. M: write it.
- [ ] O13 — A hand commit with `Proposal: PR-0001`, one parent, changing the target, `docs/index.md` and `specengine.toml`, whose own `specengine.toml` adds `specengine.toml` as a shard while the parent's does not: `spec approve PR-0001` completes nothing, reads `changes ...` naming `specengine.toml`, and stays `approved`. A second hand commit whose own config adds the shard `docs/extra.md`, which it also adds: the same, naming `docs/extra.md`, since only this case tells the parent's config from the commit's (the first is refused by the four paths alone). The same commit with neither extra path completes. M: read the commit's config; accept the four paths as outputs.
- [ ] O14 — A `pre-commit` hook appends a line to `docs/index.md` and stages it: O1's apply ends exit 0, `applied`, and step 10's warning names `docs/index.md` and `spec export index`. M: check names only.
- [ ] O15 — A test seam pausing the floor's write after each step (sibling written, synced, linked, sibling removed, directory synced): at every pause `proposal-floor` is absent or reads `100001` and LF, never empty or partial; a start killed at a pause, then a second start, ends with `100001` and LF and no sibling read as the floor. M: `create_new` then write.

## Out of scope

Any network verb, merge or push, and an index conflict rendered again in a merge commit: 10b (RM-0051). The gap warning in the confirm dialog: 10b, which owns the hub's UI line. Spec-ID collisions: a known limit, RM-0053. A project's own generator commands (ADR-0013). Moves from the Roadmap screen: RM-0019, unparked, not built. The rehearsal (`dry_run`, `spec approve --open`'s preview, the confirm read) renders nothing. `spec review` still lists a create's `index-drift` among introduced findings. An update's target put back unconditionally after a failed commit ("Known limits"). Rejected: a qualified trailer; a flag-only floor; floors for `T-`, `C-` (`docs/canon/cards.md` "Store": a hub is the one `C-` minter) or record IDs.

## Implementation

To fill in after implementation.

### Files in scope

| Module | What it changes |
|---|---|
| core `proposal.rs` | `HUB_PROPOSAL_FLOOR`; the floor's parse and print |
| store, new `outputs.rs` or in `base.rs` | a `Source` over `HEAD`'s tree under the root, as `--changed` builds its base (`Staged::head_only`, `GitIndex::from_walk`), the apply's paths overlaid; `GIT_NO_LAZY_FETCH=1` |
| store `queue.rs`, new `floor.rs` | `next_id` with the floor; `proposal-floor` read, and written as `create_file` writes (sibling, sync, hard link, remove, directory sync) |
| store `worktree.rs` | `commit_only_paths` takes the outputs; the outputs' own staging and its undo; `is_ignored`, skip-worktree, the on-disk name |
| CLI `apply.rs` (`:910`, `:1075`), `create.rs` (`approve_file`, `:1541`, `:554`), `decide.rs` (`approve_record`, `:630`) | render, "Never an output", judge, write, stage, restore, step 10's blob compare, `update_paths` over outputs; the refusal's words |
| CLI `preflight.rs` | `trailer_commit`, `file_commit`, `record_commit` accept the parent config's outputs, never the four paths; its blob read with no lazy fetch |
| http `main.rs`, `start.rs` | the floor at a hub's start |

Tests: CLI new `tests/apply_outputs.rs` (O1, O3-O8, O11-O14), http `tests/hub_confirm.rs` (O2, O8's hub half), new `tests/hub_ids.rs` (O9, O10, O15).

### Canon at ship

Nothing below is written before this slice's commit. Free bytes on 2026-11-03; a page without room rewords in place.

- **`docs/canon/proposal-outputs.md`, new Tier 2** (`proposal-*` shard): the committed tree, which outputs, "Never an output", the judges, the gap and its warnings (an output never refuses), steps 8-10 with the outputs' own staging and step 10's blob compare, completion by the parent's config, known limits. Every full page below points at it.
- **`architecture.md`** (1 B) `#apply`: "only its registered generator writes it (`spec export` the queue, a project's generator its output)" becomes "the queue `spec export`; the `index = true` entry's outputs SpecEngine, by `export index` or in an apply's commit (ADR `apply-outputs`); a project's generator its own".
- **`proposal-apply.md`** (1 050 B): the intro's "and one commit ...; nothing else" adds the changed outputs; step 8 renders, judges and writes them; step 9 stages them on their own, a failed staging narrowing, then commits them; step 10 "changes only the path" becomes the path and written outputs, blobs compared, a mismatch a warning, `update_paths` over both; "Known limits": an update's target put back unconditionally after a failed commit.
- **`proposal-completion.md`** (1 240 B) "Completion": "only the target's path changed" adds the outputs the parent's config names, `A` or `M`, unjudged, never the target, `spec.lock`, `specengine.toml` or `.spec-debt.toml`, the parent's config read with no lazy fetch; the hint stays as it is; "Known limits": the same-ID line adds two terminals' queues from one dump, a hub minting from its floor.
- **`decision-record.md`** (2 807 B): step 9 commits the outputs; step 10 and "Completion": `A <path>` beside the outputs; "Known limits": a hub and a terminal can issue one record ID, a merge shows it.
- **`proposal-create-apply.md`** (8 435 B): file form steps 9-10 and "Completion" beside the outputs; the hint unchanged.
- **`hub-confirm.md`** (2 287 B) "Rules", its **Where it ends** bullet: "A new file leaves the index stale as on a terminal (N12)" becomes "the commit carries the index's outputs as a terminal's"; "`Proposal:` numbers collide only once pushed" becomes "a hub's `PR-` start at its floor".
- **`queue-backup.md`** (2 725 B): a new "ID floor" (`<a id="id-floor">`): the file, its atomic write, who writes and reads it, the mint, import and export, the spec-ID limit; "Import" 5's "highest restored + 1" adds "never below the floor".
- **`proposal-queue.md`** (3 B) "Store": `id`'s "highest number + 1" adds "never below the data directory's floor (`queue-backup.md` "ID floor")".
- **`hub.md`** (24 B) "Trust": "A hub stores nothing new" becomes "A hub stores one file more, its proposal floor".
- **`serve.md`** (2 540 B) "Files": `proposal-floor` beside `daemon.json`.
- **`proposal-kinds.md`** (9 B) "Propose", 2: the refusal's new words; no growth.
- **`spec-roadmap-screen.md`** "3b parked: N12" becomes "3b unparked (ADR `apply-outputs`), not built"; **`spec-check-cli.md`** "spec export index": an apply renders the same outputs from the committed tree.
- **Unchanged:** ADR-0005, never edited (ADR `apply-outputs` amends it, still one commit); `cards.md` "Store" (a hub already the one `C-` minter).
- **Specs:** `roadmap-items.md` N12 issued as ADR `apply-outputs`; `roadmap-view.md` "N12 wider" met; RM-0019 unparked, and 08's Phase 6 line; this file compressed.
- **Tier 1** (free: `docs/README.md` 34 B "Generated", the apply's commit too; CLI 7 B "Rules": approve, round answer and confirm commit the outputs; store 5 B; core 8 B; http 23 B, the floor at start): reword in place. **`CLAUDE.md`**: `apply-outputs` shipped, the ADR range extended to `hub-ids`, "The hub goes on from step 10b, RM-0051".

Draft ADRs, the next free numbers, each under 1.5 KB with its front-matter, the date set at ship:

```markdown
---
class: decision
id: ADR-0062
title: An apply commit carries the index's outputs, rendered from the committed tree
status: accepted
date: 2026-11-DD
scope: [architecture]
canon: docs/canon/architecture.md#apply
ref: apply-outputs (RM-0072), N12 of roadmap-items; the orchestrator's decisions of 2026-11-03
---

# ADR-0062

**Decision.** Amends ADR-0005's commit and the generated rule: every apply, on a terminal and on a hub, commits beside its spec file the `index = true` entry's changed outputs, rendered by SpecEngine from the committed tree (`HEAD` plus the apply's paths), never the disk walk; still one commit. A project's generator command never runs (ADR-0013); with no `index = true` entry nothing changes. The target, `spec.lock`, `specengine.toml`, `.spec-debt.toml` or a path outside the walk is never an output. An output never refuses an apply (ADR-0012): a gap, or one git will not stage, commits the target alone with a warning.

**Why.** A create or a status change left `index-drift`, an error, so an `enforce` hook stopped the next Markdown commit, and a hub's pushed commit would stop everyone's. The committed tree is the one a staged hook judges.

**Cost.** Applies touch generated files, so two branches' applies conflict on the index. Completion accepts the outputs the parent's config names, their bytes unjudged. Each apply reads `HEAD`'s tree under the root; a partial clone missing blobs gets no index.
```

```markdown
---
class: decision
id: ADR-0063
title: A hub's queue mints proposal IDs from a stored floor, PR-100001 upward
status: accepted
date: 2026-11-DD
scope: [architecture]
canon: docs/canon/queue-backup.md#id-floor
ref: apply-outputs (RM-0072); the orchestrator's decisions of 2026-11-03
---

# ADR-0063

**Decision.** A hub stores a proposal floor, `100001`, once in its data directory at its first start; every queue of that data directory, a host terminal's too, mints `PR-` no lower. A terminal's data directory holds none and mints as before. The trailer, the lookup and older binaries' reads are unchanged. Spec IDs (records, created nodes) stay minted per queue: a collision is a known limit until RM-0053.

**Why.** A queue restored from a dump goes on from its source's highest number, so a hub and a terminal minted one `Proposal:` ID, and one's commit on a shared branch blocked the other's reject. A qualified trailer needs a queue identity, changes the lookup on both sides (ADR-0005) and protects no older binary; a flag-only floor lets a host terminal mint low before the hub's first mint.

**Cost.** Longer hub IDs; a hub stores one file more. Two developers' queues still collide. A hub's dump restored on a terminal mints in the hub's range, and an older `spec` on the host ignores the floor.
```
