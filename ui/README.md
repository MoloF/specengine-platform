---
class: canon
tier: 1
scope: [ui]
owner: owner
reviewed: 2026-11-13
---

# ui -- the web UI

The owner's screens: 07 s3, flow 06 s3.3-3.4. Rules: `docs/canon/architecture.md#ui`; nothing blocked (`#control`).

## Stack

A standalone pnpm project on strict TypeScript, no root `package.json`. Tests: Vitest, jsdom, Testing Library: `src/**/*.test.ts(x)`, `fetch` and `EventSource` stubbed.

## Contract seam

- One interface, `src/api/client.ts` `SpecEngineClient`, methods named after the daemon's endpoints; reads take the query's `AbortSignal` (superseded: aborted; never the check or health read), writes none. The bootstrap `src/main.tsx` alone picks one: `src/api/http.ts` `HttpClient` by default (`vite.config.ts` proxies `/api` to :7777), the mock `src/mocks/` for any `?scenario=`, lazily.
- App code imports domain types only from `src/api/types.ts`, re-exporting `src/api/provisional.ts` now, the generated types (`src/api/generated/`) later. Provisional types copy documented shapes, each citing its source as `` `<path>` "Heading" `` (replaced, never extended: `#ui`).
- An endpoint 07 s3 lacks is named in the slice spec's "Open", never invented; the mock may serve it, flagged in the interface; `HttpClient` rejects it unsent, `ClientError {status: 501, notServed: true}`: "Not built yet", no Retry.
- The UI renders what the daemon returns: diffs as hunks, decisions by `apply_proposal`, refusals in its own words. An owner action with no endpoint is its `spec` command to copy (fixed words, a validated ID); a decision is at most staged, confirmed on a terminal (`decision-staging.md`) or, on a hub, by one click per item (`hub-confirm.md`); a card is written (`board-ui.md`).

## Screen rules

Every slice keeps these (UI tests, `ui_policy.rs`).

