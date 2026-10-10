---
class: spec
status: shipped
scope: [crates/specengine-http, crates/specengine-eval, ui]
ref: ui-embed analysis 2026-10-10 (WA 1-18, R/T/S criteria; Q1, Q2 decided under the owner's pre-approval, review open); spec-serve Q3 (`rust-embed` 8.12.0 approved); 05 s1; 07 s3; 08 s2 Phase 4; Rust 1 iteration, review accepted
shipped: 2026-10-10
adrs: []
---

# ui-embed: the web UI inside the daemon

## Why

The owner ran the UI as three things: `spec serve`, `pnpm --dir ui dev`, and the page on :5173 behind Vite's proxy. Now `spec serve` alone serves it at the `listening` URL, no Node at run time, and an installed daemon serves the UI of its own commit, so UI and API cannot drift apart. Packaging work in `specengine-http`: `rust-embed` 8.12.0 compiles the Vite build `ui/dist` in; two read routes, `/` and `/assets/<file>`, sit inside the fence. `ui/` code did not change; agents are unaffected. No ADR (ADR-0011's stated cost, ADR-0015, ADR-0033, ADR-0034, ADR-0008); the crate was approved (spec-serve Q3).

## Description and interactions

Canon: `docs/canon/serve-ui.md` ("Embed", "Build order", "Routes", "Rules", "Known limits", "Tests"); the pin `docs/canon/stack.md`; the owner's steps `ui/README.md` "Owner's manual steps" and the root `README.md` "Web UI". Working answers WA 1-18 taken. Untouched: `ui/` code and scripts, the serve lines, `daemon.json`, the CLI, the MCP server, the plugin, the gate, CI.

## Data

`docs/canon/serve-ui.md` "Embed" (the pin, `Dist`, built = the key `index.html`, the build script and its warning) and **Routes** (`serve-ui.md` "Routes": the table, the 503 line, headers, the CSP, the types, the 404 list's suffix `; the web UI: / and /assets/<file>`).

## Rules and edge cases

`docs/canon/serve-ui.md` "Rules" (WA 5-13: GET only, no SPA fallback, query ignored, path safety on the raw segment, inside the fence, no config, turn or write, the scans, the CSP-clean page) and "Build order" (the mtimes note).

## Acceptance criteria

rust-developer:

- [x] R1 -- the pin and member entry as `stack.md`; the lock holds `rust-embed`, `-impl`, `-utils` once each at 8.12.0; `cargo tree -e features -i rust-embed`: only `debug-embed`.
- [x] R2 -- the facts below, with file and line.
- [x] R3 -- `ui/dist` present: 0 warnings; absent: exactly the one warning; clippy `-D warnings` clean in both.
- [x] R4 -- std-only build script; `http_direct_dependencies_are_the_cli_library_axum_and_tokio` green.
- [x] R5 -- routes and rules built; smoke: `/` 711 B byte-equal, 14 assets, probes 404, 405s, 403s, the 404 suffix; absent: the 503 line, assets 404.
- [x] R6 -- no `tower-http`; `ui/`, `scripts/`, `.cargo/` unchanged; `serve.rs` green.
- [x] R7 -- sizes below.

test-engineer (18 mutations red):

- [x] T1 -- `build_graph.rs` `daemon_pins_are_the_approved_ones`, `http_direct_dependencies_are_the_cli_library_axum_and_tokio`, `no_workspace_dependency_is_added_against_main`, `cli_normal_graph_is_model_core_store_without_measurement_mcp_or_rust_analyzer` (`CLI_FORBIDDEN`), `every_package_of_the_daemons_normal_graph_is_licensed_within_the_allowed_set`; `mime_guess` absent from `-e normal,no-proc-macro` with the `mime-guess` feature off (deviation 1). M: 3 red.
- [x] T2 -- `src/ui.rs` units over `fixtures/ui-dist/`, `fixtures/ui-dist-pageless/` and a folder that never exists: `t2_the_fixture_is_built_and_embeds_every_file_of_its_folder_alone`, `t2_slash_is_the_page_byte_equal_with_its_type_policy_and_nosniff`, `t2_each_asset_is_byte_equal_at_its_url_with_the_tables_type`, `t2_a_dotfile_a_dot_segment_a_percent_a_slash_or_a_backslash_is_the_404`, `t2_not_built_slash_is_the_503_line_and_every_asset_the_404`, `t2_the_type_is_the_tables_by_the_last_extension_as_written`. M: 4 + 2 red.
- [x] T3 -- `tests/ui.rs`: `t3_slash_and_every_asset_are_the_dists_bytes_or_slash_is_the_not_built_line`, `t3_the_query_is_ignored_and_no_answer_caches_or_shares`, `t3_every_method_but_get_is_the_read_routes_405_allow_get`, `t3_no_traversal_encoded_or_not_reaches_a_file` (31 probes), `t3_slash_answers_alike_while_a_served_config_is_broken`. M: 6 red.
- [x] T4 -- `fence.rs` `endpoints()` gains GET `/`, `/assets/x.js` (`ac01_a_foreign_host_origin_or_fetch_site_is_refused_before_anything_is_read`, `ac01_no_response_carries_an_access_control_header`). M: red.
- [x] T5 -- `methods.rs` `an_unknown_path_is_a_404_for_every_method` (`UNKNOWN` without `/`, with `/index.html`, `/assets`; `/assets/` GET 404), `projects.rs` `ac06_an_unknown_slug_is_a_404_error_body_listing_the_served` (the suffix). M: red in `projects.rs` (`methods.rs` checks the prefix only).
- [x] T6 -- `ui_policy.rs` `ui_embed_t6_the_page_holds_no_inline_script_style_or_handler`, `ui_embed_t6_the_scan_sees_each_inline_form_and_passes_the_rest` (19 cases). M: red.
- [x] T7 -- steps 1-3 (the developer): with `ui/dist` moved aside the same binary still 200; `cargo build -p specengine-http` recompiles and prints exactly the warning, `/` 503. Step 4 as amended (the orchestrator): restored by `mv` plus `touch ui/dist`, `cargo build` recompiles with no source change and no warning, the dist byte-identical (SHA-256 before and after). Step 4 as first written (`mv` alone) does not recompile (old mtimes; deviation 3). The mutations (`debug-embed` off, the rerun line removed) were not run.
- [x] T8 -- `ui/dist` present: http 113/113 (`--test ui` 5/5), eval `build_graph`, `ui_policy` 61/61, the workspace 2262/2262; absent: http 113/113, eval 61/61.

spec-writer:

- [x] S1 -- this spec. S2 -- the `stack.md` row; 04 s6's planned row gone. S3 -- `docs/canon/serve-ui.md` (new: `serve.md` would pass its 12 KB cap), the pointer in `serve.md`, the http README (intro, "Dependencies", the 404 wording, an endpoints row, "Tests"; "Binary"'s start details left to `serve.md`). S4 -- `ui/README.md`, root `README.md`, 07 s3, 08 s1, Phase 2, Phase 4, `CLAUDE.md` State. S5 -- `export index`, `check` green.

