---
class: spec
status: draft
scope: [ui, crates/specengine-eval]
ref: ui-shell analysis 2026-10-05, answers accepted (Q1 = the owner's instruction to start now; 15 packages approved)
adrs: [ADR-0033]
---

# UI shell: the Inbox on mocks

## Why

The owner decides proposals in a terminal (`proposal-apply`); the queue card (06 §3.3, 07 §3 "Queue") is where a decision reads fastest. Starting the UI before the daemon (ADR-0033) shapes it during Phases 1–2 with no daemon work: one client seam, a typed mock behind it, provisional types the generated ones replace. Slice 1: the pinned toolchain, one token file, the shell with six sections, the Inbox. MVP stays Phases 0–2 on CLI + MCP (08 §2).

## Description and interactions

The owner runs `pnpm --dir ui dev` (`ui/README.md` "Owner's manual steps"): "Mock data" always shown, six sections, the Inbox over two invented projects; four decisions change the mock's in-memory state (reset on reload). No agent, CLI, MCP or core change; crates gain tests only.

## Roles

- `ui-developer`: `ui/**` but `ui/README.md`; the UI tests; reports the pinned versions, pnpm's, the missing endpoints.
- `test-engineer`: `anonymity.rs` scans `ui/`, skipping `node_modules` and `dist`; `doc_pointers.rs` also walks `ui/src/**/*.ts(x)`; new `ui_policy.rs` (AC-01, -04, -06, -08, -15, -19, -21); the gates; each AC's mutation applied to UI code and reverted, each red reported.
- `spec-writer`, after: the versions into `ui/README.md` "Dependencies".

### Exceptions to the `ui-developer` prompt (this slice; `.claude/` untouched)

`.claude/agents/ui-developer.md`: lines 47–48 (not called before Phase 4) → called now (ADR-0033). 18–23 (contract only through generated types, no own domain types) → provisional types (Data), still no `any`. 40–43 (regenerate types, `git diff --exit-code`; manual check with `spec serve`) → no regenerate-and-diff step; manual check on Vite with mocks. 36–38 (contrast in light and dark) → dark only. 10–12 (dagre or ELK) → no graph, layout unapproved.

## Data

**Scaffold** (`ui/`): `package.json` (`private`, `type: module`, `packageManager`, the scripts of `ui/README.md` "Gates" plus `dev`), `pnpm-lock.yaml`, `.npmrc` ("Dependencies"), `.gitignore` (`*.tsbuildinfo`, `coverage/`), `tsconfig.json` (`strict`, `noUncheckedIndexedAccess`, `skipLibCheck`, `types: ["vite/client"]`; no Node API anywhere, files read via `?raw`), `vite.config.ts` (Vitest `jsdom`, `watch: false`, ≤ 2 workers, a setup failing a test on `console.error`/`warn`), `eslint.config.js` (flat: `typescript-eslint`, `react-hooks`; `no-explicit-any`, `no-alert`, `no-restricted-imports` on the seam). No template extras (`@eslint/js`, `globals`, `eslint-plugin-react-refresh`).

**Contract seam.**

```ts
export interface SpecEngineClient {
  readonly dataSource: "mock" | "daemon"; // the "Mock data" indicator
  getProjects(): Promise<Project[]>; // MISSING ENDPOINT GET /api/projects
  getInbox(project: string): Promise<Inbox>; // GET /api/projects/:p/inbox
  getNode(project: string, id: string): Promise<NodeView>; // GET /api/projects/:p/nodes/:id
  decideProposal(project: string, id: string, d: Decision): Promise<DecisionResult>; // POST …/proposals/:id/decision
}
```

`src/api/provisional.ts` opens with `// PROVISIONAL — hand-written until \`spec serve\` generates types from Rust; replace, do not extend; generated types win.`; each type cites its source as `` `<path>` "Heading" ``; keys as the JSON, absent = `null`:

- `Project {slug, name}`; `NodeView` = `spec show --json` (`crates/specengine-cli/README.md` "Output and the cap").
- `Inbox {proposals, notes}`; `Proposal` = the `review` JSON (`docs/features/proposal-apply.md` "Data") + `severity`, `gap_type`, `task_id`, `target_ids`, `evidence [{file, qpath, lines, observed, documented}]`, `options [{label, effect, price}]`, `recommendation`, `working_answer` (05 §3.3) + `summary` (07 §1.2).
- `Decision` (06 §3.4, 07 §2): `{decision: "accept", option, note}` (option, note nullable), `{decision: "reject", reason}`, `{decision: "needs_clarification", note}`, `{decision: "defer", note}` (nullable).
- `DecisionResult {proposal, commit: {sha, subject} | null}`; `ApiError {status, message}`, 409 = decided elsewhere.
- `kind`, `contour`: `string` (ADR-0031). Closed tables (severity `high|normal|low`, statuses, gap type, `preview`; later link types, sync): their literals, any other string accepted.

Read shapes copy shipped CLI JSON (`show` now; `tree`, `graph`, `check` later). `src/api/types.ts` = `export type * from "./provisional"`. Query hooks in `src/api/` call only the interface, from a context the bootstrap provides; `retry: 0`, no refetch on focus.

**Mocks** (`src/mocks/`): `MockClient` over typed fixtures of two invented projects, `harbor-sim` (a harbour-simulation game) and `ledger-api` (a payments service): Latin IDs, English, non-Latin only as `\u` escapes, nothing from the pilots; each declares disjoint `nodeKinds` in `src/mocks/<slug>/kinds.ts` (`domain`, `mechanic`, `rule`; `service`, `endpoint`, `policy`). The normal inbox: every severity and an unknown one, an unknown kind and status, options with a recommendation, a proposal without options, two proposals on one node, a diff, a 300-character unbroken token, decomposed non-Latin text. `?scenario=` at bootstrap: `normal` (default, also for unknown values), `empty`, `error` (reads reject: 503 and a message), `slow` (1.5 s per call), `conflict` (a decision rejects 409; the proposal leaves the inbox). Accept → `applied`, `commit {sha, subject: "spec: apply PR-…"}` (05 §7 item 4); reject → `rejected`; both leave. Clarification → `changes_requested`, defer → `deferred`; both stay.

**Missing endpoints**, for `rust-developer` (`ui-live`): `GET /api/projects`; full cards from `inbox`, or `GET …/proposals/:id` (`review` JSON); the decision request and response bodies; the error body, 409; the generator writing `src/api/generated/`.

**Tokens**, `src/styles/tokens.css`, the only colour literals (sRGB hex; `--scrim` rgba), semantic roles, dark blue: `--surface-{canvas,panel,raised,overlay}`, `--border-{subtle,control}`, `--focus-ring`, `--text-{primary,secondary,on-accent}`, `--accent`, `--severity-{high,normal,low,unknown}`, `--proposal-*` (05 §3.3's seven + `unknown`), `--task-*` (its ten + `unknown`), `--sync-*` (05 §5.3's eight; `cannot-verify` its own colour and icon), `--diff-{added,removed}-surface`; `--space-1…6`, `--radius-{s,m}`, `--font-{sans,mono}` (system stacks), `--font-size-{s,m,l,xl}`, `--line-height-{body,tight}`, `--duration-{fast,normal}` (0 under `prefers-reduced-motion`), `--z-{header,dialog}`, `--target-min: 24px`. Contrast (WCAG 2.2 AA): text primary, secondary, accent ≥ 4.5:1 on each surface, primary also on the diff surfaces, on-accent on accent; focus ring, control border, each status token ≥ 3:1 on each surface.

**Routes** (hash, hand-written): `#/<project>/<section>[/<id>]`, sections `inbox`, `tasks`, `tree`, `graph`, `health`, `questions`; `#/` → the first project's inbox; else "Not found" linking the Inbox. Section and project changes push history, selection replaces.

**Config**: `specengine.toml` `roots = ["CLAUDE.md", "README.md", "crates", "docs", "ui"]`.

## Rules and edge cases

**UX and accessibility.** English, sentence case. Loading: skeleton, `aria-busy`. Empty: the meaning and the next step. Error: the daemon's message verbatim, Retry. In-app dialogs only (`role="dialog"`, `aria-modal`, focus trap, Esc, focus back to the trigger; no `showModal`, absent in jsdom). Status never by colour alone: label and icon; unknown values a neutral badge with the raw text. Skip link, landmarks, one `h1` per view, a global `:focus-visible` ring. Results in an `aria-live="polite"` region; input kept on failure; no double submit. Long or non-Latin text wraps (`overflow-wrap: anywhere`, `pre-wrap`); nothing cut without a way to see it whole. The filter compares `normalize("NFC").toLowerCase()`. No "block", "blocked", "blocking" (ADR-0012; 07 §1.2 P2-8; not 06 §3.3's "⛔ block.").

