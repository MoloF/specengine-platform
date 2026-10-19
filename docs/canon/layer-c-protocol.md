---
class: canon
tier: 2
scope: [crates/specengine-ra, crates/specengine-code]
owner: owner
reviewed: 2026-10-19
---

# Layer C protocol v1: the `specengine-ra` executable

rust-analyzer (`ra_ap_*` =0.0.352) runs only in the executable `specengine-ra` (`crates/specengine-ra`, `[[bin]]` `src/main.rs`, outside `default-members`): a pure function of one request and the disk, protocol v1 on stdin and stdout (ADR-0049; the rule: `architecture-code.md#code-identity`). It runs no build script, proc macro or `cargo check` and writes nothing under the request's root. Only `spec resolve` spawns it (`layer-c.md`). The library under it: `crates/specengine-ra/README.md`.

## Run

`specengine-ra --protocol 1`, exactly these arguments; the request is one line on stdin. A caller spawns it as the leader of a process group of its own, writes the request line and keeps stdin open until it has the final line (it may then close stdin and wait: exit 0 or 1): EOF on stdin earlier means the caller is gone. By hand, hold stdin open: `{ cat request.json; cat; } | specengine-ra --protocol 1` (`< request.json` alone reaches EOF at once: the run ends after `hello`). A watchdog ends the whole group only when the binary leads it (`getpgrp() == getpid()`), else the binary alone. Started by a shell with job control alone or as a pipeline's first command, it leads its job's group and a watchdog kills that job; later in a pipeline, or without job control, another process leads and it exits alone, 3.

Install: `cargo install --locked --force --path crates/specengine-ra`, beside `spec`, from the same commit; rustc ≥ 1.98 with `rust-src` (without it answers degrade, stamped by `sysroot_packages`).

## Request

```json
{"protocol":1,"root":"/abs/root","files":["arena/src/lib.rs","tools/probe.rs"],"target_dir":"/abs/out/target","tmp_dir":"/abs/out/tmp","budget_ms":300000}
```

Checked whole, in this order, the first fault refusing it, before any cargo or rustc call:

- the line ≤ 64 MiB, not blank; a JSON object with exactly these six keys, each required; `protocol` 1;
- `root`: a clean absolute path (no `.`, `..` or empty segment, no trailing `/`, no NUL) of an existing directory, then canonicalised: all later work uses the canonical root;
- `target_dir`, `tmp_dir`: clean absolute paths, neither the root nor under it, compared as written against the root as written and with the longest existing ancestor canonicalised against the canonical root (a symlinked spelling is caught);
- `budget_ms` ≥ 1;
- `files`: clean root-relative paths (no leading `/`, `.`, `..` or empty segment), byte-sorted, unique, each an existing regular file; `[]` is allowed (`hello`, then `done` with 0 files);
- `target_dir` and `tmp_dir` created; a failure refuses too.

`hello` is written only after all of this, so a refusal is the whole reply.

## Reply

JSON lines on stdout, key `t` first, each written whole and flushed. ra-mini, abridged (two of six `file` lines; masked whole in `fixtures/layer-c/ra-mini.jsonl`):

```json
{"t":"hello","protocol":1,"ra_ap":"0.0.352","host":"aarch64-macos"}
{"t":"step","workspace":1,"dir":".","step":"discover","ms":235,"toolchain":"1.98.1","sysroot_packages":26,"metadata_degraded":false}
{"t":"step","workspace":1,"dir":".","step":"database","ms":313}
{"t":"file","path":"arena/src/lib.rs","in_crate":true,"items":[{"start":278,"end":339,"kind":"struct","state":"moniker","moniker":"arena Health#"}]}
{"t":"file","path":"tools/probe.rs","in_crate":false,"items":[{"start":74,"end":86,"kind":"function","state":"not_loaded","moniker":null}]}
{"t":"done","files":6,"items":29,"by_state":{"moniker":26,"local":0,"none":0,"unresolved":2,"not_loaded":1},"peak_rss":571441152,"ms":20474}
```

