import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { isOverdue, localToday } from "./due";

// AC-06 of docs/features/board-ui.md: overdue goes by the viewer's calendar day, not UTC's. Half past
// eleven at night eight hours behind UTC is already tomorrow there, so a UTC date fails here.

const LATE_EVENING = Date.parse("2026-03-01T23:30:00-08:00");

const open = { accepted: false, cancelled: false };

describe("the Board's clock", () => {
  beforeEach(() => {
    vi.stubEnv("TZ", "America/Los_Angeles");
  });

  afterEach(() => {
    vi.unstubAllEnvs();
  });

  it("takes the viewer's local date, zero-padded, never UTC's", () => {
    expect(new Date(LATE_EVENING).getTimezoneOffset()).toBe(480);
    expect(localToday(LATE_EVENING)).toBe("2026-03-01");
    expect(localToday(Date.parse("2026-01-05T08:00:00Z"))).toBe("2026-01-05");
    expect(localToday(Date.parse("2026-01-05T07:59:00Z"))).toBe("2026-01-04");
  });

  it("at 23:30 on the first, a card due that day is not overdue, one due the day before is (AC-06)", () => {
    const today = localToday(LATE_EVENING);
    expect(isOverdue({ ...open, due: "2026-03-01" }, today)).toBe(false);
    expect(isOverdue({ ...open, due: "2026-02-28" }, today)).toBe(true);
    expect(isOverdue({ ...open, due: "2026-03-02" }, today)).toBe(false);
  });

  it("never calls an accepted or a cancelled card overdue, nor one with no date", () => {
    const today = localToday(LATE_EVENING);
    expect(isOverdue({ due: "2026-02-28", accepted: true, cancelled: false }, today)).toBe(false);
    expect(isOverdue({ due: "2026-02-28", accepted: false, cancelled: true }, today)).toBe(false);
    expect(isOverdue({ ...open, due: null }, today)).toBe(false);
  });
});
