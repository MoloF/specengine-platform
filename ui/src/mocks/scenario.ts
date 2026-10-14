/** The mock's scenarios, picked by `?scenario=` at bootstrap (`ui/README.md` "Owner's manual steps"). */
/**
 * `cannot-check`: every read as `normal` but the check, which cannot vouch for the corpus
 * (`docs/features/ui-health.md` "Data"), and the unapplied amendments, refused 503 for the same
 * incomplete walk (`docs/features/health-left.md` "Data"). `editorial`: every read as `normal` but
 * the lock entries an editorial approval refreshes, harbor-sim's counted and ledger-api's lock
 * unreadable, and a wording fix with an editorial approve staged (`docs/features/rev-discipline.md`
 * "UI"; src/mocks/editorial.ts).
 */
export const SCENARIOS = ["normal", "empty", "error", "slow", "conflict", "large", "cannot-check", "editorial"] as const;

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