**Shell.** Header: "SpecEngine", the project switcher (labelled select), "Mock data" (icon and text, not dismissible, naming a non-normal scenario), "Keyboard shortcuts". Nav "Sections" (07 §3): Inbox (Queue), Tasks, Spec tree (Tree, Node), Graph, Health, Questions (Round); the current `aria-current="page"`. Unbuilt: its `h1` and "Not built yet: arrives in slice `<slug>`" (Next slices), no fake data. A root error boundary and one per view.

**Inbox.** List, then card (stacked when narrow). Order: severity `high`, `normal`, `low`, null or unknown; `created_at` oldest first; ID. The list is one Tab stop; ArrowDown/`j`, ArrowUp/`k` move; the first item is selected; `#/<p>/inbox/<id>` selects it, an absent ID is said beside the list. Item: severity, ID, summary, kind, task, age. Card: summary; kind, gap type, severity, task, status; targets (ID, `kind` verbatim, title) and each one's current section from `getNode` (plain text; its own error and Retry); evidence (file:lines, qpath, observed vs documented); options with price, the recommended marked; working answer; author role and model; age with its UTC time; other open proposals on the node, information only; the read-only diff from `diff` (`+`, `-`, context, `@@` lines styled, signs kept; null → "No section diff attached"). The UI computes no diff.

