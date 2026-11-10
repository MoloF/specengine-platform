import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { PropertyList, type Property } from "./PropertyList";

// DF-6 of docs/features/design-foundation.md: a `dl` of label and value rows in the caller's order;
// with `foldEmpty` an empty row leaves the list for one collapsed "More fields (n)", each reading None.

const ITEMS: Property[] = [
  { key: "a", label: "Assignee", value: null },
  { key: "r", label: "Reviewer", value: "Ivo Brandt" },
  { key: "f", label: "Flow", value: "copy" },
  { key: "d", label: "Due", value: null },
  { key: "e", label: "Estimate", value: null },
];

function terms(scope: HTMLElement): string[] {
  return within(scope)
    .queryAllByRole("term")
    .map((term) => term.textContent);
}

describe("PropertyList (DF-6)", () => {
  it("lists every row in order, an empty value reading None, without foldEmpty", () => {
    render(<PropertyList label="Details" items={ITEMS} />);
    const list = screen.getByRole("region", { name: "Details" });
    expect(terms(list)).toEqual(["Assignee", "Reviewer", "Flow", "Due", "Estimate"]);
    expect(within(list).getAllByRole("definition").map((value) => value.textContent)).toEqual(["None", "Ivo Brandt", "copy", "None", "None"]);
    expect(within(list).queryByRole("button")).toBeNull();
  });

  it("with foldEmpty, folds the empty rows, in order, under one collapsed More fields (n), each None", () => {
    render(<PropertyList label="Details" items={ITEMS} foldEmpty />);
    const list = screen.getByRole("region", { name: "Details" });
    expect(terms(list)).toEqual(["Reviewer", "Flow"]);
    const more = within(list).getByRole("button", { name: "More fields (3)" });
    expect(more.getAttribute("aria-expanded")).toBe("false");
    more.focus();
    fireEvent.click(more);
    expect(more.getAttribute("aria-expanded")).toBe("true");
    // Focus stays on the button (the disclosure helper's rule).
    expect(document.activeElement).toBe(more);
    const region = document.getElementById(more.getAttribute("aria-controls") ?? "") as HTMLElement;
    expect(terms(region)).toEqual(["Assignee", "Due", "Estimate"]);
    expect(within(region).getAllByRole("definition").map((value) => value.textContent)).toEqual(["None", "None", "None"]);
    fireEvent.click(more);
    expect(region.hidden).toBe(true);
  });

  it("shows no More fields when every value is filled", () => {
    render(<PropertyList label="Details" items={ITEMS.map((item) => ({ ...item, value: item.value ?? "x" }))} foldEmpty />);
    expect(screen.queryByRole("button")).toBeNull();
    expect(terms(screen.getByRole("region", { name: "Details" }))).toHaveLength(5);
  });
});
