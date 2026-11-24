import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { App } from "../app/App";
import { MockClient } from "./MockClient";
import type { Scenario } from "./scenario";

// docs/canon/work-references-promotion-surfaces.md "UI" (docs/features/reference-promotion.md "Acceptance criteria", PR17) over the mock as `pnpm dev` shows it
// (src/mocks/references.ts): harbor-sim's task C-0003, whose target is the night passage, lists that
// document's key "In the spec"; its harbour image's "Promote to canon..." stores a `reference` proposal,
// which the Inbox lists with its kind and the daemon's note naming the file it adds, LFS or plain.
// The mock is imported here alone (policy AC-05).

function renderMock(hash: string, scenario: Scenario = "normal") {
  window.localStorage.setItem("specengine.module.harbor-sim", "");
  window.history.replaceState(null, "", `/?scenario=${scenario}${hash}`);
  const client = new MockClient(scenario, { delayMs: 0 });
  render(<App client={client} scenario={scenario === "normal" ? null : scenario} />);
  return client;
}

describe("promotion over the mock", () => {
  it("lists C-0003's inherited files as text, promotes its image, and the Inbox shows the proposal's kind and note", async () => {
    renderMock("#/harbor-sim/board/C-0003");
    const dialog = await screen.findByRole("dialog", { name: "Night passage rules, plain words" });
    const group = await within(dialog).findByRole("region", { name: "In the spec \u00b7 3" });
    expect(within(group).getAllByRole("link").map((link) => link.textContent)).toEqual(["MEC-NIGHT-PASSAGE", "MEC-NIGHT-PASSAGE", "MEC-NIGHT-PASSAGE"]);
    expect(group.textContent).toContain("docs/spec/fairway/media/buoys-at-night.png");
    const tile = dialog.querySelector<HTMLElement>('[data-reference="RF-0002"]');
    if (tile === null) {
      throw new Error("no RF-0002");
    }
    fireEvent.click(within(tile).getByRole("button", { name: "Promote to canon\u2026" }));
    const promote = await screen.findByRole("dialog", { name: "Promote RF-0002 to canon" });
    fireEvent.click(within(promote).getByRole("button", { name: "Propose" }));
    const line = await within(promote).findByText(/: a person confirms it in the Inbox$/);
    const link = within(line).getByRole("link");
    const id = link.textContent;
    expect(link.getAttribute("href")).toBe(`#/harbor-sim/inbox/${id}`);
    expect(within(promote).getByRole("list", { name: "What the daemon notes" }).textContent).toBe("adds docs/spec/fairway/media/harbour.png: RF-0002, image/png 82 B, LFS");
    window.location.hash = `#/harbor-sim/inbox/${id}`;
    const inbox = await screen.findByRole("heading", { level: 1, name: "Inbox" });
    const main = inbox.closest("main") ?? document.body;
    await waitFor(() => {
      expect(main.textContent).toContain("adds docs/spec/fairway/media/harbour.png: RF-0002, image/png 82 B, LFS");
    });
    // The kind as the daemon words it, raw, as every kind is shown (ADR-0031).
    expect(Array.from(main.querySelectorAll(".mono"), (element) => element.textContent).filter((text) => text === "reference").length).toBeGreaterThan(0);
  });
});
