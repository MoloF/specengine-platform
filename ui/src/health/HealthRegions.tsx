import type { CheckQuery, HealthQuery } from "../api/queries";
import { CheckRegion } from "./CheckRegion";
import { DebtRegion } from "./DebtRegion";
import { FindingsRegion } from "./FindingsRegion";
import { LeftRegion } from "./LeftRegion";

/**
 * Health's four regions, a chunk of their own (the main one stays under Vite's 500 kB warning):
 * Check, What is left, Findings, Debt and budgets. The first, third and fourth share the one check
 * read; What is left reads the inbox and shows the health read (its unapplied amendments), which
 * Check's "Check again" reads again with the check.
 */
export function HealthRegions({ project, check, health }: { project: string; check: CheckQuery; health: HealthQuery }) {
  return (
    <div className="health-regions">
      <CheckRegion project={project} check={check} health={health} />
      <LeftRegion project={project} health={health} />
      <FindingsRegion project={project} check={check} />
      <DebtRegion project={project} check={check} />
    </div>
  );
}
