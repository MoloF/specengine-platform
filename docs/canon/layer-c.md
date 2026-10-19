---
class: canon
tier: 2
scope: [crates/specengine-cli, crates/specengine-store, crates/specengine-code]
owner: owner
reviewed: 2026-10-19
---

# Layer C: `spec resolve` and the advisory snapshot

`spec resolve [--timeout S] [--json]` is the one command that spawns `specengine-ra` (protocol v1: `layer-c-protocol.md`): one cold load, the reply checked line by line, stored after a complete reply over an unchanged tree as the worktree's one advisory snapshot in the data directory. No other command, read, MCP tool, daemon route or hook spawns it; no read shows the snapshot yet (11a-3, `docs/features/layer-c-reads.md`). The rule: `architecture-code.md#code-identity`, ADR-0049. Code: CLI `layer_c.rs` (steps, freshness, counts, output), `layer_c_run.rs` (a run), `layer_c_reply.rs` (checks), `group.rs` (the process-group core of `verify --tests` too, `spec-verify-tests.md` "Runner"); store `layer_c.rs`; `specengine-code` `layer_c`. Built by `docs/features/layer-c-resolve.md`.

## Run

S: `--timeout` seconds, 1..=3600, default 300 (`RESOLVE_TIMEOUT_{MIN,MAX,DEFAULT}`; clap and the library both check, `spec: --timeout N is out of range: 1 to 3600 seconds`). In order, each failure exit 2, nothing stored:

1. Unix only (`spec: spec resolve runs on Unix only`); discovery, the config, `[code]`, the spec and code indexes refreshed as the reads refresh them, a walk gap refused, in the reads' `spec: …` forms. `files`: every code-index file of `lang` `rust` read and UTF-8, root-relative, in the walk's byte order (`arena-derive/` first), never re-sorted by `Path`; none: `no Rust file to resolve`. The fingerprint's entries taken ("Freshness").
2. `<data dir>/<slug>.ra/` and its `target/`, `tmp/` created; `File::try_lock` on its `lock`, held to the store; D: that directory, canonicalised.
3. `specengine-ra` beside `current_exe()`, else the first `PATH` entry holding an executable regular file of that name (`serve.md` "Launcher"); a relative hit made absolute against `spec`'s cwd.
4. Spawned (never exec) by the group core: exactly `--protocol 1`, cwd `D/tmp`, the leader of its own process group; `spec`'s environment unchanged (the binary scrubs it, sets `TMPDIR`: `layer-c-protocol.md` "Environment").
5. Each line judged as it arrives ("Checks"); nothing written. At S from the spawn the group gets `SIGTERM`, 5 s (`GRACE`), `SIGKILL`; a fault: the same at once, stdin closed. `SIGINT`, `SIGTERM`, `SIGHUP` to `spec` (`Forwarding`): the same, `interrupted`. The first trigger wins over any exit seen after it. The binary's own budget watchdog (S + 10 s) is a fallback only.
6. After `done` and exit 0: the code index refreshed again, the entries taken again; a walk gap or any difference: `the tree changed during the run` (its notes dropped); else one `Immediate` transaction replaces the worktree's rows, the lock is released, the join counted.

## Request

One line, exactly the six keys in this order, `\n` ended:

```json
{"protocol":1,"root":"/abs/root","files":["arena-derive/src/lib.rs","arena/build.rs"],"target_dir":"/abs/data/ra-mini.ra/target","tmp_dir":"/abs/data/ra-mini.ra/tmp","budget_ms":300000}
```

`root` canonical; `target_dir`, `tmp_dir` `D/target`, `D/tmp`; `budget_ms` S × 1000. A non-UTF-8 path cannot be written (`<path> is not UTF-8: the request names UTF-8 paths only`). `spec` copies none of the binary's request checks: a refusal is reported, never pre-empted.

## Stdin, stdout, stderr

- **stdin**: written on its own thread while S runs (never read: an overrun); a broken pipe unreported (the reply and the exit decide); held open until the last line (`done`, `error`, `refused`) is read, a fault found or the group gone, then closed: the binary then exits 0 or 1; closed earlier, it kills its group.
- **stdout**: lines read on a thread. A line's bytes before `\n` over `REPLY_LINE_CAP` (8 388 608; exactly the cap passes) are a fault at once, even where EOF would cut the line; a last line without `\n` is not judged, the exit decides. Once the group is gone, read up to 5 s more (a process outside the group may hold it).
- **stderr**: inherited, interleaved live with `spec`'s. Risk: under `stty tostop` a background `spec`'s binary stops (`SIGTTOU`) at its first stderr write until S ends the run.

