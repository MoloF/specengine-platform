import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { Proposal } from "../api/types";
import { docPageHash } from "../app/routes";
import { aCited, aProposal } from "../test/builders";
import { renderApp } from "../test/render";
import { stubClient } from "../test/stubClient";

// docs/features/department-practices.md "Acceptance criteria", DP21 and "Proposals: `cited`": an Inbox item's
// texts link each string its review served in `cited` to that node's document, in the rendered
// markdown (the rationale, the working answer, the why) and in the one-line slots (the title, an
// option's label); an ID-shaped token it did not serve, and one inside code, stay text. A review
// serving no citation links nothing.

/** A question citing the served PRA-ART-NORMALS and the unserved PRA-NOPE-X in each text. */
function aCitingQuestion(fields: Partial<Omit<Proposal, "id">> = {}): Proposal {
  return aProposal({
    id: "PR-0080",
    kind: "question",
    severity: "normal",
    target_ids: ["SCN-CRANE"],
    summary: "Do props keep PRA-ART-NORMALS? PRA-NOPE-X is not written.",
    rationale: "The crane's props break PRA-ART-NORMALS, as `PRA-ART-NORMALS` in the log says; PRA-NOPE-X names nothing.\n\n- a [link PRA-ART-NORMALS](docs/x.md) stays the link's\n\n```\nPRA-ART-NORMALS in a fence\n```",
    working_answer: "Yes: PRA-ART-NORMALS holds for props.",
    why: "Per PRA-ART-NORMALS.",
    options: [
      { label: "Keep PRA-ART-NORMALS", effect: "PRA-ART-NORMALS kept.", price: "None." },
      { label: "Drop PRA-NOPE-X", effect: "Nothing.", price: "None." },
    ],
    cited: [aCited("PRA-ART-NORMALS", { name: "art/PRA-ART-NORMALS", title: "Normal maps" })],
    ...fields,
  });
}

async function inboxCard(proposal: Proposal): Promise<HTMLElement> {
  const client = stubClient([proposal]);
  renderApp(client, "#/alpha/inbox/PR-0080");
  const card = await screen.findByRole("article");
  await within(card).findByRole("heading", { level: 3, name: "Provenance" });
  return card;
}

/** The cited links in a scope: their words and where they go. */
function citedLinks(scope: HTMLElement): [string, string][] {
  return within(scope)
    .queryAllByRole("link")
    .filter((link) => link.getAttribute("href") === docPageHash("alpha", "art/PRA-ART-NORMALS"))
    .map((link) => [link.textContent, link.getAttribute("href") ?? ""]);
}

/** A section of the card by its heading. */
function section(card: HTMLElement, name: string): HTMLElement {
  const heading = within(card).getByRole("heading", { level: 3, name });
  const holder = heading.closest("section");
  if (holder === null) {
    throw new Error(`no section ${name}`);
  }
  return holder;
}

describe("an Inbox item's cited links (DP21)", () => {
  it("links the served string in the rationale's prose, never inside code, a fence or another link; an unserved ID stays text", async () => {
    const card = await inboxCard(aCitingQuestion());
    const rationale = section(card, "Rationale");
    await waitFor(() => {
      expect(citedLinks(rationale)).toEqual([["PRA-ART-NORMALS", docPageHash("alpha", "art/PRA-ART-NORMALS")]]);
    });
    expect(within(rationale).getAllByText("PRA-ART-NORMALS", { selector: "code" })).toHaveLength(1);
    expect(rationale.textContent).toContain("PRA-NOPE-X names nothing.");
    expect(rationale.textContent).toContain("PRA-ART-NORMALS in a fence");
    expect(within(rationale).queryAllByRole("link").filter((link) => link.textContent.includes("PRA-NOPE"))).toEqual([]);
  });

  it("links it in the title, an option's label, the working answer and the why", async () => {
    const card = await inboxCard(aCitingQuestion());
    expect(citedLinks(within(card).getByRole("heading", { level: 2 }))).toHaveLength(1);
    const labels = Array.from(card.querySelectorAll<HTMLElement>(".option-label"));
    expect(labels.map((label) => citedLinks(label).length)).toEqual([1, 0]);
    expect(labels[1]?.textContent).toContain("Drop PRA-NOPE-X");
    await waitFor(() => {
      expect(citedLinks(section(card, "Working answer"))).toHaveLength(1);
    });
    await waitFor(() => {
      expect(citedLinks(section(card, "Why"))).toHaveLength(1);
    });
    // Every link a served string: nothing ID-shaped links unasked.
    expect(within(card).queryAllByRole("link").filter((link) => /^PRA-NOPE/.test(link.textContent))).toEqual([]);
  });

  it("links nothing when the review served no citation, the texts as before", async () => {
    const card = await inboxCard(aCitingQuestion({ cited: null }));
    const rationale = section(card, "Rationale");
    await waitFor(() => {
      expect(rationale.querySelector(".markdown-pending")).toBeNull();
    });
    expect(citedLinks(card)).toEqual([]);
    expect(within(card).getByRole("heading", { level: 2 }).textContent).toBe("Do props keep PRA-ART-NORMALS? PRA-NOPE-X is not written.");
  });
});
