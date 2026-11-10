import { waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { closeDoc, forgetDoc, openDoc, openDocFrom, pushHash, replaceHash } from "./location";
import { withDoc } from "./routes";

// DM-6 of docs/features/doc-modal.md: a screen's own replace keeps an open document modal; a push
// drops it; the modal's close goes back to the entry before it when this page pushed it, else replaces.

function at(hash: string) {
  window.history.replaceState(null, "", `/${hash}`);
}

describe("hash writes and the document modal (DM-6 of doc-modal)", () => {
  beforeEach(() => {
    forgetDoc();
  });

  it("carries an open `doc` pair over a replace naming none: a selection, a seek's fix, a correction (M: replaceHash not carrying doc)", () => {
    at("#/p/flow?doc=A");
    replaceHash("#/p/flow?module=&scope=S");
    expect(window.location.hash).toBe("#/p/flow?module=&scope=S&doc=A");
    replaceHash("#/p/flow/X?scope=S");
    expect(window.location.hash).toBe("#/p/flow/X?scope=S&doc=A");
    replaceHash("#/p/flow");
    expect(window.location.hash).toBe("#/p/flow?doc=A");
    // A replace naming its own document keeps its own; the modal's close and the landing pass `keepDoc: false`.
    replaceHash(withDoc("#/p/flow", "B"));
    expect(window.location.hash).toBe("#/p/flow?doc=B");
    replaceHash("#/p/inbox", { keepDoc: false });
    expect(window.location.hash).toBe("#/p/inbox");
    // With no modal open a replace adds nothing.
    replaceHash("#/p/board/C-1");
    expect(window.location.hash).toBe("#/p/board/C-1");
  });

  it("drops the pair on a push", () => {
    at("#/p/flow?doc=A");
    const before = window.history.length;
    pushHash("#/p/inbox");
    expect(window.location.hash).toBe("#/p/inbox");
    expect(window.history.length).toBe(before + 1);
  });

  it("opens with one pushed entry, replaces the document while open, and its close goes back", async () => {
    at("#/p/inbox");
    const before = window.history.length;
    openDoc("ID#SEC");
    expect(window.location.hash).toBe("#/p/inbox?doc=ID%23SEC");
    expect(window.history.length).toBe(before + 1);
    openDoc("B");
    expect(window.location.hash).toBe("#/p/inbox?doc=B");
    expect(window.history.length).toBe(before + 1);
    closeDoc();
    await waitFor(() => {
      expect(window.location.hash).toBe("#/p/inbox");
    });
  });

  it("opens over a given base when the current route shows no modal: one push the close goes back from (the review's R1)", async () => {
    at("#/p/nowhere");
    openDocFrom("A", null, "#/p");
    expect(window.location.hash).toBe("#/p?doc=A");
    // Going back lands on the route it was opened from: the open was a push.
    closeDoc();
    await waitFor(() => {
      expect(window.location.hash).toBe("#/p/nowhere");
    });
  });

  it("counts no push its route cannot show as its own: the close replaces, never goes back (the review's R1)", () => {
    at("#/p/inbox");
    openDocFrom("A", null, "#/p/nowhere");
    expect(window.location.hash).toBe("#/p/nowhere?doc=A");
    const before = window.history.length;
    closeDoc();
    expect(window.location.hash).toBe("#/p/nowhere");
    expect(window.history.length).toBe(before);
  });

  it("closes a modal it did not push (a URL arrival, a reload) by a replace without `doc`, no entry added", () => {
    at("#/p/board/C-1?doc=A");
    const before = window.history.length;
    closeDoc();
    expect(window.location.hash).toBe("#/p/board/C-1");
    expect(window.history.length).toBe(before);
  });
});