- **Tokens**: `src/styles/tokens.css` holds every colour literal, `light-dark()` roles, the size scales, motion (0 if reduced); no raw size elsewhere; views build from `src/ui/` (`ui-design.md`). WCAG 2.2 AA in both themes: text >= 4.5:1; focus ring, control border, statuses >= 3:1; `cannot-verify` its own colour and icon.
- **Status** never by colour alone: label and icon. An unknown value: a neutral badge, the raw text, sorted last; `kind`, `contour`, `role`, `profile`, spec statuses, link types (but `mentions`), workspace and module names stay `string` (ADR-0031), never quoted outside `src/mocks/` and tests, but `edge.kind` in `src/flow/`. Rollups, stages, marks, author types: `labels.ts` alone, `src/trace/`'s too.
- **States**: loading: a skeleton, `aria-busy`; empty: the meaning, the next step; error: the daemon's message verbatim, Retry. Live regions (polite for results, assertive for a 409) sit outside any inert subtree.
- **Dialogs** in-app (`role="dialog"`, `aria-modal`, focus trap, Esc but mid-IME, focus back to the trigger; a scrim click closes only the palette and a work item's modal; no `alert`, `confirm`, `prompt`, `showModal`). A submit is one call; while pending no dialog closes or opens; a refusal keeps the dialog, the typed text and a `role="alert"`; a proposal revised meanwhile shows as changed.
- **Keyboard**: hotkeys act only with focus in their region, outside text fields, unmodified (WCAG 2.1.4), letters by `letterOf`; `?` lists them. Cmd/Ctrl-K opens the palette over no dialog but the document modal: the shell's keydown listener, the only one on `document` or `window`. Skip link, landmarks, one `h1` per view, a `:focus-visible` ring; focus never on `body` nor under a sticky bar (scroll padding).
- **Text**: chrome from `src/i18n/` (`ui-languages.md`); data is text, no HTML sink, a proposal's login escaped (`ProposalCard`), each `href` from `routes.ts`; wraps (`overflow-wrap: anywhere`); anything cut is seen whole; filters compare `normalize("NFC").toLowerCase()`; no "block" wording (ADR-0012), `blocked` only as `spec check`'s verdict: the `KnownCheckVerdict` line, mocks, tests; shown "Fails the check". Prose renders as markdown only via `src/markdown/` (ADR-0036): raw HTML as text, an anchor only where the links read resolved one.
- **Canvas** (`@xyflow/react`, lazy, hand-laid; the Map: `spec-map.md`): read-only, key options `null`, the wheel scrolls the page, a press up to 5 px a click; Flow's pans, zooms by key; `base.css` only, each `--xy-*` from a token; a List its text twin; attribution hidden, credited as text.
- **Shell**: hash routes `#/<project>[/<section>[/<id>]]` (bare: home; Flow: `spec-flow-place.md`; `/page`, `doc`: `doc-view.md`), nine sections, routed whatever the nav lists (`workspaces.md`); "Mock data" on every route while the mock serves; per-project screen state in the Shell (Questions: `round-ui.md`), but the Tree's Show drift, `specengine.{board.as,board.lanes.<p>,workspace.<p>[.<login>],module.<p>[.<login>],language,theme}` in `localStorage`; a root error boundary, one per view.

## Dependencies

The owner's allowlist: the 21 below (2026-10-05; then ADR-0036, ADR-0072, ADR-0080). `package.json` (both lists) names exactly the table's, at its versions; `packageManager` `pnpm@10.28.2`; `ui_policy.rs` checks these and the lockfile agree. Inter 4.001 (OFL-1.1) is files, no package: `ui/fonts/inter/`.

| Package | Version | Role |
|---|---|---|
| `react` | 19.3.0 | runtime |
| `react-dom` | 19.3.0 | runtime |
| `@tanstack/react-query` | 5.104.0 | data fetching |
| `@xyflow/react` | 12.12.0 | graph view |
| `vite` | 8.3.1 | dev server, build |
| `@vitejs/plugin-react` | 6.1.1 | React for Vite |
| `typescript` | 6.0.3 | type check |
| `@types/react` | 19.3.0 | types |
| `@types/react-dom` | 19.3.0 | types |
| `eslint` | 10.11.0 | lint |
| `typescript-eslint` | 8.70.1 | TS lint |
| `eslint-plugin-react-hooks` | 7.1.1 | hooks rules |
| `vitest` | 5.0.2 | test runner |
| `@testing-library/react` | 16.3.3 | component tests; `@testing-library/dom` lockfile only |
| `jsdom` | 29.1.1 | test DOM |
| `react-markdown` | 10.1.0 | markdown, `src/markdown/` only |
| `remark-gfm` | 4.0.1 | GFM for it |
| `i18next` | 26.5.11 | strings, `src/i18n/` only |
| `react-i18next` | 17.1.9 | React for it |
| `d3-force` | 3.0.0 | layout, `src/map/` only |
| `@types/d3-force` | 3.0.10 | types |

**Held back.** TypeScript 7 (outside `typescript-eslint`'s peers), jsdom 30 (Node >= 24.15, here 24.14).

**Pinned-versions policy.** Exact `x.y.z` only: no range, tag, URL, `file:`, `link:` or alias. Pins are at least 7 days old (`pnpm-workspace.yaml` `minimumReleaseAge: 10080`). `.npmrc`: `save-exact=true`, `strict-peer-dependencies=true`, the default isolated linker (never `node-linker=hoisted` or `shamefully-hoist`). `pnpm-lock.yaml` is committed, installed only frozen. Any change, security patches too, is an owner decision recorded here. Install scripts stay off: no `onlyBuiltDependencies` or `pnpm approve-builds`.

## Gates

Run in `ui/` by the UI roles:

| Command | Does |
|---|---|
| `pnpm install --frozen-lockfile` | installs the lockfile; no unmet peer |
| `pnpm lint` | `eslint . --max-warnings=0` |
| `pnpm build` | `tsc --noEmit`, `vite build` into `ui/dist/` |
| `pnpm test` | `vitest run`; a test calling `console.error` or `console.warn` fails |

From the root: the docs gate (`CLAUDE.md` "Documentation") and `cargo nextest run -p specengine-eval --test ui_policy --test anonymity --test doc_pointers`. The hook and CI run no UI gate.

## Laptop rules

- Role runs never start `pnpm dev`, `vite`, `vite preview`, bare `vitest` or any watcher. Wrap each gate: `perl -e 'alarm 600; exec @ARGV' pnpm --dir ui test`.
- Vitest: `watch: false`, at most 2 workers.

## Owner's manual steps

1. Build and install: `serve-ui.md` "Build order"; `spec serve` alone serves the UI.
2. UI work: `spec serve`, then `pnpm --dir ui dev`; open the printed `http://127.0.0.1:5173/`, not `localhost`; never `--host` with the daemon up. The mock: `?scenario=<name>` before the `#` (`src/mocks/scenario.ts`).

## Roles here

`ui-developer` writes `ui/**` and the UI tests, never this README or `src/api/generated/`; `test-engineer` owns `ui_policy.rs` and the `ui/` part of `anonymity.rs`, runs gates and mutations. Until `generated/` exists, each slice spec lists its exceptions to `.claude/agents/ui-developer.md` (ADR-0033).
