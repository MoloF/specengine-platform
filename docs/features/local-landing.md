---
class: spec
status: shipped
shipped: 2026-11-09
scope: [crates/specengine-http]
ref: RM-0080; v4 step 0 of the owner's re-scope of 2026-11-08 (08 s2 Phase 7, "v4 scope"); the defect found 2026-11-08 that a link to the local daemon from another site lands on a JSON 403
---

# local-landing: a link from another site opens the local daemon's page (RM-0080)

## Why

A person following a link to the local daemon from another site (a chat, a mail, another site's page) got a raw JSON 403, `` refused: Sec-Fetch-Site, when sent, must be `same-origin` or `none` ``, not the web UI: the browser sends `Sec-Fetch-Site: cross-site` on that navigation. The hub already admitted a top-level navigation to `/`; the local fence had no such exception. The orchestrator found this on 2026-11-08 while the owner reviewed Flow; the owner gave no input and needed to give none.

Decided:
- **D1** The hub's exception, unchanged: `GET /` with `Sec-Fetch-Mode: navigate` and `Sec-Fetch-Dest: document` passes the `Sec-Fetch-Site` step; both fences call one predicate, so they cannot drift.
- **D2** The order stands: `Host`, `Origin`, then `Sec-Fetch-Site` with the exception; a header sent twice is refused at its step.
- **D3** No ADR. ADR-0034's fence still stops other sites' pages: a navigation's answer goes to the person's tab, never to the other site's script; the page is static bytes under `frame-ancestors 'none'`; every request it makes is same-origin and takes the whole fence. The hub's identical exception came in through the canon alone.
- **D4** Nothing else: no HTML error page, no redirect; every other refusal stays the JSON 403.
- Assumptions: **A1** a top-level GET navigation carries no `Origin` (the hub's AC-03 relies on it too); **A2** a link picks a screen by its hash route, and the page writes nothing when it loads.

## Description and interactions

How it works now: `docs/canon/serve-ui.md` "Rules" (the landing), `crates/specengine-http/README.md` "Fence", `docs/canon/serve.md` "Trust", `docs/canon/hub.md` "Fence". The hub's fence, its texts and tests, the daemon's routes and answers, and the UI are unchanged.

## Data

The admitted request answers what a same-origin `GET /` gets (`serve-ui.md` "Routes"): with the UI built 200, `index.html`, `no-store`, `nosniff`, the CSP with `frame-ancestors 'none'`, no `Access-Control-*`; without, the 503 error body. The predicate, `app.rs` `landing(method, path, headers) -> Result<bool, String>`: `GET`, `uri.path()` exactly `/` (never decoded or normalised, the query ignored), exactly one `Sec-Fetch-Mode` equal to `navigate` and one `Sec-Fetch-Dest` equal to `document`. The local fence, `fenced(port, method, path, headers)`: `Host`, then `Origin`, then `Sec-Fetch-Site` or the landing. The four refusal texts are byte-unchanged; no key, route, flag, config or format changes.

## Rules and edge cases

**L1** past steps 1 and 2, the landing passes whatever `Sec-Fetch-Site` says; **L2** anything else meets today's step: another method of `/` (`HEAD`, `POST`), another path (`/index.html`, `/assets/<file>`, every `/api` route), a `Sec-Fetch-Dest` of `iframe` or `frame`, a `Sec-Fetch-Mode` of `cors` or `no-cors`, either header missing; **L3** `Sec-Fetch-Mode` or `Sec-Fetch-Dest` sent twice: `refused: the <header> header is sent more than once`; **L4** the landing passes neither `Host` nor `Origin`; **L5** the hub keeps its order and texts. Edge cases: `Sec-Fetch-Site` sent twice is refused before the predicate is asked; a same-origin or `none` navigation needs no predicate; a browser sending no `Sec-Fetch-*` passes as before.

## Acceptance criteria

The tests run on a `--port 0` daemon over git copies of `fixtures/spec-a`, through `tests/common/mod.rs`'s raw HTTP/1.1. A page's expected status is `page_status()` (`tests/fence.rs`): 200 with the UI built, 503 without. No new fixture. Every named mutation, M1-M8, was killed; M5 was checked with a trimming implementation, not a decoding one.

- [x] **LL-1 The landing.** `GET /` with `Sec-Fetch-Mode: navigate` and `Sec-Fetch-Dest: document`, sent with `Sec-Fetch-Site: cross-site`, with `same-site`, and as `GET /?scenario=normal`: each answers `page_status()`; at 200 the body is byte-equal to a same-origin `GET /`'s, with `Cache-Control: no-store`, `X-Content-Type-Options: nosniff`, the exact CSP with `frame-ancestors 'none'` and no `Access-Control-*`; at 503 the not-built error body. M1: the exception removed (403).
- [x] **LL-2 Only a top-level document.** The LL-1 request with one header changed is refused 403 with the `Sec-Fetch-Site` line, `no-store`, and no `Allow` or `Access-Control-*`: `Sec-Fetch-Dest: iframe` or `frame`; `Sec-Fetch-Mode: no-cors` or `cors`; `Sec-Fetch-Mode` or `Sec-Fetch-Dest` missing. M2: `Sec-Fetch-Mode` judged alone. M3: `Sec-Fetch-Dest` judged alone.
- [x] **LL-3 Only `GET /`.** The LL-1 headers with `cross-site` are refused 403 with the `Sec-Fetch-Site` line on `GET /assets/x.js`, `GET /index.html`, `GET /api/projects`, `GET /api/projects/<slug>/tree`, `HEAD /` and `POST /`. M4: the exception keyed on the headers alone. M5: the path compared after decoding or trimming.
- [x] **LL-4 `Host` and `Origin` first.** The LL-1 request with `Host: evil.example:<port>` gets the `Host` line; with `Origin: http://evil.example` the `Origin` line; with `Sec-Fetch-Mode` sent twice `refused: the Sec-Fetch-Mode header is sent more than once`. M6: the exception judged before step 1. M7: before step 2.
- [x] **LL-5 The hub and the rest of the fence are unchanged.** `tests/hub.rs` (its AC-03 landing test included), `tests/fence.rs`'s existing tests, `tests/serve.rs` and `tests/ui.rs` pass with their files byte-unchanged. M8: the shared predicate narrowed or widened for one fence alone.
- [x] **F1 Form.** `cargo clippy -p specengine-http --all-targets -- -D warnings` and `cargo fmt --check` clean, no dependency added, `cargo nextest run -p specengine-http` passes, `spec check` 0 errors and 0 warnings.

## Out of scope

The hub's fence, its texts and tests; `/assets/<file>`, the API routes, frames and fetches from another site, all still refused; an HTML error page or a redirect to `/`; the browser's own Private Network Access prompts; any change to the UI, the routes or a format.

## Implementation

Review: ACCEPT. The canon text of the draft's "Canon at ship" went in as written; `crates/specengine-http/README.md` ends at 10 221 B of 10 240 with `flow-scope`'s edits.

| Module | What it does |
|---|---|
| http `src/app.rs` | `fenced(port, method, path, headers)`; `landing`, moved here from `hub.rs` as `pub(crate)`, with `SEC_FETCH_SITE` |
| http `src/hub.rs` | the hub's fence calls `app::landing`; its order and texts unchanged |
| http `tests/fence.rs` | `ll1_a_top_level_navigation_from_any_site_opens_the_page`, `ll2_only_a_top_level_document_is_the_landing`, `ll3_only_get_slash_is_the_landing`, `ll4_host_and_origin_are_judged_before_the_landing` |
