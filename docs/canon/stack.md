---
class: canon
tier: 2
scope: [crates]
owner: owner
reviewed: 2026-10-19
---

# Stack: toolchain and pinned crates

The crates in use. A `=` version is an exact pin of the root `Cargo.toml` `[workspace.dependencies]`, approved by the owner; a member's extra features are in its own `Cargo.toml`. A new dependency or a version change is an owner decision. Planned crates (pinned on adoption) and the prior-art crates read for ideas: 04 s6.

## Toolchain

Edition 2024, `rust-version = 1.90`; `specengine-ra` needs rustc >= 1.98 (the `ra_ap_*` set); no `rust-toolchain.toml`.

## Pinned crates

| Crate | Version | Role / note |
|---|---|---|
| `rmcp` | =3.5.0 | official MCP SDK (`#[tool]`/`#[tool_router]`/`#[prompt]`, MRTR, `request-state`; revision 2026-07-28 and older). Features in use: `server`, `macros`, `transport-io`, `request-state`, stdio only; no `elicitation` feature (it pulls `url`): `elicitation/create` goes through `send_request`. Its Streamable HTTP Tower service would mount in axum. Open: the SDK tier |
| `tokio` / `getrandom` | =1.53.1 (`rt`, `macros`, `io-std`, `time`; `specengine-http` adds `net`, `sync`) / =0.4.3 | MCP and HTTP runtime / per-process `requestState` HMAC key; both already in the graph via `rmcp` |
| `futures-util` | =0.3.34 (no default features) | the live tail's stream; in the graph via `rmcp` |
| `axum` | =0.8.9 (no default features; `http1`, `tokio`) | `specengine-http`: HTTP, SSE (no `query`, `json`, `tower-http`) |
| `rust-embed` | =8.12.0 (no default features; `debug-embed`) | `specengine-http`: `ui/dist` compiled in (`docs/canon/serve-ui.md`). `rust-embed-impl`, `-utils` held at 8.12.0 in the lock (8.13.0 out). New: `mime_guess` 2.0.5 (MIT), compile time only, through `-impl` (a proc macro), not linked; reused: `walkdir`, `same-file`, `sha2` 0.11, `mime`, `unicase`. Recorded under the owner's approval of new dependencies; `mime_guess` and the hold await the owner's confirmation |
| `tree-sitter` / `tree-sitter-rust` | =0.27.0 / =0.24.2 | compatible (ABI 15); in 0.26+ cancellation is `ParseOptions { progress_callback }` (no `set_timeout_micros`); in 0.27 `child_count()` is `u32`, `kind()` is `&'tree str`. Only in `specengine-code`, which the CLI depends on (`spec refs`), so tree-sitter (with `tree-sitter-language`, `streaming-iterator`, `regex`; `cc` builds) is in the CLI's graph and, through the CLI library, the MCP server's and the daemon's; never the core's or the store's (`build_graph.rs`). Use: `crates/specengine-code/README.md` "Hash recipe" |
| own RON lexer | -- (`specengine-code`) | `.ron` markers, field paths, the Bevy dump reader; replaces `tree-sitter-ron` 0.2.0 (05 s9) |
| `rusqlite` | =0.40.2 (`bundled`: SQLite 3.53.2, FTS5) | the index; never `sqlx` or `refinery` beside it. Transitive, pending the owner (Q1, store README): `libsqlite3-sys` 0.38.2, `fallible-iterator` 0.3.0, `fallible-streaming-iterator` 0.1.9; build: `pkg-config` 0.3.34, `vcpkg` 0.2.15 (unused) |
| `pulldown-cmark` | =0.13.4 (no default features) | heading attributes `{#ID}`, offset iterator for precise patches |
| `serde-saphyr` | =1.3.0 (`deserialize` only) | YAML front-matter (why: 05 s9). `deserialize` pulls unpinned crates no feature turns off: granit-parser, arraydeque, annotate-snippets (+ anstyle, unicode-width), encoding_rs_io, encoding_rs (+ simdutf8, multiversion_no_op; core_detect on x86), pending the owner (Q1, core README) |
| `serde` / `serde_json` | =1.0.229 (`derive`) / =1.0.151 (`float_roundtrip`) | serialization; JSON output of tools and `specengine-eval`, the `specengine-ra` protocol lines; a float read back bit for bit |
| `toml` / `regex` | =1.1.4 (`std`, `parse`, `serde`) / =1.13.1 (`std`, `unicode`) | configs (`specengine.toml`, importer; a direct CLI dependency for `Cargo.toml` package names) / importer ID patterns; `regex` already in the graph via tree-sitter |
| `blake3` | =1.8.7 | node and AST hashes |
| `petgraph` | =0.8.3 (no default features) | `depends-cycle` (`tarjan_scc`; owner, Q-C); new to the default members: `fixedbitset` 0.5.7, `hashbrown` 0.15.5, `foldhash` 0.1.5 |
| `schemars` | 1.2.2 | JSON Schema 2020-12 for tools, via the `rmcp::schemars` re-export |
| `clap` | =4.6.7 (`derive`) | CLI |
| `ra_ap_*` (nine: `load-cargo`, `project_model`, `ide`, `ide_db`, `hir_expand`, `vfs`, `paths`, `syntax`, `proc_macro_api`) | =0.0.352 (2026-09-14), all together | `MonikerResult::from_def` for resolved identity (layer C); only in `specengine-ra`, outside `default-members`, so none enters the core graph; API without guarantees (`ra_ap_edition` is already 0.0.354: the set is skewed). Its binary's normal graph: the daemon's licence set plus `CC0-1.0` (`notify` 8.2.0), asserted in `build_graph.rs` |
| `salsa`, `salsa-macros`, `salsa-macro-rules` / `unicode-ident` | =0.28.2 / =1.0.24 | direct exact pins of `specengine-ra`: the `ra_ap` manifests ask `^`, and newer versions break the 0.0.352 build (salsa 0.28.5 changed `HashEqLike`; unicode-ident 1.0.26 is Unicode 18 against `unicode-properties` 0.1.4's 17). One lock file, so the `unicode-ident` pin governs the core graph too |
| `libc` | =0.2.189 (`default-features = false`) | `specengine-cli`, a plain dependency: the process groups and signals of `verify --tests` and `resolve` (`group.rs`); `specengine-eval` feature `ra`: peak RSS and process groups; the `specengine-ra` binary: `getrusage`, `getpgrp`, `kill` (`docs/canon/layer-c-protocol.md`) |
| `syn` / `quote` / `proc-macro2` | =3.0.6 (+ `extra-traits`) / =1.0.47 / =1.0.107 | comparison only, behind `specengine-eval`'s `syn` feature: not an identity source (no name resolution) and not the digest (05 s5.2, ADR-0021) |

## Not taken

`rusqlite_migration` (the queue's `user_version` steps; `refinery` and `sqlx` conflict), `similar` (`git diff`, `merge-file`), CRDTs (`loro` 1.16, `automerge` 0.12, `yrs` 0.28): one human writer and N proposing agents is a review-queue problem, not collaborative editing; optimistic locking by `base_hash` + SSE is enough.
