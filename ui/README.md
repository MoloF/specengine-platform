---
class: canon
tier: 1
scope: [ui]
owner: owner
reviewed: 2026-10-06
---

# ui -- the web UI

The owner's screens: home, queue, tasks, spec tree and graph, health. Rules: `docs/canon/architecture.md#ui` (ADR-0011, ADR-0014, ADR-0033, ADR-0036), nothing blocked (`#control`). State: `ui-shell`, `ui-tree-node`, `ui-graph`, `ui-tasks`, `ui-home`, `ui-markdown` shipped on mocks, `daemon-read` on `specengine-http` (2026-10-05, -06); the rest: 08 s2 Phase 4. Meaning: 07 s3 "Web UI — screens"; the owner's flow: 06 s3.3-3.4.

## Stack

A standalone pnpm project on strict TypeScript, packages in "Dependencies": no root `package.json`, no workspace with the Rust crates. Tests: Vitest on jsdom with Testing Library, next to the code as `src/**/*.test.ts(x)`, `fetch` and `EventSource` stubbed.

## Contract seam

- One interface, `src/api/client.ts` `SpecEngineClient`, its methods named after the daemon's endpoints (07 s3). The bootstrap `src/main.tsx` alone picks one: `src/api/http.ts` `HttpClient` by default (`vite.config.ts` proxies `/api` to :7777), the mock `src/mocks/` for any `?scenario=`.
- App code imports domain types only from `src/api/types.ts`, re-exporting `src/api/provisional.ts` now, the generated types (`src/api/generated/`) later. Provisional types copy documented shapes, each citing its source as `` `<path>` "Heading" `` (replaced, never extended: `#ui`).
- An endpoint 07 s3 lacks is named for `rust-developer` in the slice spec's "Open", never invented; the mock may serve it, flagged in the interface; `HttpClient` rejects it unsent, `ClientError {status: 501, notServed: true}`: "Not built yet", no Retry. Status 0: no response.
- The UI renders what the daemon returns: diffs as hunks, decisions by `apply_proposal`, refusals in its own words. An owner action with no endpoint is its `spec` command to copy (fixed words, a validated ID); a decision is at most staged, confirmed on a terminal (`docs/canon/decision-staging.md`, not built yet).

## Screen rules

Every slice keeps these (UI tests, `ui_policy.rs`).

- **Tokens**: `src/styles/tokens.css` holds every colour literal (semantic roles) and the spacing, type, motion (0 if reduced), `--target-min: 24px` tokens; dark only. WCAG 2.2 AA: text >= 4.5:1; focus ring, control border, statuses >= 3:1; `cannot-verify` its own colour and icon.
- **Status** never by colour alone: label and icon. An unknown value: a neutral badge, the raw text, sorted last; `kind`, `contour`, `role`, `profile`, spec statuses, link types (but `mentions`) stay `string` (ADR-0031), never quoted outside `src/mocks/` and tests.
- **States**: loading: a skeleton, `aria-busy`; empty: the meaning, the next step; error: the daemon's message verbatim, Retry. Live regions (polite for results, assertive for a 409) sit outside any inert subtree.
- **Dialogs** in-app (`role="dialog"`, `aria-modal`, focus trap, Esc but mid-IME, focus back to the trigger; a scrim click closes only the palette; no `alert`, `confirm`, `prompt`, `showModal`). A submit is one call; while pending no dialog closes or opens; a refusal keeps the dialog, the typed text and a `role="alert"`; a proposal revised meanwhile shows as changed, to submit again.
- **Keyboard**: hotkeys act only with focus in their region, outside text fields, unmodified (WCAG 2.1.4); `?` lists them. Cmd-K or Ctrl-K opens the palette anywhere: the shell's `document` keydown listener, no other on `document` or `window`. Skip link, landmarks, one `h1` per view, a `:focus-visible` ring; focus never left on `body`.
- **Text**: data is text, no HTML sink, each `href` from `routes.ts`; wraps (`overflow-wrap: anywhere`), anything cut can be seen whole; filters compare `normalize("NFC").toLowerCase()`; no "block" wording (ADR-0012). Prose renders as markdown only via `src/markdown/` (ADR-0036): raw HTML as text, an anchor only where the links read resolved one; Rendered by default, Source a toggle away.
- **Canvas** (`@xyflow/react`, lazy, a hand-written layout): read-only, key options `null`, the wheel scrolls the page; `base.css` only, each `--xy-*` from a token; a List holds the answer as text; attribution hidden, credited as text.
- **Shell**: hash routes `#/<project>[/<section>[/<id>]]` (bare: its home); "Mock data" on every route while the mock serves; an unbuilt section says "Not built yet: arrives in slice `<slug>`"; a root error boundary and one per view.

