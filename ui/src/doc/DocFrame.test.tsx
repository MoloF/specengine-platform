import { screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { docClient, FERRY } from "../test/docStub";
import { renderApp } from "../test/render";

// The frame keeps its own title through the parts' first load (docs/features/doc-modal.md
// "Acceptance criteria", DM-9, DM-11; the review's L1): the chunk is fetched once a page, so this file
// opens the modal and the page before any test here has loaded it. The heading the dialog focused is
// the same node once the body lands, and focus never leaves it, so a mouse open draws no focus ring.

describe("the frame's title through the first load", () => {
  it("in the modal: the same h2 before and after the parts land, focus on it throughout", async () => {
    renderApp(docClient(), `#/alpha/inbox?doc=${FERRY}`);
    const dialog = await screen.findByRole("dialog");
    const heading = within(dialog).getByRole("heading", { level: 2 });
    // The parts' chunk has not landed: the main column is the frame's skeleton, the title and tools stand.
    expect(within(dialog).getByRole("status", { name: "Reading the document…" })).toBeTruthy();
    expect(dialog.querySelector(".doc-part")).toBeNull();
    expect(within(dialog).getByRole("button", { name: /^Close/ })).toBeTruthy();
    expect(document.activeElement).toBe(heading);
    let lost = false;
    const watch = () => {
      if (document.activeElement === document.body || document.activeElement === null) {
        lost = true;
      }
    };
    document.addEventListener("focusout", watch);
    await within(dialog).findByText("A flat boat crosses the river on a cable.");
    document.removeEventListener("focusout", watch);
    expect(within(dialog).getByRole("heading", { level: 2, name: "The ferry" })).toBe(heading);
    expect(heading.isConnected).toBe(true);
    expect(document.activeElement).toBe(heading);
    expect(lost).toBe(false);
    // The work item is the dialog's own child: its scrolling rules hold (L2).
    expect(dialog.querySelector(":scope > .issue-view")).not.toBeNull();
  });
});
