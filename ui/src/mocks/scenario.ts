/** The mock's scenarios, picked by `?scenario=` at bootstrap (`ui/README.md` "Owner's manual steps"). */
export const SCENARIOS = ["normal", "empty", "error", "slow", "conflict"] as const;

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
