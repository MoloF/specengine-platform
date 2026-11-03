import { screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { Author, Proposal } from "../api/types";
import { aProposal } from "../test/builders";
import { renderApp } from "../test/render";
import { stubClient, type StubClient } from "../test/stubClient";

// H15 of docs/features/hub-mcp.md, its UI part, on a stub client: a proposal raised over a hub's
// `/mcp` carries the asking person's `login` (`docs/features/hub-mcp.md` "Identity"), and the
// card's Provenance shows it in a Login row after Author, as text: markup stays characters, and
// every control and format character is written `\u{..}`, as a Confirm dialog writes them. An
// author with no `login` shows as before, with no Login row, on a hub or a local page. Characters
// past ASCII are built from code points, so no source line holds one.

/** An agent's author as a hub's `/mcp` stores it: `login` the request's. */
const BY_MIRA: Author = { type: "agent", role: "drafter", model: "m-1", run: "r-1", login: "mira" };

/** The same author raised over stdio or on a terminal: no `login` key. */
const NO_LOGIN: Author = { type: "agent", role: "drafter", model: "m-1", run: "r-1" };

/** A discrepancy raised by `author`, as the review document gives it. */
function raisedBy(author: Author | null): Proposal {
  return aProposal({
    id: "PR-0060",
    kind: "discrepancy",
    severity: "normal",
    target_id: "R-1",
    target_ids: ["R-1"],
    summary: "The berth plan names the basin differently",
    author,
    created_at: "2026-11-03T09:00:00Z",
  });
}

/** A stub of a hub read as `tomas`, a member: another person than the proposal's author. */
function hubAs(proposals: Proposal[]): StubClient {
  const client = stubClient(proposals);
  client.getIdentity.mockResolvedValue({ login: "tomas" });
  client.getTeam.mockResolvedValue({ login: "tomas", name: null, allowed: ["stage", "accept", "clear"] });
  return client;
}

/** The card of PR-0060 with its review read: its Provenance section. */
async function provenanceOf(client: StubClient): Promise<HTMLElement> {
  renderApp(client, "#/alpha/inbox/PR-0060");
  const card = await screen.findByRole("article");
  const heading = await within(card).findByRole("heading", { level: 3, name: "Provenance" });
  const section = heading.closest("section");
  if (section === null) {
    throw new Error("the Provenance heading is in no section");
  }
  return section;
}

/** Each term of a section's pairs with the text of its value. */
function pairsOf(section: HTMLElement): [string | null, string | null | undefined][] {
  return Array.from(section.querySelectorAll("dt"), (term) => [term.textContent, term.nextElementSibling?.textContent]);
}

describe("the proposal card shows a hub's login of who raised it, as text (H15 of hub-mcp)", () => {
  it("shows Login `mira` after Author, the stored login and never the reader's", async () => {
    const provenance = await provenanceOf(hubAs([raisedBy(BY_MIRA)]));
    expect(pairsOf(provenance).slice(0, 4)).toEqual([
      ["Author", "agent, role drafter"],
      ["Login", "mira"],
      ["Model", "m-1"],
      ["Run", "r-1"],
    ]);
    const value = within(provenance).getByText("mira");
    expect(value.tagName).toBe("BDI");
    expect(value.parentElement?.tagName).toBe("DD");
    expect(provenance.textContent).not.toContain("tomas");
  });

  it("shows the same on a local page: the row follows the stored author, not the page's mode", async () => {
    const provenance = await provenanceOf(stubClient([raisedBy(BY_MIRA)]));
    expect(pairsOf(provenance).slice(0, 2)).toEqual([
      ["Author", "agent, role drafter"],
      ["Login", "mira"],
    ]);
  });

  it.each([
    ["on a hub", () => hubAs([raisedBy(NO_LOGIN)])],
    ["on a local page", () => stubClient([raisedBy(NO_LOGIN)])],
  ])("shows an author with no login as before, with no Login row, %s", async (_where, client) => {
    const provenance = await provenanceOf(client());
    expect(pairsOf(provenance).slice(0, 3)).toEqual([
      ["Author", "agent, role drafter"],
      ["Model", "m-1"],
      ["Run", "r-1"],
    ]);
    expect(pairsOf(provenance).map(([term]) => term)).not.toContain("Login");
  });

  it("shows an unknown author as before, with no Login row", async () => {
    const provenance = await provenanceOf(hubAs([raisedBy(null)]));
    expect(pairsOf(provenance).slice(0, 2)).toEqual([
      ["Author", "Unknown"],
      ["Model", "Unknown"],
    ]);
    expect(pairsOf(provenance).map(([term]) => term)).not.toContain("Login");
  });

  it("shows a login holding markup, an override and a format character escaped: no `b` element, no raw character", async () => {
    const override = String.fromCodePoint(0x202e);
    const space = String.fromCodePoint(0x200b);
    const stored = `<b>mi${override}ra</b>${space}`;
    const provenance = await provenanceOf(hubAs([raisedBy({ ...BY_MIRA, login: stored })]));
    const login = pairsOf(provenance).find(([term]) => term === "Login");
    expect(login).toEqual(["Login", "<b>mi\\u{202e}ra</b>\\u{200b}"]);
    const card = screen.getByRole("article");
    expect(card.querySelector("b")).toBeNull();
    expect(card.textContent).not.toContain(override);
    expect(card.textContent).not.toContain(space);
  });
});
