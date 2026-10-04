---
class: canon
tier: 1
scope: [ui]
owner: owner
reviewed: 2026-10-05
---

# ui — the web UI

The owner's screens over SpecEngine: the proposal queue, tasks, the spec tree and graph, health. Rules: `docs/canon/architecture.md#ui` (ADR-0011, ADR-0014, ADR-0033), nothing blocked (`#control`). State: slice 1 `ui-shell` (draft), the shell and the Inbox on mock data: `docs/features/ui-shell.md`. Screens' meaning: 07 §3 "Web UI — screens"; the owner's flow: 06 §3.3–3.4.

## Stack

React 19, TypeScript (strict), Vite, TanStack Query; `@xyflow/react` from the graph slice. A standalone pnpm project: no root `package.json`, no workspace with the Rust crates. Hash routing, hand-written (no router package). Tests: Vitest on jsdom with Testing Library, next to the code as `src/**/*.test.ts(x)`. Not approved, each decided at its slice: a graph layout (dagre, ELK), CodeMirror 6 and `@codemirror/merge` (in-place editing), a TS type generator, `rust-embed` (embedding), `user-event`, an a11y lint plugin, router, markdown, icon or webfont packages.

## Contract seam

- One interface, `src/api/client.ts` `SpecEngineClient`, its methods named after the daemon's endpoints (07 §3); the bootstrap `src/main.tsx` alone picks the implementation and the only place importing `src/mocks/`.
- App code imports domain types only from `src/api/types.ts`, which re-exports `src/api/provisional.ts` today and the generated types (`src/api/generated/`, written by a Rust-side generator, outside `ui-developer`'s hand edits) later. Provisional types copy documented shapes, each citing its source as `` `<path>` "Heading" ``; they are replaced, never extended, and the generated ones win.
- An endpoint the UI needs and 07 §3 lacks is named for `rust-developer`, never invented as a URL; the mock may serve it, flagged in the interface.
- The UI renders what the daemon returns: diffs arrive as hunks, decisions are applied by `apply_proposal`; the daemon's refusals are shown in its own words.
- Switching to the daemon: add an HTTP implementation, switch the bootstrap, re-point `types.ts`.

## Dependencies

The owner's allowlist of 2026-10-05: exactly these 15 names in `package.json` (dependencies and devDependencies together), each at an exact version. `ui-developer` reports the versions it pinned at the first install; `spec-writer` records them here, and `crates/specengine-eval/tests/ui_policy.rs` compares this table with `package.json`.

| Package | Version | Role |
|---|---|---|
| `react` | pinned at install | runtime, 19.x |
| `react-dom` | pinned at install | runtime, 19.x |
| `@tanstack/react-query` | pinned at install | data fetching through the client |
| `@xyflow/react` | pinned at install | graph view (from `ui-graph`) |
| `vite` | pinned at install | dev server, build |
| `@vitejs/plugin-react` | pinned at install | React transform for Vite |
| `typescript` | pinned at install | type check, within `typescript-eslint`'s peer range |
| `@types/react` | pinned at install | types |
| `@types/react-dom` | pinned at install | types |
| `eslint` | pinned at install | lint, flat config |
| `typescript-eslint` | pinned at install | TS lint rules and parser |
| `eslint-plugin-react-hooks` | pinned at install | hooks rules |
| `vitest` | pinned at install | test runner, matching Vite |
| `@testing-library/react` | pinned at install | component tests (its peer `@testing-library/dom` is installed, imported only through it) |
| `jsdom` | pinned at install | test DOM |

**Pinned-versions policy.** Exact `x.y.z` only: no range, tag, URL, `file:`, `link:` or alias. At the first install each is the newest stable version that satisfies React 19 and every peer range of the set, published at least 7 days earlier (pnpm's `minimumReleaseAge` = 10080 where the pinned pnpm supports it, else checked by hand). `ui/.npmrc`: `save-exact=true`, `strict-peer-dependencies=true`, the default isolated linker (never `node-linker=hoisted` or `shamefully-hoist`). `packageManager` names the exact pnpm. After the first install roles run only `pnpm install --frozen-lockfile`; `ui/pnpm-lock.yaml` is committed. A new package, a removal or a version change, security patches included, is an owner decision recorded in this table. Dependency install scripts stay off (pnpm's default): no `onlyBuiltDependencies`, no `pnpm approve-builds`; pnpm's "ignored build scripts" notice (esbuild) is expected.

## Gates

Run in `ui/` by `ui-developer` and `test-engineer`, each exits on its own:

| Command | Does |
|---|---|
| `pnpm install --frozen-lockfile` | installs exactly the lockfile; no unmet peer |
| `pnpm lint` | `eslint . --max-warnings=0` |
| `pnpm build` | `tsc --noEmit` and `vite build` into `ui/dist/` (git-ignored) |
| `pnpm test` | `vitest run`, never watch; a test calling `console.error` or `console.warn` fails |

From the root: the docs gate (`CLAUDE.md` "Documentation") and `cargo nextest run -p specengine-eval --test ui_policy --test anonymity`. The pre-commit hook and CI do not run the UI gates (no UI CI job).

## Laptop rules

- Role runs never start `pnpm dev`, `vite`, `vite preview`, bare `vitest` or any watcher: they do not exit. Wrap each gate: `perl -e 'alarm 600; exec @ARGV' pnpm --dir ui test`.
- Vitest: `watch: false`, at most 2 workers, in the config.
- The docs walk (`crates/specengine-store/src/source.rs` `walk`) skips dot entries and symlinks but descends every other directory, `node_modules` included: pnpm's isolated layout (`.pnpm` and symlinks) keeps that cheap; a hoisted layout would not.

## Owner's manual steps

1. `pnpm --dir ui install --frozen-lockfile` once per lockfile change.
2. `pnpm --dir ui dev`, open the printed local URL; stop with Ctrl-C. Scenarios: `?scenario=empty`, `error`, `slow`, `conflict` before the `#`.
3. The slice's own check list: its spec, "Owner's manual check".
4. Optional: add `ui/node_modules` to Spotlight's privacy list (System Settings, Spotlight).

## Roles here

`ui-developer` writes `ui/**` and the UI tests, never this README or `src/api/generated/`; `test-engineer` owns `ui_policy.rs` and the `ui/` part of `anonymity.rs`, runs the gates and the slice's mutations; `spec-writer` keeps this README.
