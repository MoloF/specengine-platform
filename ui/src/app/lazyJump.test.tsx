import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { homeClient } from "../test/homeStub";
import { renderApp } from "../test/render";

// docs/features/handoff-progress.md "Acceptance criteria", S-1: the Roadmap and the Graph arrive in
// chunks of their own. The route shows its skeleton, then the view; a palette jump to either still
// focuses the view's heading once the chunk lands, though the Suspense boundary resolves without a
// render of the Shell. Each view is reached once per file, so each test sees its chunk land.

function combobox(): HTMLInputElement {
  return screen.getByRole("combobox", { name: "Jump to a section, agent task, proposal, node or project" });
}

async function jumpTo(section: string) {
  fireEvent.keyDown(document.body, { key: "k", code: "KeyK", metaKey: true });
  await screen.findByRole("dialog", { name: "Jump to" });
  fireEvent.change(combobox(), { target: { value: section } });
  const listbox = screen.getByRole("listbox", { name: "Places to jump to" });
  fireEvent.click(within(listbox).getByRole("option", { name: section }));
}

describe("the lazy views and a palette jump (S-1)", () => {
  it("a jump to the Roadmap shows its skeleton, then focuses the view's heading once its chunk lands", async () => {
    renderApp(homeClient([]), "#/alpha");
    await screen.findByRole("heading", { level: 1, name: "Overview" });
    await jumpTo("Roadmap");
    await waitFor(() => {
      expect(window.location.hash).toBe("#/alpha/roadmap");
    });
    expect(await screen.findByRole("status", { name: "Opening the roadmap" })).toBeTruthy();
    const heading = await screen.findByRole("heading", { level: 1, name: "Roadmap" });
    await waitFor(() => {
      expect(document.activeElement).toBe(heading);
    });
    expect(screen.queryByRole("status", { name: "Opening the roadmap" })).toBeNull();
  });

  it("a jump to the Graph shows its skeleton, then the view; with no REF its field takes the focus after the heading, as before the move", async () => {
    renderApp(homeClient([]), "#/alpha");
    await screen.findByRole("heading", { level: 1, name: "Overview" });
    await jumpTo("Graph");
    await waitFor(() => {
      expect(window.location.hash).toBe("#/alpha/graph");
    });
    expect(await screen.findByRole("status", { name: "Opening the graph" })).toBeTruthy();
    await screen.findByRole("heading", { level: 1, name: "Graph" });
    const field = screen.getByLabelText("REF");
    await waitFor(() => {
      expect(document.activeElement).toBe(field);
    });
    expect(screen.queryByRole("status", { name: "Opening the graph" })).toBeNull();
  });
});
