import { afterAll, describe, expect, it, vi } from "vitest";
import { enT, ruT } from "../test/text";
import { civilOf, dayOf, dayWords, percent, placeOf, pointOf, stretchOf, TICK_GAP, tickLabel, ticksOf, TIMELINE_PAGE, type Tick } from "./timeline";

// Set before any Date is made (importing makes none), so a local-time slip shows: UTC+14, where
// 23:30 UTC is already the next day (docs/features/delivery-timeline.md "Acceptance criteria", DT7).
vi.stubEnv("TZ", "Pacific/Kiritimati");

// DT7 of docs/features/delivery-timeline.md "Acceptance criteria": a day is a stored value's first
// ten characters, a UTC date read by its digits, whatever the viewer's zone; the served ends place it
// at `(day - from) / (to - from)`; a stretch that ends before it starts is empty, never negative. The
// spec's timeline: from 2026-11-01 to 2026-11-20, C-0001 taken 2026-11-03, at 2/19.

afterAll(() => {
  vi.unstubAllEnvs();
});

/** A stored value's day, or a failure naming it. */
function day(stored: string): number {
  const found = dayOf(stored);
  if (found === null) {
    throw new Error(`${stored} names no day`);
  }
  return found;
}

const FROM = day("2026-11-01");
const TO = day("2026-11-20");

describe("a stored value's day (DT7)", () => {
  it("runs in a zone fourteen hours ahead of UTC: there 23:30 UTC is already the next day", () => {
    expect(new Date("2026-11-03T23:30:00Z").getTimezoneOffset()).toBe(-840);
    expect(new Date("2026-11-03T23:30:00Z").getDate()).toBe(4);
  });

  it("reads 2026-11-03T23:30:00Z as 2026-11-03, the day its first ten characters name, not the viewer's 4th (M: the day through new Date(...).getDate())", () => {
    expect(day("2026-11-03T23:30:00Z")).toBe(day("2026-11-03"));
    expect(day("2026-11-03T23:30:00Z")).not.toBe(day("2026-11-04"));
    expect(day("2026-11-03T00:00:00Z")).toBe(day("2026-11-03T23:59:59Z"));
    expect(civilOf(day("2026-11-03T23:30:00Z"))).toEqual({ year: 2026, month: 11, day: 3 });
  });

  it("counts days since 1970-01-01 as the core's unix_days does", () => {
    expect([day("1970-01-01"), day("1970-01-02"), day("2026-11-01"), day("2026-11-03"), day("2026-11-20")]).toEqual([0, 1, 20758, 20760, 20777]);
    expect(day("2024-03-01") - day("2024-02-28")).toBe(2);
    expect(day("2100-03-01") - day("2100-02-28")).toBe(1);
    expect(day("2000-03-01") - day("2000-02-28")).toBe(2);
  });

  it.each(["2026-02-29", "2026-02-30", "2026-13-01", "2026-00-10", "2026-11-00", "20261103", "", "soon"])("names no day for %j", (stored) => {
    expect(dayOf(stored)).toBeNull();
  });

  it("turns a day back into its date, every day of four years", () => {
    for (let at = day("2023-12-25"); at <= day("2028-01-05"); at += 1) {
      const { year, month, day: date } = civilOf(at);
      expect(dayOf(`${String(year)}-${String(month).padStart(2, "0")}-${String(date).padStart(2, "0")}`)).toBe(at);
    }
  });
});

