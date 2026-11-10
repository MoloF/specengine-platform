import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { docClient, FERRY } from "../test/docStub";
import { renderApp } from "../test/render";

// DM-11 of docs/features/doc-modal.md: a document's page, `#/<p>/tree/<REF>/page`, the modal's body
// alone under its `h1`, no tools, the Spec tree marked in the nav; a reload keeps it.

async function page(hash = `#/alpha/tree/${FERRY}/page`) {
  renderApp(docClient(), hash);
  const heading = await screen.findByRole("heading", { level: 1, name: "The ferry" });
  await screen.findByText("A flat boat crosses the river on a cable.");
  return heading;
}

describe("a document's page (DM-11 of doc-modal)", () => {
  it("shows the body under its h1, no tools, no dialog; the nav marks Spec tree", async () => {
    const heading = await page();
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(screen.queryByRole("link", { name: "Open as a page" })).toBeNull();
    expect(screen.queryByRole("button", { name: /^Close/ })).toBeNull();
    expect(screen.getAllByRole("heading", { level: 1 })).toEqual([heading]);
    expect(within(screen.getByRole("navigation", { name: "Sections" })).getByRole("link", { name: "Spec tree" }).getAttribute("aria-current")).toBe("page");
    expect(screen.getByRole("heading", { level: 2, name: "Text" })).toBeTruthy();
    // The text's own headings one below Text's: h3 on the page.
    expect(Array.from(document.querySelectorAll(".doc-text .md-heading"), (heading) => [heading.tagName, heading.textContent])).toEqual([["H3", "Tides RULE-FERRY-TIDES"]]);
    expect(screen.getByRole("link", { name: "Open in Flow" })).toBeTruthy();
  });

  it("keeps it on a reload", async () => {
    await page();
    cleanup();
    window.history.replaceState(null, "", `/#/alpha/tree/${FERRY}/page`);
    expect(await page()).toBeTruthy();
  });

  it("follows an ancestor crumb as a link: that document's page, one entry pushed", async () => {
    await page();
    const before = window.history.length;
    const crumb = within(screen.getByRole("navigation", { name: "Breadcrumbs" })).getByRole("link", { name: "The river" });
    expect(crumb.getAttribute("href")).toBe("#/alpha/tree/DOM-RIVER/page");
    // A plain click is the link's own here, never the modal's replace; jsdom follows it a task later, the browser at once.
    expect(fireEvent.click(crumb)).toBe(true);
    window.location.hash = crumb.getAttribute("href") ?? "";
    expect(await screen.findByRole("heading", { level: 1, name: "The river" })).toBeTruthy();
    expect(window.history.length).toBeGreaterThan(before);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("says a REF naming nothing in the daemon's words under lang=en", async () => {
    renderApp(docClient(), "#/alpha/tree/GONE/page");
    expect(await screen.findByRole("heading", { level: 1, name: "GONE" })).toBeTruthy();
    const reason = await screen.findByText("`GONE` resolves to no ID and no alias");
    expect(reason.getAttribute("lang")).toBe("en");
    expect(screen.getByRole("link", { name: "Open in Spec tree" }).getAttribute("href")).toBe("#/alpha/tree/GONE");
  });

  it("opens a document link on the page as the modal over it", async () => {
    await page();
    fireEvent.click(within(screen.getByRole("region", { name: "Links" })).getByRole("link", { name: "Boarding" }));
    await waitFor(() => {
      expect(window.location.hash).toBe(`#/alpha/tree/${FERRY}/page?doc=MEC-BOARDING`);
    });
    expect(await screen.findByRole("dialog", { name: "Boarding" })).toBeTruthy();
    expect(screen.getByRole("heading", { level: 1, name: "The ferry" })).toBeTruthy();
  });
});
