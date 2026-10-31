---
class: spec
status: shipped
shipped: 2026-11-01
scope: [crates/specengine-http, crates/specengine-cli, ui]
ref: RM-0049; Phase 7 step 8 (08 s2); owner's decision of 2026-11-01
adrs: [ADR-0058, ADR-0059]
---

# hub-identity: a public origin and one trusted login, nothing that confirms (RM-0049)

## Why

A person was a typed name nothing verifies, on a loopback-only page (ADR-0057). The owner cancelled the single-user principle so named people work one board on a public URL, with no account inside SpecEngine. This step lays that footing and gives nobody power over the brief.

## Summary

Canon: `docs/canon/hub.md`.

### Data

No bump to the queue schema, `daemon.json`, `spec.lock`, `export state`, `specengine.toml`, `Cargo.lock`.

### Route

`GET /api/identity`: `Identity` `["login"]`, the 107th and last set of `fixtures/daemon-keys.json`.

## Acceptance criteria

- [x] AC-01 — Start: binds `127.0.0.1`, prints `public` before `listening`, five `daemon.json` keys; a bad origin, `--host`, `--bind`: exit 2, nothing created.
- [x] AC-02 — Origin: every route refuses a foreign or loopback `Host`, a foreign or `null` `Origin`, `same-site`, `cross-site`; no `Access-Control-*`.
- [x] AC-03 — Landing: `GET /` as a `navigate` `document` passes any `Sec-Fetch-Site`; an API route, asset, iframe, POST: 403.
- [x] AC-04 — No identity: 403 on every route, no card, no event.
- [x] AC-05 — Two identities: `mira` creates, `tomas` moves; `created_by`, `history`, the tail name each.
- [x] AC-06 — `as`: null or the login stores it on all five writes; another name 403, no change.
- [x] AC-07 — `by = ["mira"]`: `mira` moves, a 409 names `tomas`.
- [x] AC-08 — Local-only: decision, round answers, accept, clear, trace clear, stage DELETE, HEAD, unknown POST: 403, no change.
- [x] AC-09 — `hub_trusts_the_header_on_its_socket` pins the forged 200; two values, `a, b`, tab, off UTF-8, 129 bytes, `local`, `agent`: 403.
- [x] AC-10 — Local unchanged: `fence.rs`, `door.rs` and kin undiffed; a local daemon ignores `Remote-User`, answers `{"login":null}`.
- [x] AC-11 — Five writes leave git alone, one `card.*` event each; `door.rs`, `no_domain.rs` read `hub.rs` unedited.
- [x] AC-12 — UI: a resolved login read-only and sent, the key untouched; else the stored name; old board tests unedited.
- [x] AC-13 — ADR-0017 `superseded-by ADR-0058`; check 0 errors, 0 warnings; each ADR at most 1 536 B; no MCP or `Cargo.lock` diff.

## Implementation

Stricter than planned: `hub.md` "Entry", "Identity". Open: "Known limits".

### Files in scope

| Module | What it does |
|---|---|
| http `src/hub.rs` (new) | `Hub`, `Login`, `acting`; keeps the UTF-8 and empty steps |
| http `src/app.rs`, `src/main.rs` | `App.hub`, `identity`, `acting` in five handlers; the flag |
| `crates/specengine-cli/src/card.rs` | `specengine_cli::hub_login_problem`; no re-export of `card_person_problem` |
| `ui/src/api/`, `ui/src/board/` | `useIdentity`; `useHubLogin` in `acting.ts`, `useActing`, `readOnly` |
| Tests | `tests/hub.rs`, `hubIdentity.test.tsx` |
