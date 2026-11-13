import type { Translate } from "../i18n";

// The Delivery timeline's day arithmetic (docs/features/delivery-timeline.md "Acceptance criteria",
// DT7): a day is a stored value's first ten characters, a UTC calendar date read as the core's
// `unix_days` reads it, by its digits alone; never a local date, never a clock. The axis' ends are
// served: a day sits at `(day - from) / (to - from)` along it. Nothing here judges a task late, a
// take or a release: those are served too.

/** The rows the view mounts at first, and each "Show more" adds as many (DT9). */
export const TIMELINE_PAGE = 200;

/** How many days apart the axis' ends stand before its Mondays are left out: the month ticks alone. */
const MONDAYS_UNDER = 60;

/** How many shares of the axis a labelled tick takes at least: a tick is a tenth of the axis from the next, never closer. */
const TICK_SPACING = 10;

/** The least share of the axis between two labelled ticks: at most eleven labels, whatever the span, `due` 9999 too. */
export const TICK_GAP = 1 / TICK_SPACING;

/** The fewest days a step of months spans: a month 28, a quarter 90 (January to April), a year 365. */
const MONTH_LEAST = 28;
const QUARTER_LEAST = 90;
const YEAR_LEAST = 365;

/** Day 0, 1970-01-01, was a Thursday: a day is a Monday when `(day + 3) % 7` is 0. */
const MONDAY_SHIFT = 3;

const DAY_MS = 86_400_000;

/** Whether `year` is a leap year in the proleptic Gregorian calendar. */
function leap(year: number): boolean {
  return (year % 4 === 0 && year % 100 !== 0) || year % 400 === 0;
}

/** How many days `month` (1 to 12) of `year` holds. */
function daysIn(year: number, month: number): number {
  return month === 2 ? (leap(year) ? 29 : 28) : [4, 6, 9, 11].includes(month) ? 30 : 31;
}

/** Days since 1970-01-01 of a civil date (Hinnant's `days_from_civil`), by integer arithmetic alone. */
function civilDays(year: number, month: number, day: number): number {
  const y = month <= 2 ? year - 1 : year;
  const era = Math.floor(y / 400);
  const yearOfEra = y - era * 400;
  const dayOfYear = Math.floor((153 * (month + (month > 2 ? -3 : 9)) + 2) / 5) + day - 1;
  const dayOfEra = yearOfEra * 365 + Math.floor(yearOfEra / 4) - Math.floor(yearOfEra / 100) + dayOfYear;
  return era * 146_097 + dayOfEra - 719_468;
}

/** The civil date of a day number (Hinnant's `civil_from_days`). */
export function civilOf(days: number): { year: number; month: number; day: number } {
  const z = days + 719_468;
  const era = Math.floor(z / 146_097);
  const dayOfEra = z - era * 146_097;
  const yearOfEra = Math.floor((dayOfEra - Math.floor(dayOfEra / 1460) + Math.floor(dayOfEra / 36_524) - Math.floor(dayOfEra / 146_096)) / 365);
  const dayOfYear = dayOfEra - (365 * yearOfEra + Math.floor(yearOfEra / 4) - Math.floor(yearOfEra / 100));
  const shifted = Math.floor((5 * dayOfYear + 2) / 153);
  const day = dayOfYear - Math.floor((153 * shifted + 2) / 5) + 1;
  const month = shifted < 10 ? shifted + 3 : shifted - 9;
  return { year: yearOfEra + era * 400 + (month <= 2 ? 1 : 0), month, day };
}

/**
 * A stored value's day: its first ten characters, `YYYY-MM-DD`, as days since 1970-01-01; null when
 * they name no calendar date. A timestamp's time and zone are never read: `2026-11-03T23:30:00Z` is
 * 2026-11-03 wherever the viewer is.
 */
export function dayOf(stored: string): number | null {
  const match = /^(\d{4})-(\d{2})-(\d{2})/.exec(stored);
  if (match === null) {
    return null;
  }
  const [year, month, day] = [Number(match[1]), Number(match[2]), Number(match[3])];
  if (month < 1 || month > 12 || day < 1 || day > daysIn(year, month)) {
    return null;
  }
  return civilDays(year, month, day);
}

/** Where a day stands between the axis' ends, 0 at `from` and 1 at `to`; 0 when they are one day. */
export function placeOf(day: number, from: number, to: number): number {
  return to === from ? 0 : (day - from) / (to - from);
}

/** A place kept on the axis. */
function clamped(place: number): number {
  return Math.min(1, Math.max(0, place));
}

/** A stretch of the axis: where it starts and how wide it is, each a share of the axis, never negative. */
export interface Stretch {
  start: number;
  width: number;
}

/**
 * The stretch from one day to another, kept on the axis; a stretch that ends before it starts (a take
 * stored before its creation, a day asked before it) is empty, drawn at its start.
 */
export function stretchOf(start: number, end: number, from: number, to: number): Stretch {
  const begins = clamped(placeOf(start, from, to));
  return { start: begins, width: Math.max(0, clamped(placeOf(end, from, to)) - begins) };
}

