---
class: spec
status: draft
scope: [crates/specengine-http]
ref: RM-0080; v4 step 0 of the owner's re-scope of 2026-11-08 (08 s2 Phase 7, "v4 scope"); the defect found 2026-11-08 that a link to the local daemon from another site lands on a JSON 403
---

# local-landing: a link from another site opens the local daemon's page (RM-0080)

## Why

### Requirement

A person who follows a link to the local daemon from another site (a chat, a mail, another site's page) gets a raw JSON 403, `` refused: Sec-Fetch-Site, when sent, must be `same-origin` or `none` ``, not the web UI. The browser sends `Sec-Fetch-Site: cross-site` on that navigation, and the local fence refuses every value but `same-origin` and `none` (`crates/specengine-http/README.md` "Fence"). The hub had the same problem and admits a top-level navigation to `/` ("The landing" in `docs/canon/hub.md` "Fence"); the local daemon has no such exception. The orchestrator found this on 2026-11-08 while the owner reviewed Flow. The owner gave no input on it and needs to give none: this step gives the local fence the hub's exception, unchanged.

### Decisions

- **D1 The hub's exception, unchanged.** `GET /` with `Sec-Fetch-Mode: navigate` and `Sec-Fetch-Dest: document` passes the `Sec-Fetch-Site` step whatever that header says. Both fences call one predicate, so they cannot drift.
- **D2 The order stands.** First `Host`, then `Origin`, then `Sec-Fetch-Site` with the exception. A header sent twice is refused at its step, as today.
- **D3 No ADR.** ADR-0034 keeps the fence to "stop other sites' pages", and that still holds:
  - a top-level navigation gives the other site nothing, because the answer goes to the person's tab and never to that site's script;
  - the page is static bytes, and `frame-ancestors 'none'` keeps it out of every frame (`docs/canon/serve-ui.md` "Routes");
  - every request the page then makes is same-origin and goes through the whole fence.

  The hub's identical exception came in through the canon alone, with `hub-identity`; no decision record names it. So this is a canon change too. The sentence that changes is the one in `serve-ui.md` "Rules" that cites ADR-0034 for refusing such a link.
- **D4 Nothing else.** There is no HTML error page and no redirect: every other refusal stays the JSON 403.

### Assumptions

- **A1** A top-level GET navigation carries no `Origin`; the hub's AC-03 relies on the same fact. A browser that sends a foreign `Origin` anyway is refused at step 2, as before.
- **A2** A link chooses which screen opens, through its hash route (`#/<p>/...`), and nothing more. The page makes no write when it loads; a card or a stage is written only after a person clicks.

## Description and interactions

In scope:
- the local fence in `app.rs`: `fenced(port, headers)` also takes the method and the path;
- the hub's `landing` predicate in `hub.rs`, moved where both fences call it.

What a person sees: a link from anywhere to `http://127.0.0.1:<port>/` (or `localhost`), hash route included, opens the UI on that route. An asset, an API route or a frame asked from another site is refused, as before.

Neither the hub's fence nor its tests change (`docs/canon/hub.md` "Fence", `crates/specengine-http/tests/hub.rs`). Neither do the daemon's routes, its answers, or the UI.

## Data

The request this step admits, from any site:

```http
GET / HTTP/1.1
Host: 127.0.0.1:7777
Sec-Fetch-Site: cross-site
Sec-Fetch-Mode: navigate
Sec-Fetch-Dest: document
```

Its answer is the one a same-origin `GET /` gets (`serve-ui.md` "Routes"):
- with the UI built: 200, `index.html`, `Cache-Control: no-store`, `X-Content-Type-Options: nosniff`, exactly `Content-Security-Policy: default-src 'self'; img-src 'self' data:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'`, and no `Access-Control-*` header;
- with no UI built: the 503 error body.

The query is ignored, as on every request to `/` (`/?scenario=normal` is `/`).

The predicate is today's `hub.rs` `landing(method: &Method, path: &str, headers: &HeaderMap) -> Result<bool, String>`. It requires:
- `method == GET`;
- `path == "/"` (the path from `uri.path()`, never decoded or normalised);
- exactly one `Sec-Fetch-Mode`, equal to `navigate`;
- exactly one `Sec-Fetch-Dest`, equal to `document`.

The local fence becomes `fenced(port, method, path, headers)`, in this order:
1. `Host` must be exactly `127.0.0.1:<port>` or `localhost:<port>`;
2. `Origin` is absent, or `http://` followed by one of them;
3. `Sec-Fetch-Site` is absent, `same-origin` or `none`, or the request is the landing.

The refusal texts stay byte for byte:

```text
refused: the Host header must be 127.0.0.1:<port> or localhost:<port>: this server serves its own origin only
refused: the Origin header, when sent, must be http://127.0.0.1:<port> or http://localhost:<port>
refused: Sec-Fetch-Site, when sent, must be `same-origin` or `none`
refused: the <header> header is sent more than once
```

No key, route, flag, config or format changes.

## Rules and edge cases

- **L1** WHEN a request passes steps 1 and 2 and is the landing, the local fence SHALL pass it whatever `Sec-Fetch-Site` says, `cross-site` and `same-site` included.
- **L2** WHEN the request is anything else, the `Sec-Fetch-Site` step SHALL be today's. That covers another method on `/` (`HEAD` and `POST` included), another path (`/index.html`, `/assets/<file>`, every `/api` route), a `Sec-Fetch-Dest` of `iframe` or `frame`, a `Sec-Fetch-Mode` of `cors` or `no-cors`, and either header missing.
- **L3** WHEN `Sec-Fetch-Mode` or `Sec-Fetch-Dest` is sent twice, the predicate's `single` SHALL refuse with `refused: the <header> header is sent more than once`, as the hub does.
- **L4** The landing SHALL NOT pass `Host` or `Origin`: both are judged before it.
- **L5** The hub's fence SHALL keep its order and texts; one predicate serves both fences.

Edge cases:
- **`Sec-Fetch-Site` sent twice with the landing's headers:** refused at step 3 for the duplicate, before the predicate is asked, as on the hub.
- **A same-origin or `none` navigation** passes as it always did, and the predicate is not needed.
- **A browser that sends no `Sec-Fetch-*`** passes as before: an absent header is no refusal.

## Acceptance criteria

The tests run on a `--port 0` daemon over git copies of `fixtures/spec-a`, through `tests/common/mod.rs`'s raw HTTP/1.1. A page's expected status is `page_status()` (`tests/fence.rs`): 200 with the UI built, 503 without. No new fixture.

- [ ] **LL-1 The landing.** `GET /` with `Sec-Fetch-Mode: navigate` and `Sec-Fetch-Dest: document`, sent once each with `Sec-Fetch-Site: cross-site` and with `same-site`, and also as `GET /?scenario=normal`:
  - each answers `page_status()`;
  - at 200, the body is byte-equal to a same-origin `GET /`'s, with `Cache-Control: no-store`, `X-Content-Type-Options: nosniff`, the exact CSP above with `frame-ancestors 'none'`, and no `Access-Control-*` header;
  - at 503, the body is the not-built error body.
  - M1: the exception removed (403).
- [ ] **LL-2 Only a top-level document.** The LL-1 request with one header changed is refused 403 with the `Sec-Fetch-Site` line, `no-store`, and no `Allow` or `Access-Control-*`. The changes:
  - `Sec-Fetch-Dest: iframe`, or `frame`;
  - `Sec-Fetch-Mode: no-cors`, or `cors`;
  - `Sec-Fetch-Mode` missing, or `Sec-Fetch-Dest` missing.
  - M2: `Sec-Fetch-Mode` judged alone (`iframe` passes). M3: `Sec-Fetch-Dest` judged alone (`no-cors` passes).
- [ ] **LL-3 Only `GET /`.** The LL-1 headers with `Sec-Fetch-Site: cross-site` are refused 403 with the `Sec-Fetch-Site` line on:
  - `GET /assets/x.js`;
  - `GET /index.html`;
  - `GET /api/projects`;
  - `GET /api/projects/<slug>/tree`;
  - `HEAD /`;
  - `POST /`.
  - M4: the exception keyed on the headers and not on `GET /`. M5: the path compared after decoding or trimming.
- [ ] **LL-4 `Host` and `Origin` first.** The LL-1 request:
  - with `Host: evil.example:<port>`, gets the `Host` line;
  - with `Origin: http://evil.example`, gets the `Origin` line;
  - with `Sec-Fetch-Mode` sent twice, gets `refused: the Sec-Fetch-Mode header is sent more than once`.
  - M6: the exception judged before step 1. M7: the exception judged before step 2.
- [ ] **LL-5 The hub and the rest of the fence are unchanged.** These pass with their files byte-unchanged:
  - `crates/specengine-http/tests/hub.rs`, its AC-03 landing test included;
  - `tests/fence.rs`'s existing tests, whose foreign requests send no `Sec-Fetch-Mode` and stay refused;
  - `tests/serve.rs` and `tests/ui.rs`.
  - M8: the shared predicate narrowed or widened for one fence alone (the hub's AC-03 or LL-1 to LL-3 go red).
- [ ] **F1 Form.** `cargo clippy -p specengine-http --all-targets -- -D warnings` and `cargo fmt --check` are clean, no dependency is added, `cargo nextest run -p specengine-http` passes, and `spec check` reports 0 errors and 0 warnings.

### Fixtures and tests to add

- Fixtures: `fixtures/spec-a` through `tests/common/mod.rs`; none is new.
- `crates/specengine-http/tests/fence.rs` (test-engineer) gains four tests:
  - `ll1_a_top_level_navigation_from_any_site_opens_the_page`;
  - `ll2_only_a_top_level_document_is_the_landing`;
  - `ll3_only_get_slash_is_the_landing`;
  - `ll4_host_and_origin_are_judged_before_the_landing`.

  Their module comment cites `docs/features/local-landing.md` "Acceptance criteria". LL-5 is the existing files, unchanged.
- `src/app.rs` units, if the developer keeps any, judge `fenced` directly, as `hub.rs`'s `the_landing_is_a_top_level_navigation_to_the_page_alone` does.

## Out of scope

- The hub's fence, its texts and its tests.
- `/assets/<file>`, the API routes, frames and fetches from another site: all still refused.
- An HTML error page, or a redirect from a refused route to `/`.
- The browser's own rules for public-to-loopback requests (Private Network Access): if a browser starts asking the person, the prompt is the browser's.
- Any change to the UI, the routes or a format.

### Risks

- **R1 Widening.** Admitting any request but `GET /` (an API GET, an asset) would let another site start reads in the person's tab. A navigation cannot read them, but LL-3 keeps the door exactly `/`.
- **R2 Drift between the fences.** One predicate, plus LL-5's unchanged hub tests.
- **R3 The cap.** `crates/specengine-http/README.md` has 6 bytes free under its 10 240 B cap. The Fence text below is 4 bytes longer than today's.

## Canon at ship

The exact text, applied by the spec-writer when the slice ships:
- **`crates/specengine-http/README.md` "Fence"**: the first paragraph becomes this text (461 B against today's 457 B; it fits):

  > Around the whole router (`app.rs` `fence`), failing -> 403 without `Allow`, nothing read: `Host` exactly `127.0.0.1:<port>` or `localhost:<port>`; `Origin` absent or `http://` + one of them; `Sec-Fetch-Site` absent, `same-origin` or `none`, but for the landing (`serve-ui.md` "Rules"); any sent twice -> 403. No authentication (ADR-0034). No `Access-Control-*` header, `OPTIONS` included; `Cache-Control: no-store` always. A hub's, `--public-origin`: `hub.md`.

- **`docs/canon/serve-ui.md` "Rules"**: the bullet "Inside the fence" becomes this text:

  > - Inside the fence (`crates/specengine-http/README.md` "Fence"): a foreign `Host` or `Origin` is a 403 first, then `Sec-Fetch-Site`. The address bar, a bookmark, a terminal link send `Sec-Fetch-Site: none`, the page's loads `same-origin`. **The landing**, a local daemon's as a hub's (`hub.md` "Fence"): `GET /` with `Sec-Fetch-Mode: navigate` and `Sec-Fetch-Dest: document` passes that step whatever it says, so a link from a chat or another site opens the page, not a JSON 403. Static bytes, `frame-ancestors 'none'`, and every request the page makes is same-origin and takes the whole fence; `/assets/<file>`, every `/api` route, a frame, a fetch and any other method of `/` still refuse it.

- **`docs/canon/serve.md#trust`**: this sentence follows "they identify no one." (+151 B of the 171 B free):

  > A link from another site still opens `/`: static bytes in the person's tab, never framed, whose requests take the whole fence (`serve-ui.md` "Rules").

- **`docs/canon/hub.md` "Fence"**: "One exception, for the hub alone:" becomes "One exception:" (−19 B; the file has 1 B free).
- `reviewed:` moves to the ship date on each canon file touched; then `export index` and `check`.

## Implementation

Filled in after implementation.
