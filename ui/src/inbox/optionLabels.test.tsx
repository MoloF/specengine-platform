import { fireEvent, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { aProposal } from "../test/builders";
import { renderApp } from "../test/render";
import { stubClient } from "../test/stubClient";

// docs/features/ui-round.md "Description and interactions" (agent text as the sheet writes it),
// applied everywhere an option's label shows (review iteration 3 of ui-round): the Inbox card's
// options and the decision dialog's radios write a control or bidirectional character as `\u{..}`
// and isolate the label, as StagedChoice and the Questions screen do. The override is built from
// its code point, so this file stays ASCII.

const RLO = String.fromCodePoint(0x202e);

const PR_0004 = aProposal({
  id: "PR-0004",
  kind: "discrepancy",
  summary: "Stamina cap",
  options: [
    { label: `left ${RLO}right`, effect: "Fix the code", price: "One test" },
    { label: "Spec to code", effect: "Edit the rule", price: "A note" },
  ],
  recommendation: 0,
});

describe("an option's label in the Inbox", () => {
  it("is written as the sheet writes a slot, isolated, on the card and in the decision dialog", async () => {
    renderApp(stubClient([PR_0004]), "#/alpha/inbox/PR-0004");
    const card = await screen.findByRole("article");
    await within(card).findByRole("heading", { level: 3, name: "Provenance" });
    const onCard = within(card).getByText("left \\u{202e}right");
    expect(onCard.tagName).toBe("BDI");
    fireEvent.click(within(card).getByRole("button", { name: "Accept" }));
    const dialog = await screen.findByRole("dialog", { name: "Accept PR-0004" });
    const radio = within(dialog).getByRole("radio", { name: /^\[0\] left \\u\{202e\}right/ });
    const inDialog = within(radio.closest("label") as HTMLElement).getByText("left \\u{202e}right");
    expect(inDialog.tagName).toBe("BDI");
    expect(document.body.textContent).not.toContain(RLO);
  });
});
