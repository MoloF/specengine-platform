import type { RoadmapDocument, RoadmapEstimate, RoadmapGroup, RoadmapSum } from "../api/types";
import type { Look } from "../ui/Badge";

// The Roadmap's words (docs/features/roadmap-screen.md): the plan's lines as `spec roadmap` prints
// them (`docs/canon/spec-roadmap.md` "Command"), presentation only. Release and priority names,
// the unit and an item's status come from the document and are never spelled here (ADR-0008).

/** A value on one line, as the CLI's one-line rule writes it: CR LF, LF and CR each a space. */
export function oneLine(text: string): string {
  return text.replaceAll("\r\n", " ").replace(/[\r\n]/g, " ");
}

/** The CLI's `<sum>`: `<min>-<max>` (`<min>` when equal), then ` <unit>` when set, then `, <u> unestimated` when some are. */
export function sumText(sum: RoadmapSum, unit: string | null): string {
  const bounds = sum.min === sum.max ? String(sum.min) : `${String(sum.min)}-${String(sum.max)}`;
  const withUnit = unit === null ? bounds : `${bounds} ${oneLine(unit)}`;
  return sum.unestimated > 0 ? `${withUnit}, ${String(sum.unestimated)} unestimated` : withUnit;
}

/** An item's estimate as the CLI prints it, `N` or `N-M`, then the unit when set. */
export function estimateText(estimate: RoadmapEstimate, unit: string | null): string {
  const bounds = estimate.min === estimate.max ? String(estimate.min) : `${String(estimate.min)}-${String(estimate.max)}`;
  return unit === null ? bounds : `${bounds} ${oneLine(unit)}`;
}

/** The header line: `spec roadmap`'s first line less `roadmap: `. */
export function headerLine(plan: RoadmapDocument): string {
  const current = plan.current === null ? "none" : oneLine(plan.current);
  const cut = plan.cut === null ? "" : `; to ${oneLine(plan.cut.release)} ${sumText(plan.cut, plan.unit)}`;
  return `${String(plan.counts.open)} open, ${String(plan.counts.done)} done; current ${current}${cut}; total ${sumText(plan.total, plan.unit)}`;
}

/** `<o> open, <d> done` of a group. */
export function countsText(group: RoadmapGroup): string {
  return `${String(group.open)} open, ${String(group.done)} done`;
}

/** The three group kinds' names; another kind is shown raw. */
const GROUP_NAMES: Readonly<Record<string, string>> = { release: "Release", pool: "Pool", unplaced: "Unplaced" };

/** Whether the group is a release: a position per item, a bar, a name of its own. */
export function isRelease(group: RoadmapGroup): boolean {
  return group.group === "release";
}

/** Whether the group is the pool: its pool tasks follow its items. */
export function isPool(group: RoadmapGroup): boolean {
  return group.group === "pool";
}

/** Whether the group holds the unplaced items: each shows its release as written. */
export function isUnplaced(group: RoadmapGroup): boolean {
  return group.group === "unplaced";
}

/** A lane's name: a release's own, verbatim on one line; Pool; Unplaced; another kind raw. */
export function laneName(group: RoadmapGroup): string {
  if (isRelease(group) && group.name !== null) {
    return oneLine(group.name);
  }
  return Object.hasOwn(GROUP_NAMES, group.group) ? (GROUP_NAMES[group.group] ?? group.group) : oneLine(group.group);
}

/** The marks of the screen: a label, a colour role of tokens.css and an icon, never colour alone. */
export const CURRENT: Look = { label: "Current", tone: "roadmap-current", icon: "current" };
export const MVP_CUT: Look = { label: "MVP cut", tone: "roadmap-cut", icon: "cutLine" };
export const IN_A_CYCLE: Look = { label: "In a cycle", tone: "roadmap-cycle", icon: "parentCycle" };
export const WAITS_ON: Look = { label: "Waits on", tone: "roadmap-waits", icon: "waiting" };
export const UNESTIMATED: Look = { label: "unestimated", tone: "roadmap-unestimated", icon: "unknown" };

/** An item's status, raw, in a neutral badge: `status <raw>`, `status -` when absent. */
export function itemStatusLook(status: string | null): Look {
  return { label: `status ${status === null ? "-" : oneLine(status)}`, tone: "roadmap-status", icon: "info" };
}

/** An item's priority, raw: `priority <raw>`, `priority -` when absent. */
export function priorityText(priority: string | null): string {
  return `priority ${priority === null ? "-" : oneLine(priority)}`;
}

/** An unplaced item's release: `release <as written>`, or `release not a string` when null. */
export function unplacedReleaseText(release: string | null): string {
  return release === null ? "release not a string" : `release ${oneLine(release)}`;
}

/** The cut rule's text. */
export function cutRuleText(plan: RoadmapDocument): string | null {
  return plan.cut === null ? null : `MVP cut: ${sumText(plan.cut, plan.unit)}`;
}

/** The note for a route ID no group lists. */
export function unlistedText(id: string): string {
  return `${id} is not an open item; done items are counted, not listed`;
}

/** The line above the tabs for a plan with no open item: none at all, or every one done; null otherwise. */
export function emptyPlanText(plan: RoadmapDocument): string | null {
  if (plan.counts.open > 0) {
    return null;
  }
  if (plan.counts.done === 0) {
    return `No plan items yet: an item is a file in ${oneLine(plan.dir)} whose id starts with ${oneLine(plan.prefix)}-`;
  }
  return `Every item is done (${String(plan.counts.done)})`;
}

/** The fixed line under every failure of the read: no matching on the daemon's words. */
export const NO_PLAN_LINE = "A project has a plan once its specengine.toml has a [roadmap] table";

/** The canvas's description: the Lanes tab is its text twin. */
export const CANVAS_DESCRIPTION = "The Lanes tab holds every item, wait and mark as text.";

/** The canvas's cap text: `<k> of <n> items not drawn`. */
export function notDrawnText(hidden: number, total: number): string {
  return `${String(hidden)} of ${String(total)} items not drawn`;
}