describe("a day's place between the served ends (DT7)", () => {
  it("puts from at 0, to at 1, C-0001's take at 2/19", () => {
    expect(placeOf(FROM, FROM, TO)).toBe(0);
    expect(placeOf(TO, FROM, TO)).toBe(1);
    expect(placeOf(day("2026-11-03T09:00:00Z"), FROM, TO)).toBe(2 / 19);
    expect(percent(placeOf(day("2026-11-03T09:00:00Z"), FROM, TO))).toBe("10.53%");
  });

  it("puts every day at 0 when the ends are one day", () => {
    expect(placeOf(FROM, FROM, FROM)).toBe(0);
    expect(stretchOf(FROM, FROM, FROM, FROM)).toEqual({ start: 0, width: 0 });
  });

  it("draws C-0001's waiting from its creation to its take and its work to its accept", () => {
    expect(stretchOf(day("2026-11-01T09:00:00Z"), day("2026-11-03T09:00:00Z"), FROM, TO)).toEqual({ start: 0, width: 2 / 19 });
    const work = stretchOf(day("2026-11-03T09:00:00Z"), day("2026-11-15T09:00:00Z"), FROM, TO);
    expect(work.start).toBe(2 / 19);
    expect(work.width).toBeCloseTo(12 / 19, 12);
  });

  it("draws a take before its creation as an empty stretch at the creation, never a negative width (M: a negative width)", () => {
    // everyShapeTimeline's C-0007: made 2026-11-06, its take stored 2026-11-02.
    expect(stretchOf(day("2026-11-06T09:00:00Z"), day("2026-11-02T09:00:00Z"), FROM, TO)).toEqual({ start: 5 / 19, width: 0 });
    // A day asked before a creation: the open stretch empty too.
    expect(stretchOf(day("2026-11-06"), day("2026-11-01"), FROM, TO).width).toBe(0);
    for (let start = FROM - 3; start <= TO + 3; start += 1) {
      for (let end = FROM - 3; end <= TO + 3; end += 1) {
        const stretch = stretchOf(start, end, FROM, TO);
        expect([start, end, stretch.width >= 0, stretch.start >= 0, stretch.start + stretch.width <= 1]).toEqual([start, end, true, true, true]);
      }
    }
  });

  it("keeps a day past either end on the axis", () => {
    expect(pointOf(FROM - 4, FROM, TO)).toBe(0);
    expect(pointOf(TO + 4, FROM, TO)).toBe(1);
    expect(stretchOf(FROM - 4, TO + 4, FROM, TO)).toEqual({ start: 0, width: 1 });
  });
});

/** A tick as its date and label, for reading. */
function said(tick: Tick): [string, string] {
  const { year, month, day: date } = civilOf(tick.day);
  return [`${String(year).padStart(4, "0")}-${String(month).padStart(2, "0")}-${String(date).padStart(2, "0")}`, tick.label];
}

/** The least share of the axis between two neighbouring ticks, to a billionth (a gap of exactly a tenth reads as one); Infinity with fewer than two. */
function leastGap(ticks: readonly Tick[], from: number, to: number): number {
  const least = ticks.slice(1).reduce((low, tick, index) => Math.min(low, placeOf(tick.day, from, to) - placeOf(ticks[index]?.day ?? from, from, to)), Infinity);
  return Math.round(least * 1e9) / 1e9;
}

