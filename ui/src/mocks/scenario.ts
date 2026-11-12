/** The mock's scenarios, picked by `?scenario=` at bootstrap (`ui/README.md` "Owner's manual steps"). */
/**
 * `cannot-check`: every read as `normal` but the check, which cannot vouch for the corpus
 * (`docs/features/ui-health.md` "Data"), and the unapplied amendments, refused 503 for the same
 * incomplete walk (`docs/features/health-left.md` "Data"). `editorial`: every read as `normal` but
 * the lock entries an editorial approval refreshes, harbor-sim's counted and ledger-api's lock
 * unreadable, and a wording fix with an editorial approve staged (`docs/features/rev-discipline.md`
 * "UI"; src/mocks/editorial.ts). `code-cannot`: every read as `normal` but Health's rows 4-6, which
 * could not be measured, row 6 for both projects; `not-configured`: as `normal` but Health's row 5,
 * which has no rule, and ledger-api's row 6, measured with zero rows (`docs/features/health-code-ui.md`
 * "Data"; `docs/features/health-drift-ui.md` "Data"; src/mocks/health.ts). The Board
 * (`docs/features/board-ui.md`; src/mocks/cards.ts): `empty` keeps the columns and no card, `conflict`
 * refuses a project's first card write as changed meanwhile, `large` puts 231 cards in one column.
 * `no-gaps`: as `normal` but harbor-sim's flow, each gap row at None, so its Flow shows the one line
 * in place of the rows (`docs/features/screen-fit.md`; src/mocks/flow.ts); ledger-api keeps its rows.
 * `config-notes`: as `normal` but the workspaces' config notes, harbor-sim's two wrong values and
 * ledger-api's malformed table (`docs/features/workspaces.md` "The UI"; src/mocks/workspaces.ts).
 * `aside`: as `normal` but harbor-sim's spec, the pottery workshop of `fixtures/flow-aside`, so Flow
 * draws only what is in the flow and lists the rest as Records for the AI; `aside40` its records
 * copied 40 times, 5 680 of them (`docs/canon/spec-flow-screen.md` "Mock"; RM-0094; src/mocks/harbor-sim/aside.ts).
 * `hub`: as `normal` behind a hub's login, `mira`, whose `[team]` row allows every act and Confirm, so
 * a staged choice is confirmed from the page as a hub does (`docs/canon/hub-confirm.md` "Routes";
 * the card's Answer..., docs/features/card-questions.md "Acceptance criteria", U4); its task counts as
 * a hub serves them. `local-only`: as `normal` but the task counts in a hub's shape, agent tasks
 * `local_only` and no list (docs/features/department-overview.md "Acceptance criteria", DO4;
 * src/mocks/cardCounts.ts). `plain-questions`: as `normal` but harbor-sim's queue holds an agent's
 * question with a gist, why it matters, options and a recommendation, and a person's without them, both
 * on C-0001 (docs/features/plain-questions.md "Acceptance criteria", PQ14; src/mocks/plainQuestions.ts).
 * `collision-pairs`: as `normal` but harbor-sim's spec holds a sprint and a stamina document and its queue
 * five `contradicts` discrepancies on them, four served a pair (the spec's example, a shared file, a gone
 * side, a grouping no looser match gives) and one of three targets served none; the example and the
 * three-target item on C-0001 (docs/features/collision-pairs.md "Acceptance criteria", PP9, PP11;
 * src/mocks/collisionPairs.ts). `hub` also serves the login's department, `foundry`, which is not the
 * first (docs/features/hub-department.md "Acceptance criteria", HD8; src/mocks/workspaces.ts).
 */
export const SCENARIOS = ["normal", "empty", "error", "slow", "conflict", "large", "cannot-check", "editorial", "code-cannot", "not-configured", "no-gaps", "config-notes", "aside", "aside40", "hub", "local-only", "plain-questions", "collision-pairs"] as const;

export type Scenario = (typeof SCENARIOS)[number];

/** Delay of every call in the `slow` scenario. */
export const SLOW_MS = 1500;

function isScenario(value: string): value is Scenario {
  return (SCENARIOS as readonly string[]).includes(value);
}

/** The scenario a location's query string names; `normal` when absent or unknown. */
export function scenarioFromSearch(search: string): Scenario {
  const value = new URLSearchParams(search).get("scenario");
  return value !== null && isScenario(value) ? value : "normal";
}
