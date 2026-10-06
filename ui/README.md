---
class: canon
tier: 1
scope: [ui]
owner: owner
reviewed: 2026-10-06
---

# ui — the web UI

The owner's screens over SpecEngine: the proposal queue, tasks, the spec tree and graph, health. Rules: `docs/canon/architecture.md#ui` (ADR-0011, ADR-0014, ADR-0033), nothing blocked (`#control`). State, on mock data: `ui-shell` (shell, Inbox) shipped 2026-10-05, `ui-tree-node` (Spec tree) and `ui-graph` (Graph) 2026-10-06, specs in `docs/features/`; the rest: 08 §2 Phase 4. Screens' meaning: 07 §3 "Web UI — screens"; the owner's flow: 06 §3.3–3.4.

## Stack

A standalone pnpm project on strict TypeScript, packages in "Dependencies": no root `package.json`, no workspace with the Rust crates. Hash routing, hand-written. Tests: Vitest on jsdom with Testing Library, next to the code as `src/**/*.test.ts(x)`. Not approved, each decided at its slice: CodeMirror 6 and `@codemirror/merge`, a TS type generator, `rust-embed`, `user-event`, an a11y lint plugin, router, markdown, icon or webfont packages.

## Contract seam

- One interface, `src/api/client.ts` `SpecEngineClient`, its methods named after the daemon's endpoints (07 §3); the bootstrap `src/main.tsx` alone picks the implementation and imports `src/mocks/`.
- App code imports domain types only from `src/api/types.ts`, which re-exports `src/api/provisional.ts` today and the generated types (`src/api/generated/`) later. Provisional types copy documented shapes, each citing its source as `` `<path>` "Heading" ``; they are replaced, never extended, and the generated ones win.
- An endpoint the UI needs and 07 §3 lacks is named for `rust-developer`, never invented as a URL; the mock may serve it, flagged in the interface. The list: each slice spec's "Open" (`docs/features/ui-*.md`).
- The UI renders what the daemon returns: diffs as hunks, decisions by `apply_proposal`, refusals in its own words.
- Switching to the daemon: add an HTTP implementation, switch the bootstrap, re-point `types.ts`.

## Screen rules

Every slice keeps these (the UI tests, `ui_policy.rs`).

- **Tokens**: `src/styles/tokens.css` holds every colour literal, as semantic roles, and the spacing, type, motion (0 under reduced motion) and `--target-min: 24px` tokens; dark only. WCAG 2.2 AA on every surface: text ≥ 4.5:1; focus ring, control border, statuses ≥ 3:1. `cannot-verify` has its own colour and icon.
- **Status** never by colour alone: label and icon. An unknown value is a neutral badge with the raw text, sorted last; `kind`, `contour`, spec statuses, link types (but `mentions`) stay `string` (ADR-0031), never quoted outside `src/mocks/` and tests.
- **States**: loading, a skeleton and `aria-busy`; empty, the meaning and the next step; error, the daemon's message verbatim and Retry.
- **Dialogs** in-app only (`role="dialog"`, `aria-modal`, focus trap, Esc, focus back to the trigger; no `alert`, `confirm`, `prompt`, `showModal`). A submit is one call; while it is pending nothing closes or opens a dialog; a refusal keeps the dialog, the typed text and a `role="alert"` message; a proposal revised meanwhile is shown as changed and needs a fresh submit.
- **Keyboard**: hotkeys act only with focus in their region, outside text fields, without modifiers (WCAG 2.1.4); `?` lists them. Skip link, landmarks, one `h1` per view, a `:focus-visible` ring; focus is never left on `body`.
- **Live regions**, polite for results and assertive for a 409, sit outside any inert subtree.
- **Text**: data is text, no HTML sink, each `href` from `sectionHash`; wraps (`overflow-wrap: anywhere`), nothing cut without a way to see it whole; filters compare `normalize("NFC").toLowerCase()`; no "block" wording (ADR-0012).
- **Canvas** (`@xyflow/react`, lazy; hand-written layout, no package): read-only, key options `null` (no `document` listener), the wheel scrolls the page; `base.css` only, each `--xy-*` from a token; a List holds the answer as text; attribution hidden, credited as text (owner question).
- **Shell**: hash routes `#/<project>/<section>[/<id>]`; "Mock data" on every route while the mock serves; an unbuilt section says "Not built yet: arrives in slice `<slug>`"; a root error boundary and one per view.

## Dependencies