/** A point on the axis, kept on it. */
export function pointOf(day: number, from: number, to: number): number {
  return clamped(placeOf(day, from, to));
}

/** A share of the axis as a CSS percentage, to a hundredth of a percent. */
export function percent(share: number): string {
  return `${String(Math.round(share * 10_000) / 100)}%`;
}

/**
 * What an axis tick's label says: a Monday's day number; a month's name; a month with its year, on
 * the first month labelled and on each January; a year alone, on a step of a year or more.
 */
export type TickLabel = "monday" | "month" | "monthYear" | "year";

/** An axis tick: its day, and what its label says. */
export interface Tick {
  day: number;
  label: TickLabel;
}

/**
 * How many months apart the labelled ticks stand over `span` days: the first step whose fewest days
 * keep a tick a tenth of the axis from the next, a month, a quarter, then a year, 2, 5, 10, 20, 50
 * years and on, so a far `due` gives a few labels, never thousands.
 */
function stepOf(span: number): number {
  if (MONTH_LEAST * TICK_SPACING >= span) {
    return 1;
  }
  if (QUARTER_LEAST * TICK_SPACING >= span) {
    return 3;
  }
  for (let years = 1; ; years *= 10) {
    for (const times of [1, 2, 5]) {
      if (YEAR_LEAST * years * times * TICK_SPACING >= span) {
        return 12 * years * times;
      }
    }
  }
}

/**
 * The axis' ticks, in day order, a tenth of the axis apart at least (`TICK_GAP`): each step's first
 * between the ends, its step a month, a quarter or a round number of years by the span; and, when
 * the ends stand under 60 days apart, each Monday not within a tenth of the axis of a month's first,
 * so a month's label and a Monday's never sit on one another.
 */
export function ticksOf(from: number, to: number): Tick[] {
  if (to < from) {
    return [];
  }
  const span = to - from;
  const step = stepOf(span);
  const start = civilOf(from);
  // Months since year 0 of the first month's first on or after `from`, then up to the step's first.
  const after = start.year * 12 + start.month - 1 + (start.day === 1 ? 0 : 1);
  const firsts: Tick[] = [];
  for (let index = Math.ceil(after / step) * step; ; index += step) {
    const year = Math.floor(index / 12);
    const month = index - year * 12 + 1;
    const first = civilDays(year, month, 1);
    if (first > to) {
      break;
    }
    firsts.push({ day: first, label: step >= 12 ? "year" : firsts.length === 0 || month === 1 ? "monthYear" : "month" });
  }
  if (span >= MONDAYS_UNDER) {
    return firsts;
  }
  const weekday = (((from + MONDAY_SHIFT) % 7) + 7) % 7;
  const mondays: Tick[] = [];
  for (let day = from + ((7 - weekday) % 7); day <= to; day += 7) {
    if (civilOf(day).day !== 1 && firsts.every((first) => Math.abs(day - first.day) * TICK_SPACING >= span)) {
      mondays.push({ day, label: "monday" });
    }
  }
  return [...firsts, ...mondays].sort((a, b) => a.day - b.day);
}

/** Each form a day is said in: in words; a tick's labels. */
const FORM_OPTIONS = {
  day: { day: "numeric", month: "short", year: "numeric" },
  monthYear: { month: "short", year: "numeric" },
  month: { month: "short" },
  monday: { day: "numeric" },
  year: { year: "numeric" },
} satisfies Record<TickLabel | "day", Intl.DateTimeFormatOptions>;

/** One formatter per language and form, made once: the day's parts in the viewer's language, at UTC. */
const FORMATS = new Map<string, Intl.DateTimeFormat>();

function formatOf(language: string, form: keyof typeof FORM_OPTIONS): Intl.DateTimeFormat {
  const key = `${language} ${form}`;
  let format = FORMATS.get(key);
  if (format === undefined) {
    format = new Intl.DateTimeFormat(language, { timeZone: "UTC", ...FORM_OPTIONS[form] });
    FORMATS.set(key, format);
  }
  return format;
}

/** A day number as the instant its UTC day starts, for `Intl` alone. */
function startOf(day: number): Date {
  return new Date(day * DAY_MS);
}

/** A day in words in the viewer's language, `3 Nov 2026` in English, its order the catalog's; a stored value that names no day as it is. */
export function dayWords(stored: string, language: string, t: Translate): string {
  const day = dayOf(stored);
  if (day === null) {
    return stored;
  }
  const parts = new Map(formatOf(language, "day").formatToParts(startOf(day)).map((part) => [part.type, part.value]));
  return t("roadmap.delivery.timeline.day", { day: parts.get("day") ?? "", month: parts.get("month") ?? "", year: parts.get("year") ?? "" });
}

/** A tick's label in the viewer's language: `Nov 2026`, `Dec`, `2030` or a Monday's `9`, as its label says. */
export function tickLabel(tick: Tick, language: string): string {
  return formatOf(language, tick.label).format(startOf(tick.day));
}
