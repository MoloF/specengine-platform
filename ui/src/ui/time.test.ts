import { describe, expect, it } from "vitest";
import { formatAge, formatUtc } from "./time";
import { ru } from "../i18n/ru";
import { enT, ruT } from "../test/text";

const NOW = Date.parse("2026-10-05T12:00:00Z");

describe("times", () => {
  it.each([
    ["2026-10-05T11:59:30Z", "just now"],
    ["2026-10-05T11:48:00Z", "12 min ago"],
    ["2026-10-05T09:00:00Z", "3 h ago"],
    ["2026-10-03T12:00:00Z", "2 d ago"],
    ["not a time", "not a time"],
  ])("gives the age of %s as %s", (stored, age) => {
    expect(formatAge(stored, NOW, enT)).toBe(age);
  });

  it("shows the stored time in UTC, raw when it does not parse", () => {
    expect(formatUtc("2026-10-05T21:14:03Z", enT)).toBe("2026-10-05 21:14 UTC");
    expect(formatUtc("yesterday", enT)).toBe("yesterday");
  });
});

// LG-4 of docs/features/ui-languages.md, dates and ages: a stamp's digits from Intl, its order from
// the catalog; an age's words in the plural form Intl.PluralRules names.
describe("times in Russian (LG-4)", () => {
  const filled = (value: string, count: number) => value.replace("{{count}}", String(count));

  it("fills the Russian stamp, day first, the hour 00 at midnight, never 24", () => {
    expect(formatUtc("2026-10-05T21:14:03Z", ruT)).toBe("05.10.2026 21:14 UTC");
    expect(formatUtc("2026-10-05T21:14:03Z", ruT)).toBe(
      ru.ui.time.stamp.replace("{{day}}", "05").replace("{{month}}", "10").replace("{{year}}", "2026").replace("{{hour}}", "21").replace("{{minute}}", "14"),
    );
    expect(formatUtc("2026-10-05T00:05:00Z", enT)).toBe("2026-10-05 00:05 UTC");
    expect(formatUtc("2026-10-05T00:05:00Z", ruT)).toBe("05.10.2026 00:05 UTC");
  });

  it("is the same instant whatever the page's own time zone: UTC", () => {
    expect(formatUtc("2026-10-05T23:59:59+02:00", enT)).toBe("2026-10-05 21:59 UTC");
  });

  it.each([
    [1, "minutesAgo_one"],
    [3, "minutesAgo_few"],
    [5, "minutesAgo_many"],
    [21, "minutesAgo_one"],
  ] as const)("says %i minutes ago in its form (%s)", (minutes, form) => {
    expect(formatAge(new Date(NOW - minutes * 60_000).toISOString(), NOW, ruT)).toBe(filled(ru.ui.time[form], minutes));
  });

  it("says just now, hours and days in Russian, and shows an unparsable value raw", () => {
    expect(formatAge("2026-10-05T11:59:30Z", NOW, ruT)).toBe(ru.ui.time.justNow);
    expect(formatAge("2026-10-05T09:00:00Z", NOW, ruT)).toBe(filled(ru.ui.time.hoursAgo_few, 3));
    expect(formatAge("2026-10-03T12:00:00Z", NOW, ruT)).toBe(filled(ru.ui.time.daysAgo_few, 2));
    expect(formatAge("not a time", NOW, ruT)).toBe("not a time");
    expect(formatUtc("yesterday", ruT)).toBe("yesterday");
  });
});
