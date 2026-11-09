import type { RoadmapDocument, RoadmapEstimate, RoadmapGroup, RoadmapSum } from "../api/types";
import type { Translate } from "../i18n";
import type { Look } from "../ui/Badge";

// The Roadmap's words (docs/features/roadmap-screen.md): the plan's lines in the form `spec roadmap`
// prints them (`docs/canon/spec-roadmap.md` "Command"), presentation only, their words the catalog's
// and their IDs, numbers and `-` as the CLI writes them (docs/canon/ui-languages.md "Converted").
// Release and priority names, the unit and an item's status come from the document and are never
// spelled here (ADR-0008). Nothing resolves at module load: each function takes the shown `t`.

/** A value on one line, as the CLI's one-line rule writes it: CR LF, LF and CR each a space. */
export function oneLine(text: string): string {
  return text.replaceAll("\r\n", " ").replace(/[\r\n]/g, " ");
}

/** The CLI's `<sum>`: `<min>-<max>` (`<min>` when equal), then ` <unit>` when set, then `, <u> unestimated` when some are. */
export function sumText(sum: RoadmapSum, unit: string | null, t: Translate): string {
  const bounds = sum.min === sum.max ? String(sum.min) : `${String(sum.min)}-${String(sum.max)}`;
  const withUnit = unit === null ? bounds : `${bounds} ${oneLine(unit)}`;
  return sum.unestimated > 0 ? t("roadmap.sumUnestimated", { sum: withUnit, count: sum.unestimated }) : withUnit;
}

/** An item's estimate as the CLI prints it, `N` or `N-M`, then the unit when set. */
export function estimateText(estimate: RoadmapEstimate, unit: string | null): string {
  const bounds = estimate.min === estimate.max ? String(estimate.min) : `${String(estimate.min)}-${String(estimate.max)}`;
  return unit === null ? bounds : `${bounds} ${oneLine(unit)}`;
}

/** `<o> open, <d> done`: two counts on one line, each its own plural group. */
function openDone(open: number, done: number, t: Translate): string {
  return t("roadmap.counts", { open: t("roadmap.open", { count: open }), done: t("roadmap.done", { count: done }) });
}

/** The header line: `spec roadmap`'s first line less `roadmap: `. */
export function headerLine(plan: RoadmapDocument, t: Translate): string {
  const counts = openDone(plan.counts.open, plan.counts.done, t);
  const current = plan.current === null ? t("roadmap.none") : oneLine(plan.current);
  const total = sumText(plan.total, plan.unit, t);
  if (plan.cut === null) {
    return t("roadmap.header", { counts, current, total });
  }
  return t("roadmap.headerCut", { counts, current, release: oneLine(plan.cut.release), cut: sumText(plan.cut, plan.unit, t), total });
}

/** `<o> open, <d> done` of a group. */
export function countsText(group: RoadmapGroup, t: Translate): string {
  return openDone(group.open, group.done, t);
}

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

/** A lane's name: a release's own, verbatim on one line; Release, Pool, Unplaced in the shown language; another kind raw. */
export function laneName(group: RoadmapGroup, t: Translate): string {
  if (isRelease(group)) {
    return group.name === null ? t("roadmap.group.release") : oneLine(group.name);
  }
  if (isPool(group)) {
    return t("roadmap.group.pool");
  }
  return isUnplaced(group) ? t("roadmap.group.unplaced") : oneLine(group.group);
}

/** The marks of the screen: a label, a colour role of tokens.css and an icon, never colour alone. The current release: */
export function currentLook(t: Translate): Look {
  return { label: t("roadmap.mark.current"), tone: "roadmap-current", icon: "current" };
}

/** The cut's mark: in a lane's head and on its rule. */
export function mvpCutLook(t: Translate): Look {
  return { label: t("roadmap.mark.mvpCut"), tone: "roadmap-cut", icon: "cutLine" };
}

/** An item in a waits_on cycle. */
export function inACycleLook(t: Translate): Look {
  return { label: t("roadmap.mark.inACycle"), tone: "roadmap-cycle", icon: "parentCycle" };
}

/** What an item waits on. */
export function waitsOnLook(t: Translate): Look {
  return { label: t("roadmap.mark.waitsOn"), tone: "roadmap-waits", icon: "waiting" };
}

/** An item with no estimate. */
export function unestimatedLook(t: Translate): Look {
  return { label: t("roadmap.mark.unestimated"), tone: "roadmap-unestimated", icon: "unknown" };
}

/** An item's status, raw, in a neutral badge: `status <raw>`, `status -` when absent. */
export function itemStatusLook(status: string | null, t: Translate): Look {
  return { label: t("roadmap.status", { status: status === null ? "-" : oneLine(status) }), tone: "roadmap-status", icon: "info" };
}

/** An item's priority, raw: `priority <raw>`, `priority -` when absent. */
export function priorityText(priority: string | null, t: Translate): string {
  return t("roadmap.priority", { priority: priority === null ? "-" : oneLine(priority) });
}

/** An unplaced item's release: `release <as written>`, or `release not a string` when null. */
export function unplacedReleaseText(release: string | null, t: Translate): string {
  return release === null ? t("roadmap.releaseNotString") : t("roadmap.release", { release: oneLine(release) });
}

/** The cut rule's text. */
export function cutRuleText(plan: RoadmapDocument, t: Translate): string | null {
  return plan.cut === null ? null : t("roadmap.cutRule", { sum: sumText(plan.cut, plan.unit, t) });
}

/** The note for a route ID no group lists. */
export function unlistedText(id: string, t: Translate): string {
  return t("roadmap.unlisted", { id });
}

/** The line above the tabs for a plan with no open item: none at all, or every one done; null otherwise. */
export function emptyPlanText(plan: RoadmapDocument, t: Translate): string | null {
  if (plan.counts.open > 0) {
    return null;
  }
  if (plan.counts.done === 0) {
    return t("roadmap.noItems", { dir: oneLine(plan.dir), prefix: oneLine(plan.prefix) });
  }
  return t("roadmap.allDone", { count: plan.counts.done });
}

/** The canvas's cap text: `<k> of <n> items not drawn`. */
export function notDrawnText(hidden: number, total: number, t: Translate): string {
  return t("roadmap.notDrawn", { hidden, count: total });
}
