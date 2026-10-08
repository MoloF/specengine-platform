import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { App } from "../app/App";
import { MockClient } from "./MockClient";
import type { Scenario } from "./scenario";

// U-11 of docs/features/ui-round.md: the Questions screen over the real mock, as `pnpm dev` shows it
// with `?scenario=`: normal (stage two answers, see them staged with the terminal step), conflict
// (one answer refused as changed since read, marked, picked again and staged), empty, error.

function renderMock(scenario: Scenario, hash: string) {
  window.history.replaceState(null, "", `/?scenario=${scenario}${hash}`);
  const client = new MockClient(scenario, { delayMs: 0 });
  render(<App client={client} scenario={scenario === "normal" ? null : scenario} />);
  return client;
}

function card(id: string): HTMLElement {
  return screen.getByRole("article", { name: id });
}

function result(): string {
  return document.querySelector(".decision-result p")?.textContent ?? "";
}

describe("the Questions screen over the mock", () => {
  it("normal: harbor-sim's sheet in round order; two answers staged in one go, shown staged with the terminal step", async () => {
    renderMock("normal", "#/harbor-sim/questions");
    await screen.findByRole("article", { name: "PR-0041" });
    expect(screen.getAllByRole("article").map((article) => article.querySelector("h2")?.textContent)).toEqual([
      "PR-0041",
      "PR-0048",
      "PR-0043",
      "PR-0047",
      "PR-0044",
    ]);
    expect(within(screen.getByRole("list", { name: "Notes from the daemon" })).getByRole("listitem").textContent).toBe(
      "1 open change proposal(s) not on this sheet: PR-0042; spec inbox lists them",
    );
    fireEvent.click(within(card("PR-0048")).getByRole("radio", { name: "Take the working answer" }));
    fireEvent.click(within(card("PR-0041")).getByRole("radio", { name: /^\[0\] Code to spec/ }));
    fireEvent.click(screen.getByRole("button", { name: "Stage 2 answers" }));
    await waitFor(() => {
      expect(result()).toMatch(/^Staged 2 answers\. Nothing is applied yet\. Confirm on a terminal: spec round answer --staged;/);
    });
    await within(card("PR-0041")).findByRole("region", { name: "Staged decision" });
    await within(card("PR-0048")).findByRole("region", { name: "Staged decision" });
    const confirm = screen.getByRole("region", { name: "Confirm on a terminal" });
    expect(confirm.querySelector(".round-confirm-line")?.textContent).toBe(
      "Confirm on a terminal in /work/harbor-sim: spec round answer --staged; each answer is asked again there, one at a time.",
    );
    expect(screen.getByText("Mock data")).toBeTruthy();
  });

  it("conflict: the first Stage refuses one answer as changed since read; picked again, it is staged", async () => {
    renderMock("conflict", "#/harbor-sim/questions");
    await screen.findByRole("article", { name: "PR-0041" });
    fireEvent.click(within(card("PR-0041")).getByRole("radio", { name: /^\[0\] Code to spec/ }));
    fireEvent.click(within(card("PR-0048")).getByRole("radio", { name: "Take the working answer" }));
    fireEvent.click(screen.getByRole("button", { name: "Stage 2 answers" }));
    expect(await within(card("PR-0041")).findByText(/^`PR-0041` changed since it was read/)).toBeTruthy();
    await within(card("PR-0041")).findByText("Changed since you answered");
    await within(card("PR-0048")).findByRole("region", { name: "Staged decision" });
    fireEvent.click(within(card("PR-0041")).getByRole("button", { name: "Keep my answer for this version of PR-0041" }));
    fireEvent.click(screen.getByRole("button", { name: "Stage 1 answer" }));
    await waitFor(() => {
      expect(result()).toMatch(/^Staged 1 answer\./);
    });
    const staged = await within(card("PR-0041")).findByRole("region", { name: "Staged decision" });
    expect(within(staged).getByText("Code to spec")).toBeTruthy();
  });

  it("empty: no open questions or discrepancies, and the next step", async () => {
    renderMock("empty", "#/harbor-sim/questions");
    expect(await screen.findByRole("heading", { level: 2, name: "No open questions or discrepancies." })).toBeTruthy();
  });

  it("error: the mock's 503 in its words, with Retry", async () => {
    renderMock("error", "#/harbor-sim/questions");
    const alert = await within(screen.getByRole("main")).findByText(/the database is locked by another process/);
    expect(alert.closest('[role="alert"]')).not.toBeNull();
  });
});
