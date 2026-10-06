import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { FindingSeverity } from "../api/types";
import { aCheckFinding, aCheckReport } from "../test/builders";
import { renderApp } from "../test/render";
import { stubClient } from "../test/stubClient";
import type * as LabelsModule from "./labels";

// A move in the findings re-renders the rows whose Tab stop changes, not every open row (review of
// ui-health, iteration 1: `large` holds 3 000). Each row asks for its severity's look once per
// render; the wrapper counts the asks and returns the real look.

const looks = vi.hoisted(() => ({ asked: 0 }));

vi.mock("./labels", async (importOriginal) => {
  const actual = await importOriginal<typeof LabelsModule>();
  return {
    ...actual,
    findingSeverityLook: (value: FindingSeverity) => {
      looks.asked += 1;
      return actual.findingSeverityLook(value);
    },
  };
});

describe("moving in the findings (memoised rows)", () => {
  it("re-renders two rows per move, whatever the number of open rows", async () => {
    const findings = ["a-code", "b-code", "c-code"].flatMap((code) =>
      Array.from({ length: 10 }, (_, index) => aCheckFinding({ code, path: `docs/${code}-${String(index).padStart(2, "0")}.md` })),
    );
    const client = stubClient();
    client.getCheck.mockImplementation(() => Promise.resolve(aCheckReport({ verdict: "observed", findings })));
    renderApp(client, "#/alpha/health");
    await screen.findByRole("region", { name: "Findings" });
    await waitFor(() => {
      expect(document.querySelectorAll("[data-finding]")).toHaveLength(30);
    });
    const rows = Array.from(document.querySelectorAll<HTMLElement>("[data-finding]"));
    const first = rows[0] as HTMLElement;
    first.focus();
    await waitFor(() => {
      expect(document.activeElement).toBe(first);
    });
    looks.asked = 0;
    fireEvent.keyDown(first, { key: "j" });
    await waitFor(() => {
      expect(document.activeElement).toBe(rows[1]);
    });
    expect(looks.asked).toBeGreaterThan(0);
    expect(looks.asked).toBeLessThanOrEqual(2);
  });
});
