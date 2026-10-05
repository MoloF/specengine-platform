import { describe, expect, it } from "vitest";
import { formatAge, formatUtc } from "./time";

const NOW = Date.parse("2026-10-05T12:00:00Z");

describe("times", () => {
  it.each([
    ["2026-10-05T11:59:30Z", "just now"],
    ["2026-10-05T11:48:00Z", "12 min ago"],
    ["2026-10-05T09:00:00Z", "3 h ago"],
    ["2026-10-03T12:00:00Z", "2 d ago"],
    ["not a time", "not a time"],
  ])("gives the age of %s as %s", (stored, age) => {
    expect(formatAge(stored, NOW)).toBe(age);
  });

  it("shows the stored time in UTC, raw when it does not parse", () => {
    expect(formatUtc("2026-10-05T21:14:03Z")).toBe("2026-10-05 21:14 UTC");
    expect(formatUtc("yesterday")).toBe("yesterday");
  });
});