The owner's allowlist of 2026-10-05: exactly these 15 names in `package.json` (both dependency lists), each at the exact version below; `packageManager` `pnpm@10.28.2`. `crates/specengine-eval/tests/ui_policy.rs` compares this table with `package.json` and the lockfile.

| Package | Version | Role |
|---|---|---|
| `react` | 19.3.0 | runtime |
| `react-dom` | 19.3.0 | runtime |
| `@tanstack/react-query` | 5.104.0 | data fetching |
| `@xyflow/react` | 12.12.0 | graph view (from `ui-graph`) |
| `vite` | 8.3.1 | dev server, build |
| `@vitejs/plugin-react` | 6.1.1 | React transform for Vite |
| `typescript` | 6.0.3 | type check |
| `@types/react` | 19.3.0 | types |
| `@types/react-dom` | 19.3.0 | types |
| `eslint` | 10.11.0 | lint, flat config |
| `typescript-eslint` | 8.70.1 | TS lint rules and parser |
| `eslint-plugin-react-hooks` | 7.1.1 | hooks rules |
| `vitest` | 5.0.2 | test runner |
| `@testing-library/react` | 16.3.3 | component tests; peer `@testing-library/dom` in the lockfile only, never imported |
| `jsdom` | 29.1.1 | test DOM |

**Held back.** TypeScript 7 is outside `typescript-eslint`'s peer range; jsdom 30 needs Node ≥ 24.15 (the laptop has 24.14): an owner decision after a Node upgrade.

**Pinned-versions policy.** Exact `x.y.z` only: no range, tag, URL, `file:`, `link:` or alias. Each pin was the newest stable version satisfying React 19 and every peer range of the set, published at least 7 days earlier: `ui/pnpm-workspace.yaml` `minimumReleaseAge: 10080` (minutes) keeps pnpm from resolving a younger one. `ui/.npmrc`: `save-exact=true`, `strict-peer-dependencies=true`, the default isolated linker (never `node-linker=hoisted` or `shamefully-hoist`). `ui/pnpm-lock.yaml` is committed, installed only frozen. A new package, a removal or a version change, security patches included, is an owner decision recorded in this table. Dependency install scripts stay off (pnpm's default): no `onlyBuiltDependencies`, no `pnpm approve-builds`.

## Gates

Run in `ui/` by `ui-developer` and `test-engineer`, each exits on its own:

| Command | Does |
|---|---|
| `pnpm install --frozen-lockfile` | installs exactly the lockfile; no unmet peer |
| `pnpm lint` | `eslint . --max-warnings=0` |
| `pnpm build` | `tsc --noEmit` and `vite build` into `ui/dist/` (git-ignored) |
| `pnpm test` | `vitest run`, never watch; a test calling `console.error` or `console.warn` fails |

From the root: the docs gate (`CLAUDE.md` "Documentation") and `cargo nextest run -p specengine-eval --test ui_policy --test anonymity --test doc_pointers`. The pre-commit hook and CI do not run the UI gates.

## Laptop rules

- Role runs never start `pnpm dev`, `vite`, `vite preview`, bare `vitest` or any watcher: they do not exit. Wrap each gate: `perl -e 'alarm 600; exec @ARGV' pnpm --dir ui test`.
- Vitest: `watch: false`, at most 2 workers, in the config.
- The docs walk (`crates/specengine-store/src/source.rs` `walk`) descends `node_modules` (`exclude` matches files; symlinks skipped): pnpm's isolated layout keeps that cheap, a hoisted one would not.

## Owner's manual steps

1. `pnpm --dir ui install --frozen-lockfile` once per lockfile change.
2. `pnpm --dir ui dev`, open the printed URL; Ctrl-C stops it. Mock projects `harbor-sim` and `ledger-api`; decisions live in memory until a reload. Scenarios, before the `#`: `?scenario=empty`, `error` (reads fail, 503), `slow` (1.5 s a call), `conflict` (a decision fails, 409), `large` (3 000+ nodes).
3. The slice's own check list: its spec, "Owner's manual check".
4. Optional: `ui/node_modules` on Spotlight's privacy list.

## Roles here

`ui-developer` writes `ui/**` and the UI tests, never this README or `src/api/generated/`; `test-engineer` owns `ui_policy.rs` and the `ui/` part of `anonymity.rs`, runs the gates and the slice's mutations; `spec-writer` keeps this README. Until `src/api/generated/` exists, each slice spec lists its exceptions to `.claude/agents/ui-developer.md` (ADR-0033).
