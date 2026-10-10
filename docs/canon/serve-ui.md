---
class: canon
tier: 2
scope: [crates/specengine-http, ui]
owner: owner
reviewed: 2026-10-10
---

# Daemon: the web UI it serves

`specengine-http` (`docs/canon/serve.md`) serves the web UI (`ui/README.md`) compiled into it, one URL with the API, no Node at run time; as built by `ui-embed`. The binary serves the UI of its own commit. Code: http `ui.rs` (`Dist`, `page`, `asset`, generic `page_of`, `asset_of`, `built`, `type_of`; `NOT_BUILT`, `CSP`), `build.rs`, `app.rs` (`/`, `/assets/{file}` on the fenced router). Pin: `docs/canon/stack.md`.

## Embed

The Vite build `ui/dist` (git-ignored, `pnpm --dir ui build`) is compiled in by `rust-embed` (`Dist`, `#[folder = "../../ui/dist"]`): `debug-embed`, so debug and test builds embed too and no file is read at run time; `allow_missing`, so a missing `ui/dist` compiles to an empty embed. Keys are paths under `ui/dist` with `/`. **Built**: the embed holds `index.html`.

The build script `src/build.rs`, std only, prints `cargo::rerun-if-changed=../../ui/dist` (`rust-embed` tracks only the files it embedded) and, while `ui/dist/index.html` is missing (every build then reruns it and recompiles the crate), one `cargo::warning=ui/dist holds no index.html: this specengine-http serves no web UI; build it first: pnpm --dir ui install --frozen-lockfile && pnpm --dir ui build, then build this crate again`.

## Build order

One clone, one commit: `pnpm --dir ui install --frozen-lockfile` (after a lockfile change), `pnpm --dir ui build`, `cargo install --locked --force --path crates/specengine-http` and `crates/specengine-cli`; then `spec serve`, the `listening` URL. Cargo never runs pnpm; the hook and CI build only `specengine-cli`. UI work keeps `pnpm --dir ui dev` and its proxy.

**Mtimes**: cargo reruns the script only when an mtime under `ui/dist` is newer than its last run (`cargo build`, the tests and `cargo install --path` reuse `target.noindex`): a `ui/dist` put back by `mv` or `cp -p` keeps the old embed; run `pnpm --dir ui build` or `touch ui/dist`.

## Routes

| GET | Built | Not built |
|---|---|---|
| `/` | 200, `index.html`, `text/html; charset=utf-8` | 503, the error body, the line below |
| `/assets/<file>` | 200, `assets/<file>`, its type; not embedded: the unknown-route 404 | the unknown-route 404 |

The 503 `message`: `this specengine-http holds no web UI (ui/dist had no index.html when it was compiled): run pnpm --dir ui install --frozen-lockfile && pnpm --dir ui build, then cargo install --locked --force --path <clone>/crates/specengine-http, then restart spec serve`.

A UI 200 carries `Cache-Control: no-store` (as every response, assets too) and `X-Content-Type-Options: nosniff`; `/` also exactly `Content-Security-Policy: default-src 'self'; img-src 'self' data:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'`; no `ETag`, `Last-Modified`, `max-age`, compression, ranges. Types (`type_of`) by the extension after the last `.`, as written: `html` `text/html; charset=utf-8`; `js`, `mjs` `text/javascript; charset=utf-8`; `css` `text/css; charset=utf-8`; `svg` `image/svg+xml`; `png` `image/png`; `ico` `image/x-icon`; `woff2` `font/woff2`; `json`, `map` `application/json`; `txt` `text/plain; charset=utf-8`; else `application/octet-stream` (no `mime-guess`).

## Rules

- Read routes: GET only; HEAD and any other method 405 `Allow: GET`, whatever the built state.
- No SPA fallback (the UI's routes are hash routes): another path, `/index.html`, `/assets`, `/assets/`, a dist-root file but `index.html` -> the unknown-route 404, its list ending `; the web UI: / and /assets/<file>`.
- The query is ignored (`/?scenario=normal` is `/`; the page then loads the mock); the API's parser is not applied.
- Path safety: `<file>` is the raw path after `/assets/` (`uri.path()`, never decoded); empty, holding `/`, `\` or `%`, or starting with `.`: the 404, no lookup; else one exact-key lookup of `assets/<file>` (a case-sensitive binary search).
- Inside the fence (`crates/specengine-http/README.md` "Fence"): a foreign `Host`, `Origin` or `Sec-Fetch-Site` is a 403 first. The address bar, a bookmark, a terminal link send `Sec-Fetch-Site: none`, the page's loads `same-origin`; a link from another site is refused (ADR-0034).
- No CLI call, config read, project turn or write: `/` answers alike while a root's config is broken (`/api/projects` 503).
- Literals in `src` (the build script and `cfg(test)` too) pass `door.rs` and `no_domain.rs` (README "Answers and statuses").
- The page stays CSP-clean: `ui/index.html` holds no inline script, `<style>`, `style=` or `on*=` attribute (eval `ui_policy.rs`).

## Known limits

- The warning catches a missing `ui/dist`, not a stale one: build the UI before installing.
- Size: `ui/dist` 1016 KB; the release binary 10 941 136 -> 11 969 600 B (2026-10-10), not capped.

## Tests

http `src/ui.rs` units (6, over `fixtures/ui-dist`, `fixtures/ui-dist-pageless` and a folder that never exists), `tests/ui.rs` (5: bytes or the 503, the query, methods, 31 traversal probes, a broken config; the branch by `ui/dist/index.html` on disk), `fence.rs`, `methods.rs`, `projects.rs`; eval `build_graph.rs` (the pin), `ui_policy.rs` (the page; `import_support` allowlists both fixtures). Not covered by tests: the not-built state end to end (unit level only), a `%` refusal over HTTP, the stale-embed run (manual, `docs/features/ui-embed.md` T7), a browser (O1).