describe("the axis' ticks and words (DT7)", () => {
  it("ticks the month's first and, under 60 days, each Monday a tenth of the axis from it: 2026-11-01 a Sunday, the 2nd left out, then the 9th and 16th", () => {
    expect(ticksOf(FROM, TO).map(said)).toEqual([
      ["2026-11-01", "monthYear"],
      ["2026-11-09", "monday"],
      ["2026-11-16", "monday"],
    ]);
    // The 2nd stands 1/19 of the axis from the 1st, under a tenth: their labels would sit on one another.
    expect(placeOf(day("2026-11-02"), FROM, TO)).toBeLessThan(TICK_GAP);
  });

  it("ticks the months alone over 60 days and more, the year on the first and on January; a Monday that is a first once, as a month", () => {
    expect(ticksOf(day("2026-07-15"), day("2026-11-20")).map(said)).toEqual([
      ["2026-08-01", "monthYear"],
      ["2026-09-01", "month"],
      ["2026-10-01", "month"],
      ["2026-11-01", "month"],
    ]);
    expect(ticksOf(day("2026-10-01"), day("2027-03-10")).map(said)).toEqual([
      ["2026-10-01", "monthYear"],
      ["2026-11-01", "month"],
      ["2026-12-01", "month"],
      ["2027-01-01", "monthYear"],
      ["2027-02-01", "month"],
      ["2027-03-01", "month"],
    ]);
    // 2027-02-01 is a Monday: one tick, a month's; 2027-01-25 and 2027-02-08 a third of the axis from it.
    expect(ticksOf(day("2027-01-20"), day("2027-02-10")).map(said)).toEqual([
      ["2027-01-25", "monday"],
      ["2027-02-01", "monthYear"],
      ["2027-02-08", "monday"],
    ]);
  });

  it("leaves out a Monday a few days either side of a month's first over a long span: 2026-10-05 to 2026-12-01", () => {
    // 57 days: a tenth is 5.7 days. 2026-11-02 is a day after November's first, 2026-10-26 six before it.
    expect(ticksOf(day("2026-10-05"), day("2026-12-01")).map(said)).toEqual([
      ["2026-10-05", "monday"],
      ["2026-10-12", "monday"],
      ["2026-10-19", "monday"],
      ["2026-10-26", "monday"],
      ["2026-11-01", "monthYear"],
      ["2026-11-09", "monday"],
      ["2026-11-16", "monday"],
      ["2026-11-23", "monday"],
      ["2026-12-01", "month"],
    ]);
  });

  it("thins by the span: months, then quarters, then years, then round numbers of years", () => {
    // 280 days: a month a tenth of the axis, its fewest 28 days; nine months, ten labels.
    expect(ticksOf(day("2026-11-01"), day("2027-08-08")).map(said)).toHaveLength(10);
    // 281 days: quarters, January's year said.
    expect(ticksOf(day("2026-11-01"), day("2027-08-09")).map(said)).toEqual([
      ["2027-01-01", "monthYear"],
      ["2027-04-01", "month"],
      ["2027-07-01", "month"],
    ]);
    expect(ticksOf(day("2026-11-01"), day("2030-02-01")).map(said)).toEqual([
      ["2027-01-01", "year"],
      ["2028-01-01", "year"],
      ["2029-01-01", "year"],
      ["2030-01-01", "year"],
    ]);
    expect(ticksOf(day("2026-11-01"), day("2062-06-30")).map(said)).toEqual([
      ["2030-01-01", "year"],
      ["2035-01-01", "year"],
      ["2040-01-01", "year"],
      ["2045-01-01", "year"],
      ["2050-01-01", "year"],
      ["2055-01-01", "year"],
      ["2060-01-01", "year"],
    ]);
  });

  it("gives a due in 9999 a few labels, a thousand years apart, never tens of thousands (M: a tick on every month's first)", () => {
    const to = day("9999-12-31");
    const ticks = ticksOf(FROM, to);
    expect(ticks.map(said)).toEqual(["3000", "4000", "5000", "6000", "7000", "8000", "9000"].map((year) => [`${year}-01-01`, "year"]));
    expect(ticks.map((tick) => tickLabel(tick, "en"))).toEqual(["3000", "4000", "5000", "6000", "7000", "8000", "9000"]);
    expect(leastGap(ticks, FROM, to)).toBeGreaterThanOrEqual(TICK_GAP);
  });

  it("keeps every span's labels to about twelve, in day order, between the ends, never two closer than a tenth of the axis (M: a Monday beside a month's first)", () => {
    const starts = ["2026-11-01", "2026-11-02", "2026-11-15", "2027-01-31", "2028-02-29", "0001-01-01"].map(day);
    const lengths = [0, 1, 6, 7, 10, 19, 27, 28, 45, 57, 59, 60, 61, 90, 120, 280, 281, 300, 365, 400, 900, 901, 1000, 3650, 3651, 7300, 20_000, 36_500, 100_000, 1_000_000];
    for (const from of starts) {
      for (const to of [...lengths.map((length) => from + length), day("9999-12-31")]) {
        const ticks = ticksOf(from, to);
        const days = ticks.map((tick) => tick.day);
        const shape = [ticks.length <= 12, days.every((at, index) => index === 0 || at > (days[index - 1] ?? at)), days.every((at) => at >= from && at <= to), leastGap(ticks, from, to) >= TICK_GAP];
        expect([from, to, ...shape]).toEqual([from, to, true, true, true, true]);
      }
    }
  });

  it("says a day in the viewer's language by its UTC date, wherever the viewer is", () => {
    expect(dayWords("2026-11-03T23:30:00Z", "en", enT)).toBe("3 Nov 2026");
    expect(dayWords("2026-11-10", "en", enT)).toBe("10 Nov 2026");
    expect(dayWords("2026-11-03T23:30:00Z", "ru", ruT)).toMatch(/^3 \S+ 2026$/);
    expect(dayWords("soon", "en", enT)).toBe("soon");
    expect(tickLabel({ day: FROM, label: "monthYear" }, "en")).toBe("Nov 2026");
    expect(tickLabel({ day: day("2026-12-01"), label: "month" }, "en")).toBe("Dec");
    expect(tickLabel({ day: day("2030-01-01"), label: "year" }, "en")).toBe("2030");
    expect(tickLabel({ day: day("2026-11-09"), label: "monday" }, "en")).toBe("9");
    expect(tickLabel({ day: FROM, label: "monthYear" }, "ru")).toMatch(/^\S+ 2026/);
  });

  it("pages at 200 rows (DT9)", () => {
    expect(TIMELINE_PAGE).toBe(200);
  });
});