**Decisions.** Accept, Reject, Needs clarification, Defer (`aria-keyshortcuts` `a`, `r`, `c`, `d`), each a dialog: accept (option radio, the recommended preselected; note optional), reject (reason required), needs clarification (note required), defer (note optional). Hotkeys act only with focus in the queue, outside a text field, with no modifier (WCAG 2.1.4); `?` lists them. A submit is one `decideProposal` call, disabled while pending. Success: the dialog closes, the live region announces it (accept: sha and subject), focus to the next item. Failure: the dialog stays, `role="alert"` shows the message verbatim, the text survives. 409: the dialog closes, the message shows, the inbox refetches.

Gates, pins, laptop rules: `ui/README.md`.

## Acceptance criteria

Verifiers: Vitest (`ui/src/**/*.test.ts(x)`), `ui_policy.rs`, the gates. M: the mutation that must turn it red.

- [ ] AC-01 — `package.json` names = the "Dependencies" table's; versions exact, equal to the table once filled; `packageManager` exact; lockfile present; `.npmrc` per policy; no install script, no `onlyBuiltDependencies` (M: `globals` added; one `^`; lockfile deleted; `node-linker=hoisted`; a `postinstall`).
- [ ] AC-02 — `pnpm --dir ui install --frozen-lockfile` exits 0, no unmet-peer line; `pnpm --dir ui ls --depth 0` lists exactly the 15 (M: a version changed in `package.json` only).
- [ ] AC-03 — under `perl -e 'alarm 600; exec @ARGV'`, `pnpm --dir ui lint`, `build`, `test` exit 0, lint 0 warnings (M: an unused variable; a type error).
- [ ] AC-04 — no `alert(`, `confirm(`, `prompt(` in `ui/src` (M: `window.confirm` in a component: lint and `ui_policy` red).
- [ ] AC-05 — the Inbox on a stub client shows its data and counts its calls; only `src/main.tsx` imports `src/mocks/`, only `src/api/` imports `provisional` (M: a component imports either).
- [ ] AC-06 — `provisional.ts` opens with the header; each type cites an existing heading; a test assigns `kind: "widget"` and builds (M: header dropped; a cited heading renamed; `kind` a union).
- [ ] AC-07 — severity `urgent`, an unknown status and kind: neutral badge, raw text, sorted after `low` (M: unknown mapped to `normal`).
- [ ] AC-08 — no hex, `rgb(`, `hsl(`, `oklch(`, `color-mix(` in `ui/src` outside `tokens.css` (M: `color: #fff` in a component).
- [ ] AC-09 — a test reads `tokens.css?raw`, resolves aliases, checks each "Tokens" pair (M: `--text-secondary` `#5a6b85`; a sync token deleted; `cannot-verify` = `ok`).
- [ ] AC-10 — `:focus-visible` uses `--focus-ring`, no bare `outline: none`; reduced motion zeroes each `--duration-*` (M: the media block removed; `outline: none` on buttons).
- [ ] AC-11 — six nav entries in order, `aria-current` on the active; each unbuilt route names its slice; back returns; unknown → "Not found" (M: a route dropped; no `aria-current`).
- [ ] AC-12 — `slow`: skeleton, `aria-busy="true"`; `empty`: meaning and next step; `error`: message verbatim, Retry calls `getInbox` again (M: generic text; Retry inert).
- [ ] AC-13 — Tab: skip link, header, nav, list, card; arrows and `j`/`k` move; `a` typed in the filter opens nothing; Esc closes, focus returns, Tab stays inside (M: a document-wide key listener; no focus return).
- [ ] AC-14 — empty reason or note: no call, a message; double click: one call; success announced with sha and subject; a rejected call keeps dialog and text; `conflict`: message, refetch, proposal gone (M: no pending guard; reset on error; no refetch).
- [ ] AC-15 — no `blocked`, `blocking`, `blocker`, `unblock` (any case) in `ui/src`, no provisional key starting `block` (M: a "Blocking" label).
- [ ] AC-16 — the setup fails a test on `console.error`/`warn`; a throwing view shows its fallback, the nav still works (M: the per-view boundary removed).
- [ ] AC-17 — "Mock data" on every route and scenario, a non-normal one named (M: shown on the Inbox only).
- [ ] AC-18 — a raw Cyrillic letter in `ui/src` fails `anonymity.rs`, one in `ui/dist` or `ui/node_modules` does not; the filter matches a composed query on decomposed text (M: `ui` unscanned; `dist` unskipped; no NFC).
- [ ] AC-19 — the two `nodeKinds` disjoint, each mock node's kind in its set; no kind quoted in `ui/src` outside `src/mocks/` and tests (M: `kind === "mechanic"` in a component).
- [ ] AC-20 — `ui` in roots; `ui/README.md` tier 1 ≤ 8 192 B; ADR-0033 ≤ 1 536 B, `canon:` `#ui`, a "Cost"; docs gate clean; worst W ≤ min(109 484, W at the start); this spec and 08 < 15 737 B; `CLAUDE.md` not grown (M: `ui` out of roots).
- [ ] AC-21 — `test` is `vitest run`, the config `watch: false`, ≤ 2 workers; no role command starts `dev`, `vite`, `preview` or a watcher (the reviewer reads the commands) (M: `test` = `vitest`; the cap removed).

