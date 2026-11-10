import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { aProposal } from "../test/builders";
import { docClient, FERRY } from "../test/docStub";
import { renderApp } from "../test/render";

// DM-9 of docs/features/doc-modal.md on the river ferry: the modal is one dialog over the view, the
// view inert and the live regions not; it opens with one history entry from a link and closes as a
// card's modal does (Esc, Close, the scrim, Back), focus back to its opener; opened by URL its close
// replaces; a link inside replaces the document; a REF naming nothing says the daemon's reason.

/** Whether an element lies in an inert subtree. */
function inert(element: Element): boolean {
  for (let node: Element | null = element; node !== null; node = node.parentElement) {
    if (node instanceof HTMLElement && node.inert) {
      return true;
    }
  }
  return false;
}

/** The ferry's modal, its body landed and its heading focused. */
async function ferry(): Promise<HTMLElement> {
  const dialog = await screen.findByRole("dialog", { name: "The ferry" });
  await within(dialog).findByText("A flat boat crosses the river on a cable.");
  await waitFor(() => {
    expect(document.activeElement).toBe(within(dialog).getByRole("heading", { level: 2, name: "The ferry" }));
  });
  return dialog;
}

/** The Inbox with a proposal on the ferry, its card open: the "Open document" link a person follows. */
async function fromInbox(): Promise<HTMLAnchorElement> {
  renderApp(docClient([aProposal({ id: "PR-0001", target_ids: [FERRY] })]), "#/alpha/inbox/PR-0001");
  const card = await screen.findByRole("article");
  return within(card).findByRole<HTMLAnchorElement>("link", { name: /^Open document/ });
}

describe("the document modal (DM-9 of doc-modal)", () => {
  it("is one dialog, aria-modal, labelled by its h2, focus on it; the view inert beneath, the live regions not", async () => {
    renderApp(docClient(), `#/alpha/inbox?doc=${FERRY}`);
    const dialog = await ferry();
    expect(screen.getAllByRole("dialog")).toEqual([dialog]);
    expect(dialog.getAttribute("aria-modal")).toBe("true");
    expect(document.getElementById(dialog.getAttribute("aria-labelledby") ?? "")?.textContent).toBe("The ferry");
    expect(inert(screen.getByRole("heading", { level: 1, name: "Inbox", hidden: true }))).toBe(true);
    expect(inert(dialog)).toBe(false);
    for (const live of document.querySelectorAll("[aria-live]")) {
      expect(inert(live)).toBe(false);
    }
  });

  it("opened from a link pushes one entry; Esc goes back, focus to that link; again, Close likewise", async () => {
    const opener = await fromInbox();
    for (const how of ["Escape", "Close"] as const) {
      const before = window.history.length;
      fireEvent.click(opener);
      expect(window.location.hash).toBe(`#/alpha/inbox/PR-0001?doc=${FERRY}`);
      // The first push adds an entry; the second takes the place of the one Back left ahead.
      expect(window.history.length).toBe(how === "Escape" ? before + 1 : before);
      const dialog = await ferry();
      if (how === "Escape") {
        fireEvent.keyDown(document.activeElement ?? dialog, { key: "Escape" });
      } else {
        fireEvent.click(within(dialog).getByRole("button", { name: /^Close/ }));
      }
      await waitFor(() => {
        expect(screen.queryByRole("dialog")).toBeNull();
      });
      expect([how, window.location.hash]).toEqual([how, "#/alpha/inbox/PR-0001"]);
      await waitFor(() => {
        expect(document.activeElement).toBe(opener);
      });
    }
  });

  it("opened by URL, Esc replaces the hash without `doc`, no entry added, focus to the view's h1 (M: Esc always going back)", async () => {
    renderApp(docClient(), `#/alpha/inbox?doc=${FERRY}`);
    const dialog = await ferry();
    const before = window.history.length;
    fireEvent.keyDown(dialog, { key: "Escape" });
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect([window.location.hash, window.history.length]).toEqual(["#/alpha/inbox", before]);
    await waitFor(() => {
      expect(document.activeElement).toBe(screen.getByRole("heading", { level: 1, name: "Inbox" }));
    });
    expect(document.activeElement).not.toBe(document.body);
  });

  it("closes on a click on the scrim (M: no closeOnScrim)", async () => {
    renderApp(docClient(), `#/alpha/inbox?doc=${FERRY}`);
    const dialog = await ferry();
    fireEvent.click(dialog.closest("[data-dialog-host]")?.querySelector("[data-dialog-scrim]") as Element);
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect(window.location.hash).toBe("#/alpha/inbox");
  });

  it("closes on Back; with New card open over it, Back closes both", async () => {
    const opener = await fromInbox();
    fireEvent.click(opener);
    await ferry();
    act(() => {
      window.history.back();
    });
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect(window.location.hash).toBe("#/alpha/inbox/PR-0001");
    fireEvent.click(opener);
    const dialog = await ferry();
    fireEvent.click(within(dialog).getByRole("button", { name: "New card" }));
    expect(await screen.findByRole("dialog", { name: "New card" })).toBeTruthy();
    act(() => {
      window.history.back();
    });
    await waitFor(() => {
      expect(screen.queryAllByRole("dialog")).toEqual([]);
    });
    expect(inert(screen.getByRole("heading", { level: 1, name: "Inbox" }))).toBe(false);
  });

  it("follows a link inside by replacing `doc`: no entry added, the other document's title its heading", async () => {
    const opener = await fromInbox();
    fireEvent.click(opener);
    const dialog = await ferry();
    const before = window.history.length;
    fireEvent.click(within(within(dialog).getByRole("region", { name: "Links" })).getByRole("link", { name: "Boarding" }));
    expect(window.location.hash).toBe("#/alpha/inbox/PR-0001?doc=MEC-BOARDING");
    expect(window.history.length).toBe(before);
    const other = await screen.findByRole("dialog", { name: "Boarding" });
    await waitFor(() => {
      expect(document.activeElement).toBe(within(other).getByRole("heading", { level: 2, name: "Boarding" }));
    });
    // One entry however far one walks: the close goes back past both documents, focus to the first opener.
    fireEvent.keyDown(other, { key: "Escape" });
    await waitFor(() => {
      expect(window.location.hash).toBe("#/alpha/inbox/PR-0001");
    });
    await waitFor(() => {
      expect(document.activeElement).toBe(opener);
    });
  });

  it("a REF that names nothing: the daemon's reason verbatim under lang=en; Close and Open in Spec tree stay, no other act", async () => {
    renderApp(docClient(), "#/alpha/inbox?doc=GONE");
    const dialog = await screen.findByRole("dialog", { name: "GONE" });
    const reason = await within(dialog).findByText("`GONE` resolves to no ID and no alias");
    expect(reason.getAttribute("lang")).toBe("en");
    expect(within(dialog).getByRole("button", { name: /^Close/ })).toBeTruthy();
    expect(within(dialog).getByRole("link", { name: "Open in Spec tree" }).getAttribute("href")).toBe("#/alpha/tree/GONE");
    for (const act of ["New card", "Open in Flow"]) {
      expect([act, within(dialog).queryByRole("button", { name: act }), within(dialog).queryByRole("link", { name: act })]).toEqual([act, null, null]);
    }
  });
});
