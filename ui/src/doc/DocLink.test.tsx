import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { forgetDoc } from "../app/location";
import { docPageHash } from "../app/routes";
import { DocLink } from "./DocLink";

// DM-7 of docs/features/doc-modal.md: a document link's href is its page; a plain click opens the
// modal (a push from a screen, a replace inside the modal); a modified, middle or context click is
// the browser's.

/** Whether the last click reaching the window was prevented; the window then stops jsdom following it. */
let prevented: boolean | null = null;
function watch(event: Event) {
  prevented = event.defaultPrevented;
  event.preventDefault();
}

function at(hash: string) {
  window.history.replaceState(null, "", `/${hash}`);
}

function link(): HTMLAnchorElement {
  render(
    <DocLink project="p" docRef="ID#SEC" className="probe">
      The section
    </DocLink>,
  );
  return screen.getByRole("link", { name: "The section" });
}

describe("DocLink (DM-7 of doc-modal)", () => {
  beforeEach(() => {
    forgetDoc();
    prevented = null;
    window.addEventListener("click", watch);
  });

  afterEach(() => {
    window.removeEventListener("click", watch);
  });

  it("links the document's page, its REF encoded", () => {
    expect(link().getAttribute("href")).toBe(docPageHash("p", "ID#SEC"));
    expect(docPageHash("p", "ID#SEC")).toBe("#/p/tree/ID%23SEC/page");
  });

  it("opens the modal on a plain click: the default prevented, one pushed entry", () => {
    at("#/p/inbox");
    const before = window.history.length;
    fireEvent.click(link());
    expect(prevented).toBe(true);
    expect(window.location.hash).toBe("#/p/inbox?doc=ID%23SEC");
    expect(window.history.length).toBe(before + 1);
  });

  it("replaces the open modal's document inside it: no entry added", () => {
    at("#/p/inbox?doc=OTHER");
    const before = window.history.length;
    fireEvent.click(link());
    expect(window.location.hash).toBe("#/p/inbox?doc=ID%23SEC");
    expect(window.history.length).toBe(before);
  });

  it.each([
    ["Meta", { metaKey: true }],
    ["Ctrl", { ctrlKey: true }],
    ["Shift", { shiftKey: true }],
    ["Alt", { altKey: true }],
    ["the middle button", { button: 1 }],
  ])("leaves a click with %s to the browser: nothing prevented, the hash unchanged (M: preventing under Meta)", (_name, fields) => {
    at("#/p/inbox");
    const before = window.history.length;
    fireEvent.click(link(), fields);
    expect(prevented).toBe(false);
    expect(window.location.hash).toBe("#/p/inbox");
    expect(window.history.length).toBe(before);
  });

  it("leaves auxclick and the context menu to the browser", () => {
    at("#/p/inbox");
    const anchor = link();
    expect(fireEvent(anchor, new MouseEvent("auxclick", { bubbles: true, cancelable: true, button: 1 }))).toBe(true);
    expect(fireEvent.contextMenu(anchor)).toBe(true);
    expect(window.location.hash).toBe("#/p/inbox");
  });

  it("takes Enter on the focused link as a plain click: the click a browser dispatches for it, detail 0", () => {
    at("#/p/inbox");
    const anchor = link();
    anchor.focus();
    fireEvent.click(anchor, { detail: 0 });
    expect(prevented).toBe(true);
    expect(window.location.hash).toBe("#/p/inbox?doc=ID%23SEC");
  });

  it("keeps a caller's tab stop and drag choices (a Board tile's source)", () => {
    render(
      <DocLink project="p" docRef="R-1" tabIndex={-1} draggable={false}>
        R-1
      </DocLink>,
    );
    const anchor = screen.getByRole("link", { name: "R-1" });
    expect([anchor.getAttribute("tabindex"), anchor.getAttribute("draggable")]).toEqual(["-1", "false"]);
  });
});