- `hello`: `ra_ap` the pinned release; `host` `std::env::consts` arch-os.
- `step` `discover`: `cargo metadata` of the workspace and the sysroot; `toolchain` rustc's version or null; `metadata_degraded`: metadata with dependencies failed and `--no-deps` was used (the text on stderr). `step` `database`: files and crate graph loaded.
- `file`: `in_crate`, the file is in some crate's module tree; `items` by `start`, each:
  - `start`: the byte of the first token after the outer attributes and the comments rust-analyzer attaches (doc comments, a marker line), on the raw text (CRLF kept); `end`: past the last token. Equal to layer A's item range on the same bytes.
  - `kind`: `function`, `method`, `struct`, `enum`, `union`, `trait`, `impl`, `const`, `static`, `type_alias`, `mod`, `macro_rules`, `macro_def`: layer A's item set, not entering bodies or macro invocations.
  - `state`: `moniker`; `local` (a local moniker only); `none` (resolved, no moniker); `unresolved` (`cfg`-disabled, or the file in no crate's module tree); `not_loaded` (the file parsed alone, "Order").
  - `moniker`, with `moniker` only, else null: the crate name, a space, SCIP-style descriptors, no version: `a/` namespace, `T#` type, `f().` method, `x.` term, `[T]` type parameter, `(p)` parameter, `m!` macro; an impl `impl#[Self][Trait]` (`arena systems/impl#[Health][Tick]tick().`). Two inherent impls of one type share one moniker.
- `done`: `files`, `items`, `by_state` equal the `file` lines' tallies; `peak_rss` in bytes, `getrusage` `ru_maxrss` of the process plus that of its largest reaped child (KiB × 1024 on Linux; null where the unit is unknown); `ms` wall time since start.

## Order

`hello`; the workspaces one at a time, each its `discover` then `database` step as they happen; then one `file` line per requested path, in request order; `done` last.

- A file's workspace is the one its nearest `Cargo.toml` belongs to, looked up from the file's directory to the root, never above. Those manifest directories are taken in the byte order of their root-relative form (`.` the root): each is discovered (cargo resolves its workspace root), claims every pending directory whose `Cargo.toml` is that workspace's root or member manifest, is loaded, its files scanned, and closed before the next.
- `workspace` numbers them from 1 in that order. `dir` is the workspace root cargo resolved, relative to the root: `.`, `a/b`, or with `..` segments when it lies above the root. The order equals the order by `dir` except where a workspace's first member's directory sorts after an unrelated workspace's while its own root sorts before it.
- A file under no manifest, or one its workspace's load did not take (outside every package root, excluded), is parsed alone from its raw text: `in_crate` false, every item `not_loaded`.
- A file in two crates gets one crate's answer. `cfg(test)` is on.

## Endings

| Ending | Last line | Exit |
|---|---|---|
| complete | `done` | 0 |
| a step failed | `{"t":"error","workspace":n,"category":"…"}` | 1 |
| bad argv or request | `{"t":"refused","protocol":1,"reason":"…"}`, the only line | 2 |
| outside the protocol | none, the reason on stderr | 3, or SIGKILL |

- `error` `category`: `metadata` (the manifest, `cargo metadata`, toolchain queries, or a rust-analyzer panic while discovering); `database` (the load, or a rust-analyzer panic while loading or scanning a file); `path` (a requested file unreadable or not UTF-8: the first in request order within the first workspace, in manifest-directory order, that has one; files under no manifest last; or a path rust-analyzer cannot take); `build_scripts` is never written (no build script runs). `workspace` is the failing workspace's number, after its steps: all its requested files are read before any is scanned, so a bad module file is `path` even after a sibling. 0 for a file under no manifest: right after `hello` when no requested file has a manifest, else after every workspace's steps. Every text (`LoadError`, panic, I/O) goes to stderr only: it may name paths.
- `refused`: no cargo or rustc has run, nothing under the root was written; `protocol` is the binary's own.
- Exit 3: a watchdog fired, or stdout closed (EPIPE), while the binary does not lead its group; or the analysis thread failed outside a guarded step. Leading its group, a watchdog or a closed stdout ends in `kill(0, SIGKILL)`, the thread then parked, never exiting: death by signal. Once `done` or `error` is written, a firing watchdog exits with that line's code, 0 or 1, silently, and kills nothing; while it is being written (a mark set before its first byte), the watchdog waits up to 1 s, then ends the run as before.

## Environment

First in `main`, single-threaded: the seven toolchain overrides `specengine-eval ra` removes for its worker are removed (`RUSTUP_TOOLCHAIN`, `CARGO`, `RUSTC`, `RUSTC_WRAPPER`, `RUSTC_WORKSPACE_WRAPPER`, `CARGO_TARGET_DIR`, `CARGO_BUILD_TARGET_DIR`), then `RUSTC_WRAPPER` and `RUSTC_WORKSPACE_WRAPPER` set to `""`: cargo runs no wrapper, over a project's `build.rustc-wrapper` and `build.rustc-workspace-wrapper` too. Once the request is accepted, still single-threaded, `TMPDIR` = `tmp_dir`: the copy of a project's `Cargo.lock` goes there. Every cargo and rustc rust-analyzer starts inherits the result. rust-analyzer finds `rustc` on `PATH`, then in `$CARGO_HOME/bin`, and `cargo`, once a sysroot is known, in `$CARGO_HOME/bin` first (`ra_ap_toolchain` `prefer_proxy`), each with `RUSTUP_AUTO_INSTALL=0`: rustup never installs a toolchain.

## Watchdogs

Armed once the request is accepted, before `hello`; none waits on the request line itself (the caller's budget covers it):

- stdin reaches EOF or fails: the caller is gone;
- `budget_ms` + 10 s since start: the caller failed to act on its budget.

Either ends the run as "Endings" says, the group taking cargo, rustc and what they started.

## Read-only

- Each workspace is discovered with `proc_macros: false`: `cargo metadata --locked --offline`, resolved against the lock copied into `tmp_dir`, its target directory `target_dir`; no `cargo check`, build script or proc-macro server; then loaded, scanned, closed.
- Writes: `target_dir` and `tmp_dir` only; the root's bytes and mtimes stay, no `target/` appears under it.
- Known limits: a project's `build.rustc` and `[env]` (cargo configuration) and `CARGO_BUILD_RUSTC` in the caller's environment still reach cargo; Unix only (elsewhere `peak_rss` null and no group kill); no memory guard (≈ 3.2 GiB cold on the heavier pilot, 05 §5.1); no attribute expansion is followed (11b).
- Known limits of the answer: workspace membership follows cargo's spelling of member paths, so a file reached through a symlinked alias directory gets cargo's refusal (`error` `metadata`), while a member listed through a symlink works under its listed spelling; a non-UTF-8 module file of a scanned crate that is not requested ends the run with `error` `database` (rust-analyzer panics on its missing text; stderr names the scanned file and a `FileId`, not the culprit).

## Kind map

Layer C's `kind` against layer A's node kinds, for joining rows on `(start, end, kind)`: `specengine_code::layer_c` (`KIND_MAP`, `layer_a_kinds`, `kind_matches`): `method` -> `function_item`, `function_signature_item`; `type_alias` -> `type_item`; `mod` -> `mod_item`; `macro_rules`, `macro_def` -> `macro_definition`; any other `<kind>_item`. SCIP text is data in `specengine-ra` and `specengine-code` only (`architecture.md#universal`).

## Tests

`crates/specengine-ra/tests/protocol.rs` (Unix): the real binary on scratch copies of `fixtures/ra-mini`, under `alarm`, leading its own group, behind `cargo` and `rustc` shims (`PATH`, a scratch `$CARGO_HOME/bin`): order and tallies; the masked golden `fixtures/layer-c/ra-mini.jsonl` (stamps masked; for 11a-2's fakes); states; spans equal to layer A's, LF and CRLF; 19 refusals, the shim log empty; both watchdogs; no build script; the scrub, the emptied wrappers; monikers across an unrelated `use` edit; `error` `metadata` (a broken manifest) and `path` (non-UTF-8 requested files, a module after a sibling too); exit 1 or 0 when stdin closes right after `error` or `done`. `specengine-eval` `tests/build_graph.rs`: the 16 direct dependencies, the licence set (`stack.md`).
