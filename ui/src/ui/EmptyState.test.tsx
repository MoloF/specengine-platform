import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { Button } from "./Button";
import { EmptyState } from "./EmptyState";

// DF-7 of docs/features/design-foundation.md: an empty state is a heading at the level its caller
// picks, a short description, and at most two acts, at most one of them primary.

describe("EmptyState (DF-7)", () => {
  it.each([2, 3, 4] as const)("puts its heading at level %i", (level) => {
    render(<EmptyState title="No agent task yet." level={level} />);
    expect(screen.getByRole("heading", { level, name: "No agent task yet." })).toBeTruthy();
    expect(screen.queryByRole("button")).toBeNull();
  });

  it("holds its description and at most two acts, at most one primary", () => {
    const { container } = render(
      <EmptyState
        title="No cards yet"
        level={2}
        description={<p>Make the first one.</p>}
        primary={<Button appearance="primary">New card</Button>}
        secondary={<Button>Read how</Button>}
      />,
    );
    expect(screen.getByText("Make the first one.")).toBeTruthy();
    const acts = container.querySelector(".empty-state-acts");
    expect(acts?.querySelectorAll("button")).toHaveLength(2);
    expect(acts?.querySelectorAll(".button-primary")).toHaveLength(1);
    expect(screen.getAllByRole("button").map((button) => button.textContent)).toEqual(["New card", "Read how"]);
  });
});