## Owner's manual check

On `pnpm --dir ui dev`: each scenario; 200 % zoom reflows without horizontal scroll; targets ≥ 24 px; a visible focus ring on every control; VoiceOver reads badges with labels; the long token and non-Latin text wrap.

## Next slices

`ui-tasks`; `ui-tree-node` (mocks captured from the CLI on `fixtures/spec-a`, `spec-b`); `ui-graph` (`@xyflow/react`; a dagre or ELK approval, or hand-written layering); `ui-health-round` (Health, Questions); `ui-live` after Phase 2 (HTTP client, generated types, SSE, the missing endpoints); the rest of Phase 4 (embedding, editing), each approved on its own.

## Out of scope

The daemon (`spec serve`, `specengine-http`, HTTP client, SSE, 08 AC-12, token, `Origin`, `rust-embed`, ADR-0015); type generation; the five other screens; a light theme; editing (CodeMirror, `e`, `POST /nodes/:id`); core logic in TS (diffs, rebases, rounds, checks); a UI CI job; hook and `.claude/` changes; browser e2e; any package beyond the 15.

## Open

- Q1 (owner): `.claude/agents/ui-developer.md` 47–48 → "Called from ADR-0033 on; slice specs list exceptions until `src/api/generated/` exists"; `.claude/commands/feature.md` 25 drops "(Phase 4+)". Recommended: once this ships.

## Implementation

Filled in after implementation.
