---
class: spec
status: draft
scope: [crates/specengine-http, crates/specengine-eval, ui]
ref: ui-embed analysis 2026-10-10 (WA 1-18, R/T/S criteria; Q1, Q2 decided under the owner's pre-approval, review open); spec-serve Q3 (`rust-embed` 8.12.0 approved); 05 s1; 07 s3; 08 s2 Phase 4
adrs: []
---

# ui-embed: the web UI inside the daemon

## Why

The owner runs the UI as three things: `spec serve`, `pnpm --dir ui dev`, and the page on :5173 whose Vite proxy sends `/api` to :7777 (`ui/README.md` "Owner's manual steps"). The goal: `spec serve` alone, one URL (the `listening` line, `docs/canon/serve.md` "Lines"), no Node at run time; and an installed daemon serves the UI of its own commit, so UI and API cannot drift apart (`serve.md` "Known limits", version skew). 05 s1 already draws the daemon with a "static Web UI"; 07 s3 reserves `/`.

Packaging work in `specengine-http`, not UI work: at compile time the crate embeds the Vite build `ui/dist` (git-ignored, written by `pnpm --dir ui build`) with `rust-embed` 8.12.0 and answers two new read routes, `/` and `/assets/<file>`, inside the existing fence. `ui/` code does not change (the HTTP client calls relative `/api` URLs, routes are hash routes, the icon is a `data:` URL); agents are unaffected. No ADR (ADR-0011's stated cost, ADR-0015, ADR-0034, ADR-0033, ADR-0008): the crate is approved (spec-serve Q3) and enters `docs/canon/stack.md` on adoption.

## Description and interactions

**Build order** (owner, WA 14), one clone, one commit: 1. `pnpm --dir ui install --frozen-lockfile` (again after a lockfile change); 2. `pnpm --dir ui build`; 3. `cargo install --locked --force --path crates/specengine-http` and `crates/specengine-cli` (`spec`) from the same commit; then `spec serve` and open the `listening` URL. Cargo never runs pnpm; no `scripts/` helper, no `.cargo/config.toml` change; the pre-commit hook and CI build only `specengine-cli`.

**Development** unchanged (WA 16): `pnpm --dir ui dev` and its proxy for UI work. **The mock stays** (WA 15): a lazy chunk, loaded only under `?scenario=`, flagged "Mock data"; `/?scenario=normal` on the daemon shows it.

**The owner sees** the UI at `http://127.0.0.1:7777/`; a daemon compiled without `ui/dist` warned at install and answers `/` with the 503 line ("Data"). An agent sees nothing new (MCP stays stdio).

**Touched**: `specengine-http` (`Cargo.toml`, `src`, the workspace's first build script), the root `Cargo.toml`, `Cargo.lock`, `specengine-eval` tests, a new `fixtures/ui-dist/`; nothing else ("Out of scope").

## Data

**Pin** (root `[workspace.dependencies]`, an approval comment in the style of the `axum` one citing spec-serve Q3 and this spec; WA 1, WA 3):

```toml
rust-embed = { version = "=8.12.0", default-features = false, features = ["debug-embed"] }
```

Member: `rust-embed.workspace = true`, plain; `build = "src/build.rs"` under `[package]` (Q2); no `[build-dependencies]`, no `[features]`. The lock resolves `rust-embed`, `rust-embed-impl`, `rust-embed-utils` once each at 8.12.0; other new crates are measured and recorded (expected: `walkdir` 2.5.0, `same-file` 1.0.6, already locked; maybe `sha2` 0.10 beside the locked 0.11.0). Off: `mime-guess`, `compression`, `include-exclude`, `interpolate-folder-path`, `deterministic-timestamps`.

**The embed** (WA 1-3):

```rust
#[derive(rust_embed::RustEmbed)]
#[folder = "../../ui/dist"] // the crate's; publish = false: `cargo install --path` reads the clone's
#[allow_missing = true]     // a missing ui/dist compiles to an empty embed
struct Dist;
```

`debug-embed` embeds in debug and test builds too: one code path, files only as compile-time keys, no file read at run time. Keys are paths under `ui/dist` with `/`: `index.html`, `assets/<file>`. **Built** = the embed holds the key `index.html` (an empty `ui/dist` or a failed Vite run is not built). Serving is generic over `RustEmbed`, so unit tests run it over a fixture and over a folder that never exists. Dist today: `index.html` and 14 files under `assets/` (13 `.js`, one `.css`), `du -sk` 1016; base `/`, no `public/`.

**Assumptions** (R2; the developer reads the fetched 8.12.0 source, records file:line): `allow_missing` exists (if not: stop, Q1's alternative); `debug-embed` embeds in debug; embed-mode `get` matches exact keys; no default features; the `RustEmbed` trait (`get`, `iter`); no folder tracking beyond embedded files; MSRV <= 1.90; `sha2`'s major in `rust-embed-utils`; licences in the allowed set.

**Build script** `crates/specengine-http/src/build.rs` (WA 4), std only, prints:

- always `cargo::rerun-if-changed=../../ui/dist` (cargo scans the directory): `rust-embed` tracks only the files it embedded, and builds, tests and `cargo install --path` reuse `target.noindex`, so without it an added or removed file keeps a stale UI; while `ui/dist` is missing every build reruns it and recompiles the crate (accepted);
- exactly one `cargo::warning=` when `<CARGO_MANIFEST_DIR>/../../ui/dist/index.html` is not a file, this text exactly: `ui/dist holds no index.html: this specengine-http serves no web UI; build it first: pnpm --dir ui install --frozen-lockfile && pnpm --dir ui build, then build this crate again`.

Nothing else: no file written, no process run.

**Routes** (WA 5-13), on the fenced router beside the API:

| GET | Built | Not built |
|---|---|---|
| `/` | 200, the `index.html` bytes, `text/html; charset=utf-8` | 503, the error body, the line below |
| `/assets/<file>` | 200, the bytes of `assets/<file>`, the table's type; a name not embedded: the unknown-route 404 | the unknown-route 404 |

The not-built `message` exactly (WA 8): `this specengine-http holds no web UI (ui/dist had no index.html when it was compiled): run pnpm --dir ui install --frozen-lockfile && pnpm --dir ui build, then cargo install --locked --force --path <clone>/crates/specengine-http, then restart spec serve`.

Headers of a UI 200 (WA 9, WA 11): `Cache-Control: no-store` (the fence's, every response, assets too), `X-Content-Type-Options: nosniff`; `/` also exactly `Content-Security-Policy: default-src 'self'; img-src 'self' data:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'`. No `ETag`, `Last-Modified`, `max-age`, `Content-Encoding`, `Accept-Ranges`, `Access-Control-*`.

Types (WA 10), by the extension after the last `.`, as written; no `mime-guess`:

| Extension | `Content-Type` |
|---|---|
| `html` | `text/html; charset=utf-8` |
| `js`, `mjs` | `text/javascript; charset=utf-8` |
| `css` | `text/css; charset=utf-8` |
| `svg`, `png`, `ico` | `image/svg+xml`, `image/png`, `image/x-icon` |
| `woff2` | `font/woff2` |
| `json`, `map` | `application/json` |
| `txt` | `text/plain; charset=utf-8` |
| other, none | `application/octet-stream` |

**The 404 list** (WA 6): the UI routes appended after the API's, so the prefix the tests assert (`no route <path>: the routes are /api/projects `) holds: `no route <path>: the routes are /api/projects and /api/projects/<slug>/{tree, nodes/<REF>, search, bundle, graph, check, health, inbox, proposals/<id>, tasks, tasks/<id>, round, events}; the web UI: / and /assets/<file>`.

## Rules and edge cases

- Methods (WA 5): both routes through `read()`: GET only; HEAD and every other method 405 `Allow: GET` (`crates/specengine-http/README.md` "Fence"), whatever the built state.
- No SPA fallback (WA 6): hash routes need none; another non-API path, `/index.html`, `/assets`, `/assets/` and a dist-root file but `index.html` (a future `public/`) are the unknown-route 404.
- The query is ignored on both (WA 7): `/?scenario=normal` is `/`'s bytes; the API's strict query parser is not applied.
- Path safety (WA 12): `<file>` is the raw last path segment, never percent-decoded (not axum's decoded `Path`): empty, holding `/`, `\` or `%`, or starting with `.` -> the 404, no lookup; else one lookup of `assets/<file>` by exact key. More segments (`/assets/a/b.js`, `/assets/../x`, `/assets//etc/passwd`) match no route: 404. Vite's hashed names hold none of these.
- Inside the fence (WA 13): a foreign `Host`, `Origin` or `Sec-Fetch-Site` is a 403 before the routes. Address bar, bookmark and terminal links send `Sec-Fetch-Site: none`, the page's loads `same-origin`; a link from another site is refused (intended, ADR-0034).
- No config, no turn, no write (WA 13): a UI route calls no CLI function, reads no config, takes no project turn, writes nothing; `/` answers alike while a root's `specengine.toml` is broken and `/api/projects` answers 503.
- Source scans (`src`, the build script and `cfg(test)` included; README "Answers and statuses"): `door.rs` refuses a bare identifier `index`, `init`, `reject`, `approve`, `propose*`, `export_*`, `task_*` but `task_list`, `task_show`, and `Staged`, `Changed`, `baseline`; `no_domain.rs` a literal holding as a word a verdict, a task state, a `LINK_TYPES` name or a configured kind (`command`, `note`, `rule`, `domain`, `question`, `ready`, `done`, `draft`, `review`, `canon`, ...). The pinned lines, the CSP and the types pass; a new literal must.
- CSP-clean UI (WA 11): `ui/index.html` holds no `<script>` without `src`, no `<style>`, no `style=` or `on*=` attribute (T6). Evidence: no inline script or style, `dangerouslySetInnerHTML`, worker or `blob:` URL; the icon is the one `data:` URL.
- Size (WA 17): recorded (R7), not capped; estimate ~+1 MB.

## Acceptance criteria

rust-developer:

- [ ] R1 -- the pin and member entry as "Data"; `Cargo.lock` holds `rust-embed`, `-impl`, `-utils` once each at 8.12.0; `cargo tree -p specengine-http -e features -i rust-embed` shows only `debug-embed`.
- [ ] R2 -- "Implementation" records each assumption with file and line in the fetched source.
- [ ] R3 -- `ui/dist` absent: `cargo build -p specengine-http` succeeds with exactly one warning, the "Data" text; present: none; `cargo clippy -p specengine-http --all-targets -- -D warnings` clean in both states.
- [ ] R4 -- the build script uses std only; `http_direct_dependencies_are_the_cli_library_axum_and_tokio`'s no-build-dependency, no-feature assertion green.
- [ ] R5 -- "Routes" and "Rules and edge cases" built; the 404 list ends `; the web UI: / and /assets/<file>`.
- [ ] R6 -- nothing else: no `tower-http`; `ui/`, `scripts/`, `.cargo/` unchanged; the serve lines and `daemon.json` byte-identical (`serve.rs` green).
- [ ] R7 -- "Implementation" records `du -sk ui/dist` and the release `specengine-http` size before and after.

test-engineer:

- [ ] T1 -- `build_graph.rs` (WA 18): `HTTP_DEPENDENCIES` gains `rust-embed`, its member entry plain (only `tokio` adds features); `daemon_pins_are_the_approved_ones` drops it from the absent list and asserts its pin (`=8.12.0`, default features off, features exactly `debug-embed`, resolves once; `-impl`, `-utils` at 8.12.0); `tower-http`, `notify`, `h2`, `mime_guess`, `include-flate`, `shellexpand`, `globset` absent from the daemon's normal graph; `no_workspace_dependency_is_added_against_main` allows it; `CLI_FORBIDDEN` gains it; the licence list gains it. M: `mime-guess` on; `rust-embed` declared in `specengine-cli`; a caret `8.12.0`.
- [ ] T2 -- `cfg(test)` units over a committed `fixtures/ui-dist/` (`index.html`, `assets/` `.js`, `.css`, `.svg`, an unknown extension, a dotfile) and an embed over a folder that never exists: built, `/` 200 byte-equal, each type, the exact CSP, `nosniff`; a dotfile, `..`, `%`, `\` -> 404; missing, `/` 503 with the exact line. M: `.js` -> `application/octet-stream`; the built test inverted; the CSP dropped; `nosniff` dropped.
- [ ] T3 -- new `crates/specengine-http/tests/ui.rs` (spawned daemon, `common`), the branch chosen by `ui/dist/index.html` on disk: present, `/` its exact bytes and every `ui/dist/assets/` file byte-equal at its URL with its type; absent, `/` 503 with the exact line, `/assets/x.js` 404. Both: `no-store`, no `Access-Control-*`; `/?scenario=normal` equals `/`; HEAD, POST, PUT, DELETE, PATCH, OPTIONS, TRACE on `/` and `/assets/x.js` 405 `Allow: GET`; `/assets/../../../Cargo.toml`, `/assets/..%2f..%2f..%2fCargo.toml`, `%2e%2e` forms, `/assets//etc/passwd`, `/..%2fCargo.toml`, `/index.html`, `/assets` -> 404 (or 400), never 200 or `Cargo.toml`'s bytes; nothing written beyond `daemon.lock`, `daemon.json` (`common::beyond_daemon`); `/` unchanged after a served root's `specengine.toml` is broken while `/api/projects` turns 503. M: an SPA fallback; `get(page)` for `read(page)`; a disk read of `ui/dist/assets/<decoded name>`; `max-age` on assets in the fence; the page handler reading configs; strict query parsing on `/`.
- [ ] T4 -- `fence.rs` `endpoints()` gains GET `/` and `/assets/x.js`. M: the UI routes outside the fence's layer.
- [ ] T5 -- `methods.rs` `UNKNOWN` drops `/`, gains `/index.html`, `/assets`; `/assets/` asserted 404 on GET only; `projects.rs` drops `/` and asserts the list names `/` and `/assets/<file>`. M: the 404 list without the UI routes.
- [ ] T6 -- `ui_policy.rs`: `ui/index.html` has no `<script>` without `src`, no `<style>`, no `style=` or `on*=` attribute. M: an inline `<script>`.
- [ ] T7 -- manual, recorded: dist built, `/` 200; `ui/dist` moved to scratch: the same binary still 200; `cargo build -p specengine-http` recompiles and warns, `/` 503; dist restored: `cargo build` recompiles with no source change, `/` 200. Copy kept, no second target directory. M: `debug-embed` off (step 2 fails); the rerun line removed (step 4 does not recompile).
- [ ] T8 -- `cargo nextest run -p specengine-http` and `-p specengine-eval --test build_graph --test ui_policy` pass with `ui/dist` present and absent; then one full workspace run.

spec-writer, at shipping:

- [x] S1 -- this spec.
- [ ] S2 -- `docs/canon/stack.md` gains the `rust-embed` row (pin, `debug-embed`, role, transitive crates measured); 04 s6 drops its planned row.
- [ ] S3 -- "Data" and "Rules" move to canon: a "Web UI" section of `docs/canon/serve.md`; the http README (10 179 of 10 240 B) a pointer and its "Dependencies", "Fence", "Tests" lines.
- [ ] S4 -- `ui/README.md` "Owner's manual steps" (`spec serve` alone; `pnpm dev` for UI work), the root README "Web UI" (build the UI first), 07 s3 `/`, 08 s1 and Phase 4, `CLAUDE.md` state.
- [ ] S5 -- `export index`, `check` green, READMEs within caps.

owner:

- [ ] O1 -- build the UI, install both binaries, `spec serve` in this repository, open `http://127.0.0.1:7777/`: every 07 s3 screen renders, a live event arrives, a stage succeeds and is confirmed on a terminal; the console shows no CSP violation, no 404; `?scenario=normal` shows "Mock data". An install without `ui/dist` prints the warning; `/` shows the 503 line.

## Risks

- Red by design: tests treating `/` as unknown (`methods.rs`, `projects.rs`) change with T5; appending keeps the asserted prefix.
- Unverified `rust-embed` facts (R2); `allow_missing` missing stops the slice (Q1).
- The CSP could block a screen: O1 settles it.
- The warning catches a missing `ui/dist`, not a stale one: the owner builds it before installing.

## Out of scope

Any `ui/` code; a second Vite mode; dropping the mock; a new `package.json` script; cargo running pnpm; committing `ui/dist`; a `scripts/` helper; CI building the UI or the daemon; `ETag`, `max-age`, compression, ranges, HEAD; an SPA fallback, a sub-path base, an `/index.html` alias; changes to the serve lines, `daemon.json`, the no-binary install line; opening a browser; `/mcp`, the bridge, generated TS types; the features "Data" leaves off; a stale-dist warning.

## Open

1. Q1 (keep `rust-embed` =8.12.0; a generated `include_bytes!` table only if `allow_missing` is missing or the measured graph is large) and Q2 (the build script at `src/build.rs` via `build =`) decided under the owner's pre-approval; the owner's review is open.
2. O1, the owner's manual check.

## Implementation

Not started. To record: R2's file:line facts, the transitive crates (R1), R7's sizes, T7's run, and any deviation from this spec with its reason.
