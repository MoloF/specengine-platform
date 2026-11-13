import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { Proposal } from "../api/types";
import { aProposal } from "../test/builders";
import { homeClient } from "../test/homeStub";
import { renderApp } from "../test/render";
import { stubClient } from "../test/stubClient";
import { treeClient } from "../test/treeStub";
import { shownSlot } from "../ui/text";

// docs/features/release-hardening.md "Acceptance criteria", RH-U4: a held gist is drawn as the lead
// draws it, verbatim in a `<bdi>` through shownSlot, by one shared component on every surface that
// lists a proposal: the Spec tree's Proposals tab, Health's queue, the palette and the Inbox card's
// others, as the Inbox's row and the card's title already did. An entry with a summary alone looks as
// before. M: any one surface back on raw `summaryOf`.

const RLO = String.fromCodePoint(0x202e);
const ESC = String.fromCodePoint(0x1b);
const GIST = `Should a deep ship ${RLO}wait${ESC}[31m at the anchorage?`;
const SHOWN = shownSlot(GIST);
const SUMMARY = "Entry only inside the tide window";

/** On DOC-A: a question with the gist (its summary never shown while the gist leads), one with a summary alone, and the one the Inbox opens. */
const QUEUE: Proposal[] = [
  aProposal({ id: "PR-0071", kind: "question", target_ids: ["DOC-A"], summary: "The old words", gist: GIST, severity: "high", created_at: "2026-10-03T10:00:00Z" }),
  aProposal({ id: "PR-0072", target_ids: ["DOC-A"], summary: SUMMARY, severity: "high", created_at: "2026-10-02T10:00:00Z" }),
  aProposal({ id: "PR-0073", target_ids: ["DOC-A"], summary: "The one opened", severity: "low", created_at: "2026-10-01T10:00:00Z" }),
];

/** Each `bdi`'s text in a scope, then the scope's whole text. */
function drawn(scope: Element | null | undefined): [(string | null)[], string | null | undefined] {
  return [Array.from(scope?.querySelectorAll("bdi") ?? [], (bdi) => bdi.textContent), scope?.textContent];
}

describe("a gist drawn as the lead draws it, on every surface (RH-U4)", () => {
  it("writes the controls out `\\u{..}` and isolates the gist", () => {
    expect(SHOWN).toBe("Should a deep ship \\u{202e}wait\\u{1b}[31m at the anchorage?");
  });

  it("in the Inbox card's others", async () => {
    renderApp(stubClient(QUEUE), "#/alpha/inbox/PR-0073");
    const card = await screen.findByRole("article");
    const others = within(card).getByRole("heading", { name: "Also open on these nodes" }).closest("section");
    const line = (id: string) => within(others as HTMLElement).getByRole("button", { name: id }).closest("li")?.lastElementChild;
    expect(drawn(line("PR-0071"))).toEqual([[SHOWN], SHOWN]);
    expect(drawn(line("PR-0072"))).toEqual([[], SUMMARY]);
  });

  it("in the Spec tree's Proposals tab", async () => {
    renderApp(treeClient(QUEUE), "#/alpha/tree/DOC-A");
    await screen.findByRole("tab", { name: "Text" });
    await waitFor(() => {
      expect(screen.getByRole("tab", { name: /^Proposals/ }).textContent).toBe("Proposals 3");
    });
    fireEvent.click(screen.getByRole("tab", { name: /^Proposals/ }));
    const panel = screen.getByRole("tabpanel");
    const line = (id: string) => within(panel).getByRole("link", { name: id }).closest("li")?.lastElementChild;
    expect(drawn(line("PR-0071"))).toEqual([[SHOWN], SHOWN]);
    expect(drawn(line("PR-0072"))).toEqual([[], SUMMARY]);
  });

  it("in Health's queue", async () => {
    renderApp(stubClient(QUEUE), "#/alpha/health");
    const left = await screen.findByRole("region", { name: "What is left" });
    const link = async (id: string) => (await within(left).findByText(id)).closest("a");
    expect(drawn(await link("PR-0071"))).toEqual([[SHOWN], `PR-0071: ${SHOWN}`]);
    expect(drawn(await link("PR-0072"))).toEqual([[], `PR-0072: ${SUMMARY}`]);
  });

  it("in the palette", async () => {
    renderApp(homeClient([], QUEUE), "#/alpha");
    await screen.findByRole("heading", { level: 1, name: "Overview" });
    fireEvent.keyDown(document.body, { key: "k", code: "KeyK", metaKey: true });
    const dialog = await screen.findByRole("dialog", { name: "Jump to" });
    fireEvent.change(within(dialog).getByRole("combobox"), { target: { value: "PR-007" } });
    const option = async (id: string) => (await within(dialog).findByText(id)).closest(".palette-option-main");
    expect(drawn(await option("PR-0071"))).toEqual([[SHOWN], `PR-0071: ${SHOWN}`]);
    expect(drawn(await option("PR-0072"))).toEqual([[], `PR-0072: ${SUMMARY}`]);
  });
});
