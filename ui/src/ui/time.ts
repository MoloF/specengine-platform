import type { Translate } from "../i18n";

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

/** How long ago a stored UTC time was, e.g. `12 min ago`, in the catalog's words; the raw text when it does not parse. */
export function formatAge(stored: string, now: number, t: Translate): string {
  const at = Date.parse(stored);
  if (Number.isNaN(at)) {
    return stored;
  }
  const elapsed = Math.max(0, now - at);
  if (elapsed < MINUTE) {
    return t("ui.time.justNow");
  }
  if (elapsed < HOUR) {
    return t("ui.time.minutesAgo", { count: Math.floor(elapsed / MINUTE) });
  }
  if (elapsed < DAY) {
    return t("ui.time.hoursAgo", { count: Math.floor(elapsed / HOUR) });
  }
  return t("ui.time.daysAgo", { count: Math.floor(elapsed / DAY) });
}

/**
 * A stamp's digits, the same in every language (docs/features/ui-languages.md "Plurals and dates"):
 * `h23`, never `hour12: false`, which gives `24` at midnight on some engines.
 */
const STAMP_PARTS = new Intl.DateTimeFormat("en-US", {
  timeZone: "UTC",
  year: "numeric",
  month: "2-digit",
  day: "2-digit",
  hour: "2-digit",
  minute: "2-digit",
  hourCycle: "h23",
});

/** A stored time in UTC, minutes precision, its order the catalog's (`2026-10-05 21:14 UTC` in English); raw when it does not parse. */
export function formatUtc(stored: string, t: Translate): string {
  const at = Date.parse(stored);
  if (Number.isNaN(at)) {
    return stored;
  }
  const parts = new Map(STAMP_PARTS.formatToParts(at).map((part) => [part.type, part.value]));
  const part = (type: Intl.DateTimeFormatPartTypes) => parts.get(type) ?? "";
  return t("ui.time.stamp", { year: part("year"), month: part("month"), day: part("day"), hour: part("hour"), minute: part("minute") });
}