owner:

- [ ] O1 -- build the UI, install both binaries, `spec serve` here, open `http://127.0.0.1:7777/`: every 07 s3 screen renders, a live event arrives, a stage is confirmed on a terminal; no CSP violation or 404 in the console; `?scenario=normal` shows "Mock data". An install without `ui/dist` warns; `/` shows the 503 line.

## Out of scope

`ui/` code, a second Vite mode, dropping the mock, a `package.json` script, cargo running pnpm, committing `ui/dist`, a `scripts/` helper, CI building the UI; `ETag`, `max-age`, compression, ranges, HEAD; an SPA fallback, a sub-path base, an `/index.html` alias; serve-line or `daemon.json` changes; `/mcp`, the bridge, generated TS types; other `rust-embed` features; a stale-dist warning.

## Open

1. Q1 (keep `rust-embed` =8.12.0) and Q2 (`src/build.rs` via `build =`) decided under the owner's pre-approval; review open. `mime_guess` 2.0.5 and the lock's hold of `-impl`, `-utils` at 8.12.0 (8.13.0 out) are recorded under the owner's approval of new dependencies: the owner confirms.
2. O1, the owner's manual check; T7's mutations were not run.
3. Code comments cite this spec's "Data", "Routes", "Rules and edge cases"; they hold pointers now: a later sweep may cite `docs/canon/serve-ui.md`.

## Implementation

| Module | What it does |
|---|---|
| root `Cargo.toml`, `Cargo.lock` | the pin and its approval comment; the lock +46 lines: `rust-embed`, `-impl`, `-utils` 8.12.0 (held by `cargo update --precise`), `mime_guess` 2.0.5 |
| http `Cargo.toml` | `build = "src/build.rs"`, `rust-embed.workspace = true` |
| http `src/build.rs` (new) | std only: the rerun line, the warning |
| http `src/ui.rs` (new) | `Dist`; `page`, `asset`; generic `page_of`, `asset_of`, `built`, `type_of`; `plain` (the name check before any lookup), `served`; `NOT_BUILT`, `CSP`; T2 units over `Fixture`, `Pageless`, `Missing` |
| http `src/app.rs`, `src/main.rs` | `/`, `/assets/{file}` through `read()` on the fenced router; the 404 list's suffix; `mod ui` |
| tests, fixtures | http `tests/ui.rs` (new), `fence.rs`, `methods.rs`, `projects.rs`; eval `build_graph.rs`, `ui_policy.rs`, `import_support/mod.rs` (both fixtures allowlisted); `fixtures/ui-dist/`, `fixtures/ui-dist-pageless/` (new) |

**Facts (R2)**, the fetched `rust-embed` 8.12.0: `allow_missing` (impl `lib.rs:444`, `:521`, `:487-502`; utils `lib.rs:16-22`; `tests/allow_missing.rs`); `debug-embed` embeds in debug (impl `:51-57`, `:306-308`, `:349-351`; `Cargo.toml:59-62`); lookup by exact key: `\` to `/`, a binary search, case-sensitive, no decoding (impl `:128-135`, `:26`, `:37`, `:66-70`); no default features; the trait's `get`, `iter` (`lib.rs:35-58`); no folder tracking (utils `:18`, `follow_links(true)`); MSRV 1.80; `sha2` 0.11 reused; licences MIT, Unlicense.

**Sizes (R7)**: `ui/dist` `du -sk` 1016 (15 files, 1 011 269 B); release `specengine-http` 10 941 136 -> 11 969 600 B (+1 028 464).

Deviations, accepted by the review: 1. `mime_guess` cannot leave the graph: `rust-embed-impl` 8.12.0, a proc macro, depends on it unconditionally; it compiles, is not linked, and T1 asserts it absent from the linked graph. 2. New crates: only `mime_guess`; reused `walkdir`, `same-file`, `sha2` 0.11 (no 0.10), `mime`, `unicase`, `syn`. 3. T7 step 4 needs fresh mtimes (canon "Build order"). 4. `ui.rs` sets `no-store` again beside the fence's (a review nit, harmless).