## Checks

The first fault ends the judging: `malformed reply at line <n>` (from 1) or the mismatch.

- The reply is `refused` alone, or begins with `hello`. The `protocol` of a `hello` or `refused` line, wherever it stands, is read before its other checks: an integer ≠ 1 is the mismatch, a string or float malformed.
- `step`: workspaces numbered from 1 without a gap, each `discover` then `database` with one `dir` (any string: `.`, `a/b`, `..` segments; repeated, unsorted); a lone `discover` only before an `error`. `discover`: `toolchain` a string or null, `sysroot_packages`, `metadata_degraded`.
- The end: an `error` (after `file` lines too) whose `workspace` is 0 ..= the last step's number + 1, `category` `metadata`, `database` or `path` (`build_scripts`, which v1 never writes, is malformed); or, after every step, one `file` line per requested path in request order, then `done`.
- `file`: items strictly by `start` (`end` unchecked); `kind` one of `KIND_MAP`'s 13; `state` one of five; `moniker` a string exactly when `state` is `moniker`. `done`: `files`, `items`, `by_state` (five keys) equal the tallies; `peak_rss` an integer or null; `ms`.
- Every line: a JSON object with exactly its type's keys (a repeated key folds into one); none after the last.

## Output and exits

Exit 0, ra-mini (debug):

```
layer c: specengine-ra protocol 1, ra_ap 0.0.352, host aarch64-macos
workspace 1 .: discover 285 ms (toolchain 1.98.1, sysroot 26, degraded no), database 580 ms
files 6 (in crate 4), items 29: moniker 26, local 0, none 0, unresolved 2, not_loaded 1
joined 29, unmatched 0, c_only 0; agrees 26, disagrees 0; collisions 1; stored fresh, 21.4 s, peak 0.5 GiB
```

- Null stamps: `toolchain unknown`, `peak unknown`; `dir` as given, controls escaped. `--json`: `{protocol, ra_ap, host, workspaces: [{dir, toolchain, sysroot_packages, metadata_degraded, discover_ms, database_ms}], files, in_crate, items, by_state, joined, unmatched, c_only, agrees, disagrees, collisions, ms, peak_rss}`; notes on stderr.
- Timing (`ms`, `s`, `GiB`, `peak_rss`) on `resolve`'s stdout only, the CLI's one exception to "no timing"; never stored.

Exit 2: stdout empty, `spec resolve: <reason>; the previous snapshot is kept`, the snapshot untouched.

| The binary ends | Reason |
|---|---|
| `done`, exit 0 | none: stored; or `the tree changed during the run` |
| `error`, exit 1 | `load failed: <category> (workspace <n>)`; n 0: `(files under no manifest)` |
| `hello` or `refused`, `protocol` ≠ 1 | `specengine-ra speaks protocol <n>, spec needs 1: install both from one commit` |
| `refused`, exit 2 | `specengine-ra refused the request: <reason>`, verbatim |
| any other last line and exit | `specengine-ra ended (exit <c>) before done`, `(signal <s>)`; `after done` once `done` was read |

A trigger first: `over the budget of <S> s`, `interrupted`, `malformed reply at line <n>`. Before the spawn: `no Rust file to resolve`; `a layer C run of <slug> is in progress`; `specengine-ra not found beside <spec> or on PATH; install it: cargo install --locked --force --path <clone>/crates/specengine-ra (rustc >= 1.98, rust-src)`; `cannot create`, `open`, `lock`, `resolve <path>: <error>`; the not-UTF-8 path; `cannot write the request: <error>`. Then `specengine-ra cannot be started: <error>` (a `#!` to a missing interpreter too), `specengine-ra cannot be waited for: <error>`, `cannot store the snapshot: <error>`. Accepted: the counts failing after the store exit 2 over a new snapshot.

## Tables

`INDEX_FORMAT` 11 (`symbol-index.md` "Tables"): `STRICT`, in `FORMAT_TABLES` children first, before `code_worktrees`; columns: store `schema.rs`. Outside `CODE_TABLES` and 6a's dump-equals-rebuild: `CODE_INDEX_FORMAT` stays 4.

