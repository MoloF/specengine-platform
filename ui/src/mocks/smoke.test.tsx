import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { App } from "../app/App";
import { BASIN_NAME_DECOMPOSED, LONG_TOKEN } from "./harbor-sim/fixtures";
import { MockClient } from "./MockClient";
import type { Scenario } from "./scenario";

// The whole UI over the real mock, as `pnpm dev` shows it (the bootstrap's wiring, without the DOM root).

function renderMock(scenario: Scenario, hash: string) {
  window.history.replaceState(null, "", `/?scenario=${scenario}${hash}`);
  const client = new MockClient(scenario, { delayMs: 0 });
  render(<App client={client} scenario={scenario === "normal" ? null : scenario} />);
  return client;
}

describe("the UI over the mock", () => {
  it("shows harbor-sim's queue in order with its odd cases intact", async () => {
    renderMock("normal", "#/harbor-sim/inbox");
    const list = await screen.findByRole("listbox");
    const ids = within(list)
      .getAllByRole("option")
      .map((option) => option.dataset.proposal);
    expect(ids).toEqual(["PR-0041", "PR-0042", "PR-0046", "PR-0043", "PR-0047", "PR-0045", "PR-0044"]);
    expect(within(list).getByText(new RegExp(BASIN_NAME_DECOMPOSED))).toBeTruthy();
    fireEvent.click(within(list).getByText(/undocumented sample format/));
    expect(await screen.findByText(new RegExp(LONG_TOKEN))).toBeTruthy();
    expect(screen.getByText("Mock data")).toBeTruthy();
  });

  it("accepts PR-0041 and announces the commit", async () => {
    renderMock("normal", "#/harbor-sim/inbox/PR-0041");
    const card = await screen.findByRole("article");
    await within(card).findByText("Entry only inside the tide window");
    fireEvent.click(within(card).getByRole("button", { name: "Accept" }));
    const dialog = await screen.findByRole("dialog", { name: "Accept PR-0041" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Accept" }));
    await waitFor(() => {
      expect(document.querySelector('[aria-live="polite"]')?.textContent).toMatch(
        /^Accepted PR-0041: committed [0-9a-f]{40} "spec: apply PR-0041"\.$/,
      );
    });
    await waitFor(() => {
      expect(within(screen.getByRole("listbox")).queryByText("PR-0041")).toBeNull();
    });
  });

  it("shows a refused apply in the daemon's words and keeps the dialog", async () => {
    renderMock("normal", "#/harbor-sim/inbox/PR-0046");
    const card = await screen.findByRole("article");
    fireEvent.click(within(card).getByRole("button", { name: "Accept" }));
    const dialog = await screen.findByRole("dialog");
    fireEvent.change(within(dialog).getByLabelText("Note for the record (optional)"), { target: { value: "try" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Accept" }));
    expect(await within(dialog).findByText(/conflicts with the current text of RULE-PILOT-REQ/)).toBeTruthy();
    expect(within(dialog).getByLabelText("Note for the record (optional)")).toHaveProperty("value", "try");
  });

  it("conflict: a decision closes with the 409 message and the proposal leaves", async () => {
    renderMock("conflict", "#/ledger-api/inbox");
    const card = await screen.findByRole("article");
    fireEvent.click(within(card).getByRole("button", { name: "Defer" }));
    const dialog = await screen.findByRole("dialog");
    fireEvent.click(within(dialog).getByRole("button", { name: "Defer" }));
    expect(await within(screen.getByRole("main")).findByText(/PR-0007 is no longer open/)).toBeTruthy();
    expect(document.querySelector('[aria-live="assertive"]')?.textContent).toMatch(/PR-0007 is no longer open/);
    await waitFor(() => {
      expect(within(screen.getByRole("listbox")).queryByText("PR-0007")).toBeNull();
    });
    expect(screen.getByText("scenario: conflict")).toBeTruthy();
  });

  it("error: the inbox says the daemon's message and offers Retry", async () => {
    renderMock("error", "#/harbor-sim/inbox");
    const alert = await screen.findByRole("alert");
    expect(within(alert).getByText(/mock scenario: error/)).toBeTruthy();
    expect(alert.contains(screen.getByRole("button", { name: "Retry" }))).toBe(false);
  });

  it("empty: the queue is clear for both projects", async () => {
    renderMock("empty", "#/ledger-api/inbox");
    expect(await screen.findByRole("heading", { name: "The queue is clear" })).toBeTruthy();
  });
});