## Dependencies

The owner's allowlist: the 17 below (2026-10-05; the last two 2026-10-06, ADR-0036). `package.json` (both lists) names exactly the table's, at its versions; `packageManager` `pnpm@10.28.2`. `ui_policy.rs` checks table, `package.json` and lockfile agree.

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
| `eslint` | 10.11.0 | lint, flat config |
| `typescript-eslint` | 8.70.1 | TS lint |
| `eslint-plugin-react-hooks` | 7.1.1 | hooks rules |
| `vitest` | 5.0.2 | test runner |
| `@testing-library/react` | 16.3.3 | component tests; peer `@testing-library/dom` in the lockfile only, never imported |
| `jsdom` | 29.1.1 | test DOM |
| `react-markdown` | 10.1.0 | markdown to React elements, `src/markdown/` only |
| `remark-gfm` | 4.0.1 | GFM tables, task lists, footnotes, autolinks |

**Held back.** TypeScript 7 (outside `typescript-eslint`'s peer range), jsdom 30 (Node >= 24.15, the laptop 24.14): the owner decides after a Node upgrade.

**Pinned-versions policy.** Exact `x.y.z` only: no range, tag, URL, `file:`, `link:` or alias. Pins are at least 7 days old (`ui/pnpm-workspace.yaml` `minimumReleaseAge: 10080`). `ui/.npmrc`: `save-exact=true`, `strict-peer-dependencies=true`, the default isolated linker (never `node-linker=hoisted` or `shamefully-hoist`). `ui/pnpm-lock.yaml` is committed, installed only frozen. Any change, security patches too, is an owner decision recorded here. Install scripts stay off (pnpm's default): no `onlyBuiltDependencies`, no `pnpm approve-builds`.

## Gates

Run in `ui/` by the UI roles, each exits on its own:

| Command | Does |
|---|---|
| `pnpm install --frozen-lockfile` | installs exactly the lockfile; no unmet peer |
| `pnpm lint` | `eslint . --max-warnings=0` |
| `pnpm build` | `tsc --noEmit` and `vite build` into `ui/dist/` (git-ignored) |
| `pnpm test` | `vitest run`, never watch; a test calling `console.error` or `console.warn` fails |

From the root: the docs gate (`CLAUDE.md` "Documentation") and `cargo nextest run -p specengine-eval --test ui_policy --test anonymity --test doc_pointers`. Neither the pre-commit hook nor CI runs the UI gates.

## Laptop rules

- Role runs never start `pnpm dev`, `vite`, `vite preview`, bare `vitest` or any watcher: they do not exit. Wrap each gate: `perl -e 'alarm 600; exec @ARGV' pnpm --dir ui test`.
- Vitest: `watch: false`, at most 2 workers, in the config.
- The docs walk descends `node_modules`, symlinks skipped: cheap only in pnpm's isolated layout.

## Owner's manual steps

1. `pnpm --dir ui install --frozen-lockfile` once per lockfile change.
2. `specengine-http --root <project>`, then `pnpm --dir ui dev`; open the printed `http://127.0.0.1:5173/`, not `localhost`. Never `pnpm dev --host` with the daemon up. The mock, `?scenario=` before the `#`: `normal`, `empty`, `error` (reads fail, 503), `slow` (1.5 s a call), `conflict` (a decision fails, 409), `large` (3 000+ nodes); projects `harbor-sim`, `ledger-api`, decisions in memory until a reload.
3. The slice's own check list: its spec, "Owner's manual check".

## Roles here

`ui-developer` writes `ui/**` and the UI tests, never this README or `src/api/generated/`; `test-engineer` owns `ui_policy.rs` and the `ui/` part of `anonymity.rs`, runs the gates and the slice's mutations; `spec-writer` keeps this README. Until `src/api/generated/` exists, each slice spec lists its exceptions to `.claude/agents/ui-developer.md` (ADR-0033).