| Table | Key | Columns |
|---|---|---|
| `code_resolve` | `wt` -> `code_worktrees`, cascade | `tree_fp`, `protocol`, `ra_ap`, `host`, `toolchains` (JSON: the workspaces' distinct non-null toolchains, byte-sorted), `degraded` (any `metadata_degraded`), `workspaces`, `files`, `items` |
| `code_resolve_files` | `(wt, path)` -> `code_resolve`, cascade | `blake3` (the code index's at the run), `in_crate` |
| `code_monikers` | `(wt, path, start)` -> `code_resolve`, cascade | `"end"`, `kind`, `state`, `moniker` (raw; index `code_monikers_by_moniker`) |

- **Writes**: `SqliteIndex::replace_layer_c(&LayerCSnapshot)` only: one `Immediate` transaction, delete then insert, rows by `(path, start)`; no `code_worktrees` row: `NotIndexed`. **Reads**: `layer_c_stamp()` (`tree_fp`, `protocol`, `ra_ap`, `host`), `layer_c_snapshot()`, `code_tree_rows()` (the code index's files, packages). The store knows no protocol word, Cargo name or moniker syntax.
- **Lifetime**: `clear_code` (no `[code]`) and an `INDEX_FORMAT` change drop it; `spec index --full`, an `exclude` change, a `CODE_INDEX_FORMAT` change keep it. No `c_only` column: the join runs per read.
- **Dump**: `wt` as `(project, root)`; no absolute path or time stored: two roots, runs or creation orders, identical rows.

## Freshness

`tree_fp`: `b3:` BLAKE3 of entries, each a JSON array, unique, byte-sorted, joined by `\n`:

- `["protocol",1]`, `["ra_ap",…]`, `["host",…]`: from `hello`; `layer_c_state` takes the stored ones;
- `["file",path,blake3]` per requested file; `["package",dir,manifest_b3]` per `code_packages` row;
- per `TREE_FILES` name (`Cargo.lock`, `Cargo.toml`, `rust-toolchain`, `rust-toolchain.toml`, `.cargo/config`, `.cargo/config.toml`) in the root and each directory from a walked file's (any `code_files` row) up to it, never above (`tree_files`; a path with `..`, `.`, an empty part, or absolute adds the root only): `["bytes",name,b3]` for a regular file, `["unreadable",name]` if it cannot be read; absent or a directory: none. A virtual workspace root's `Cargo.toml` is covered. The names are `specengine-code`'s; the CLI hashes bytes.

`layer_c_state(&Env, &Globals) -> LayerCState`, public: the spec and code indexes refreshed first, exit 2 as the reads (no `[code]`, a config error, a walk gap); `absent` (no snapshot), `fresh` (equal), `stale` (any change: every row stale, its values kept); never spawns. `state_in(&SqliteIndex, root)`, crate-private, over an index the caller refreshed: 11a-3's reads, inside their own refresh.

Not covered (stamped at most): cargo configs above the root or in `$CARGO_HOME`; `rustup default`; a workspace root above the project root (`dir` with `..`); a reinstalled binary of the same `ra_ap` and `host`.

## Join

Per file of `code_resolve_files`, layer A's `symbols` rows against its `code_monikers` rows (`specengine_code::layer_c`):

- **Rows**: `join_file`: equal `start` and `end`, `kind_matches` (C's kind, A's); never a line or a name. `joined`, `unmatched` (A rows with none), `c_only` (C rows no A row joins).
- **`agrees`**, joined `moniker` rows of a file with one target (`target_crate`: `TargetIndex::file_role` over the package's stored `targets`; no unit: the primary target's name, else the unit's; `-` -> `_`): the moniker's crate (before its first space) equals it, and its leading `name/` descriptors before the item's own last (`moniker_modules`: a `mod` row's own `name/` is no module) equal A's modules, the file's module path plus the enclosing inline `mod` names by `parent_ord`; names and owners not compared. A shared or unrooted file, a `path_attribute` or `unrooted` row: neither. False: counted and shown, never fixed or a finding.
- **Collisions**: `collision_groups`, two or more C rows of one file sharing a raw moniker; `collisions` counts groups. Shown as `<raw> #<value>`: the joined A rows' discriminators when all present and distinct, else the 1-based ordinal by `start` (`collision_values`, `shown_moniker`; ` #` never in raw text).
- **Precedence**: the marker binds; the qpath names for `spec.lock` and `verify`; the moniker displays and finds.
- Known limit: `moniker_modules` stops at the first non-namespace descriptor character, so a raw-identifier module `r#name` may miscount (TE-6 watches).
