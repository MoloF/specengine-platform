import type { DeliveryDepartment, DeliveryItem, DeliveryLate, DeliveryRelease, LeadDays } from "../api/types";
import type { Translate } from "../i18n";
import type { LozengeLook } from "../ui/Lozenge";
import { oneLine } from "./text";

// The Delivery tab's words (docs/features/delivery-analytics.md "Acceptance criteria", DA-12): the
// one place a delivery group's word and an item's kind are told apart, each looked up in the
// catalog; a word this build does not know is shown raw, as the daemon sent it (`served`). A release
// name and a department's title are the project's, shown as written, never compared. Nothing is
// counted here: every count, day and lead time is the daemon's.

/** A label and whether it is the daemon's own word, tagged `lang="en"` where it is shown. */
export interface DeliveryWord {
  text: string;
  served: boolean;
}

/** A row's group: a release by its name, the pool, the unplaced items, no release; another word raw. */
export function groupLabel({ group, release }: Pick<DeliveryRelease, "group" | "release">, t: Translate): DeliveryWord {
  switch (group) {
    case "release":
      return { text: release === null ? t("ui.emptyValue") : oneLine(release), served: false };
    case "pool":
      return { text: t("roadmap.delivery.group.pool"), served: false };
    case "unplaced":
      return { text: t("roadmap.delivery.group.unplaced"), served: false };
    case "none":
      return { text: t("roadmap.delivery.group.none"), served: false };
    default:
      return { text: oneLine(group), served: true };
  }
}

/** A question, discrepancy or decision by its word; another kind raw. */
export function itemKindLabel({ kind }: Pick<DeliveryItem, "kind">, t: Translate): DeliveryWord {
  switch (kind) {
    case "question":
      return { text: t("roadmap.delivery.item.question"), served: false };
    case "discrepancy":
      return { text: t("roadmap.delivery.item.discrepancy"), served: false };
    case "decision":
      return { text: t("roadmap.delivery.item.decision"), served: false };
    default:
      return { text: oneLine(kind), served: true };
  }
}

/** A row's lead time as served: its median and its most, or none delivered. */
export function leadText(lead: LeadDays | null, t: Translate): string {
  return lead === null ? t("roadmap.delivery.noLead") : t("roadmap.delivery.lead", { median: String(lead.median), max: String(lead.max) });
}

/** A late task's mark, words and an icon, never colour alone: overdue while open, delivered late once accepted, by the served days. */
export function lateLook({ accepted_at, days_late }: Pick<DeliveryLate, "accepted_at" | "days_late">, t: Translate): LozengeLook {
  return accepted_at === null
    ? { appearance: "danger", icon: "cardOverdue", label: t("roadmap.delivery.overdueBy", { count: days_late }) }
    : { appearance: "warning", icon: "taskAccepted", label: t("roadmap.delivery.lateBy", { count: days_late }) };
}

/** A late task's departments by their served titles, in served order; a name no row holds as written. */
export function departmentTitles(names: readonly string[], departments: readonly DeliveryDepartment[]): string[] {
  const titles = new Map(departments.map((department) => [department.name, department.title]));
  return names.map((name) => titles.get(name) ?? name);
}
