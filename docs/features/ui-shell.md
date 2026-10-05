---
class: spec
status: shipped
scope: [ui, crates/specengine-eval]
ref: ui-shell analysis 2026-10-05, answers accepted (Q1 = the owner's instruction to start now; 15 packages approved); iterations 1-3, review accepted
shipped: 2026-10-05
adrs: [ADR-0033]
---

# UI shell: the Inbox on mocks

## Why

The owner decides proposals in a terminal (`proposal-apply`); the queue card (06 §3.3, 07 §3 "Queue") is where a decision reads fastest. Starting the UI before the daemon (ADR-0033) shapes it during Phases 1–2 with no daemon work: one client seam, a typed mock behind it, provisional types the generated ones replace. Slice 1: the pinned toolchain, one token file, the shell with six sections, the Inbox. MVP stays Phases 0–2 on CLI + MCP (08 §2). What became true: `ui/README.md` ("Contract seam", "Screen rules", "Dependencies"), `docs/canon/architecture.md#ui`.

## Data

The provisional contract `ui/src/api/provisional.ts` cites, kept until `src/api/generated/` replaces it; keys as the JSON, absent = `null`; the endpoints it presumes: "Open".

- `Project {slug, name}`; `NodeView` = `spec show --json` (`crates/specengine-cli/README.md` "Output and the cap"); `Inbox {proposals, notes}`; `Proposal` = the `review` JSON (`docs/features/proposal-apply.md` "Data") + 05 §3.3's `severity`, `gap_type`, `task_id`, `target_ids`, `evidence [{file, qpath, lines, observed, documented}]`, `options [{label, effect, price}]`, `recommendation`, `working_answer` + 07 §1.2's `summary`.
- `Decision` (06 §3.4, 07 §2): `{decision: "accept", option, note}` (both nullable), `{decision: "reject", reason}`, `{decision: "needs_clarification", note}`, `{decision: "defer", note}` (nullable). `DecisionResult {proposal, commit: {sha, subject} | null}`; `ApiError {status, message}`, 409 = decided elsewhere.
- `kind`, `contour`: `string` (ADR-0031). Closed tables (severity `high|normal|low`, proposal states, gap type, `preview`): their literals, any other string kept verbatim.
- Outcomes (the mock's today): accept → `applied`, `commit {sha, subject: "spec: apply PR-…"}` (05 §7 item 4); reject → `rejected`; both leave the inbox. Needs clarification → `changes_requested`, defer → `deferred`; both stay.

## Acceptance criteria

Verifiers: Vitest (`ui/src/**/*.test.ts(x)`), `ui_policy.rs`, the gates. M: the mutation that must turn it red.

- [x] AC-01 — `package.json` names = the "Dependencies" table's; versions exact, equal to the table once filled; `packageManager` exact; lockfile present; `.npmrc` per policy; no install script, no `onlyBuiltDependencies` (M: `globals` added; one `^`; lockfile deleted; `node-linker=hoisted`; a `postinstall`).
- [x] AC-02 — `pnpm --dir ui install --frozen-lockfile` exits 0, no unmet-peer line; `pnpm --dir ui ls --depth 0` lists exactly the 15 (M: a version changed in `package.json` only).
- [x] AC-03 — under `perl -e 'alarm 600; exec @ARGV'`, `pnpm --dir ui lint`, `build`, `test` exit 0, lint 0 warnings (M: an unused variable; a type error).
- [x] AC-04 — no `alert(`, `confirm(`, `prompt(` in `ui/src` (M: `window.confirm` in a component: lint and `ui_policy` red).
- [x] AC-05 — the Inbox on a stub client shows its data and counts its calls; only `src/main.tsx` imports `src/mocks/`, only `src/api/` imports `provisional` (M: a component imports either).
- [x] AC-06 — `provisional.ts` opens with the header; each type cites an existing heading; a test assigns `kind: "widget"` and builds (M: header dropped; a cited heading renamed; `kind` a union — caught by the build gate, not `pnpm test`).
- [x] AC-07 — severity `urgent`, an unknown status and kind: neutral badge, raw text, sorted after `low` (M: unknown mapped to `normal`).
- [x] AC-08 — no hex, `rgb(`, `hsl(`, `oklch(`, `color-mix(` in `ui/src` outside `tokens.css` (M: `color: #fff` in a component).
- [x] AC-09 — a test reads `tokens.css?raw`, resolves aliases, checks each contrast pair (M: `--text-secondary` `#5a6b85`; a sync token deleted; `cannot-verify` = `ok`).
- [x] AC-10 — `:focus-visible` uses `--focus-ring`, no bare `outline: none`; reduced motion zeroes each `--duration-*` (M: the media block removed; `outline: none` on buttons).
- [x] AC-11 — six nav entries in order, `aria-current` on the active; each unbuilt route names its slice; back returns; unknown → "Not found" (M: a route dropped; no `aria-current`).
- [x] AC-12 — `slow`: skeleton, `aria-busy="true"`; `empty`: meaning and next step; `error`: message verbatim, Retry calls `getInbox` again (M: generic text; Retry inert).
- [x] AC-13 — Tab: skip link, header, nav, list, card; arrows and `j`/`k` move; `a` typed in the filter opens nothing; Esc closes, focus returns, Tab stays inside (M: a document-wide key listener; no focus return).
- [x] AC-14 — empty reason or note: no call, a message; double click: one call; success announced with sha and subject; a rejected call keeps dialog and text; `conflict`: message, refetch, proposal gone (M: no pending guard; reset on error; no refetch).
- [x] AC-15 — no `blocked`, `blocking`, `blocker`, `unblock` (any case) in `ui/src`, no provisional key starting `block` (M: a "Blocking" label).
- [x] AC-16 — the setup fails a test on `console.error`/`warn`; a throwing view shows its fallback, the nav still works (M: the per-view boundary removed).
- [x] AC-17 — "Mock data" on every route and scenario, a non-normal one named (M: shown on the Inbox only).
- [x] AC-18 — a raw Cyrillic letter in `ui/src` fails `anonymity.rs`, one in `ui/dist` or `ui/node_modules` does not; the filter matches a composed query on decomposed text (M: `ui` unscanned; `dist` unskipped; no NFC).
- [x] AC-19 — the two `nodeKinds` disjoint, each mock node's kind in its set; no kind quoted in `ui/src` outside `src/mocks/` and tests (M: `kind === "mechanic"` in a component).
- [x] AC-20 — `ui` in roots; `ui/README.md` tier 1 ≤ 8 192 B; ADR-0033 ≤ 1 536 B, `canon:` `#ui`, a "Cost"; docs gate clean; worst W ≤ min(109 484, W at the start); this spec and 08 < 15 737 B; `CLAUDE.md` not grown (M: `ui` out of roots).
- [x] AC-21 — `test` is `vitest run`, the config `watch: false`, ≤ 2 workers; no role command starts `dev`, `vite`, `preview` or a watcher (the reviewer reads the commands) (M: `test` = `vitest`; the cap removed).

## Owner's manual check

`pnpm --dir ui install --frozen-lockfile`, then `pnpm --dir ui dev` (`ui/README.md` "Owner's manual steps"):

- each scenario; in `normal`, PR-0046 shows a refused apply verbatim; PR-0045's 300-character token and the decomposed non-Latin text of PR-0043 and DOM-BERTHS wrap, and the filter finds them;
- 200 % zoom reflows without horizontal scroll; targets ≥ 24 px; a visible focus ring on every control; VoiceOver reads badges with their labels;
- `slow`: while "Sending", Esc and Cancel do nothing and the hotkeys open no other dialog; browser Back while sending stays where Back went;
- in a dialog, click its text, then Shift+Tab: focus stays inside;
- `error`: VoiceOver speaks the error once, not again at each Retry; the switcher's Retry reads "Retry loading the projects";
- tests only (the mock cannot stage them): the "This proposal changed since you opened it" notice, focus after a successful Retry.

## Open

- **Decision precondition** (daemon contract, `ui-live`): `Decision.option` indexes the options as shown; the request should carry the shown proposal's `updated_at` (or `patch_hash`) so the daemon refuses a decision on a revised one. Today only the dialog guards it.
- **Endpoints** (shapes in "Data"): `GET /api/projects`, `GET …/proposals/:id`, `GET …/nodes/{*ref}`, the error body: settled by `docs/features/daemon-read.md` "Data", built with it. Open (`rust-developer`, `ui-live`): deciding (its POST answers 403 until its Q4; `Decision` → `DecisionResult`, 409, a refused apply); states `changes_requested`, `deferred`; the generator writing `src/api/generated/`.
- **Owner's `.claude/` edits** (Q1, due now): `.claude/agents/ui-developer.md` lines 47–48 → "Called from ADR-0033 on; slice specs list exceptions until `src/api/generated/` exists"; `.claude/commands/feature.md` line 25 drops "(Phase 4+)".
- After the manual check: the card's section order (diff last). jsdom 30 after a Node ≥ 24.15 upgrade (`ui/README.md` "Held back").

## Implementation

| Module | What it does |
|---|---|
| `ui/` root | `package.json` (15 exact pins, `pnpm@10.28.2`, scripts `dev`, `lint`, `build`, `test`), `pnpm-lock.yaml`, `.npmrc`, `pnpm-workspace.yaml`, `.gitignore`, `tsconfig.json`, `vite.config.ts` (jsdom, `watch: false`, 2 workers, dev server on 127.0.0.1), `eslint.config.js` (strict type-checked, hooks, `no-alert`, `no-explicit-any`, seam imports), `index.html` |
| `src/api/` | `client.ts` the seam and `ClientError`; `provisional.ts` the cited types; `types.ts`; `provider.tsx` (retry 0, no refetch on focus); `queries.ts`, the only TanStack Query user |
| `src/mocks/` | `MockClient` over `harbor-sim` and `ledger-api` (each its `kinds.ts`), `scenario.ts`, in-memory decisions |
| `src/app/` | the shell: header, project switcher (failure, Retry), "Mock data", shortcuts, nav, hash routes (`routes.ts`, `location.ts`), error boundaries, unbuilt views |
| `src/inbox/` | ordered, filtered list; card (targets, evidence, options, diff); four decision dialogs with the revised-proposal notice |
| `src/ui/` | dialog (trap, Esc, focus return), badge, icon, states, live regions (`announcer.tsx`), focus helpers (`focus.ts`, `useFocusLater.ts`) |
| `src/styles/` | `tokens.css`, the only colour literals; `app.css` |
| `src/test/` | the setup failing on `console.error`/`warn`, a stub client, builders |
| eval tests | `ui_policy.rs` (new, 17); `anonymity.rs` scans `ui/` but `ui/dist` and `node_modules`; `doc_pointers.rs` walks `ui/src/**/*.ts(x)` |
| `specengine.toml` | `ui` in `roots` |

Tests: 301 Vitest tests in 16 files; lint 0 warnings, build green; each AC's named mutation red.

Iterations: 1, the slice (273 tests); the review found a pending decision closable by Esc or Cancel and decidable twice. 2, guards while pending, a dialog closing only itself, live regions outside the inert subtree, focus kept on a refusal or a failed Retry, the switcher's failure and Retry (291). 3, browser Back while sending keeps history, the revised-proposal notice, `role="alert"` on title and message only, focus after a successful Retry, Shift+Tab from the panel, the Retry accessible name, a narrowed `try` (301).

Deviations (accepted): jsdom 29.1.1, not 30; runtime and presentation types outside `provisional.ts` (`ClientError`, `MockProject`); an extra token `--attention` (≥ 3:1); accept preselects option 1 when nothing is recommended; the mock answers 422 to accepting a `conflicts` preview; after the last item is decided focus goes to the previous one, none left → the `h1`; `#/<project>` without a section → "Not found"; `ui/pnpm-workspace.yaml` joined the scaffold.
