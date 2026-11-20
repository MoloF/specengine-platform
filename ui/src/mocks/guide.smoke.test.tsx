import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { App } from "../app/App";
import { guideHash } from "../app/routes";
import { MockClient } from "./MockClient";
import type { Scenario } from "./scenario";

// docs/features/specengine-guide.md "The UI", GU14, over the mock as `pnpm dev` shows it: Documents'
// SpecEngine guide shelf lists the ten topics in the guide's order; a topic opens read only with its
// rules and Copy citation; harbor-sim's C-0002 goal links its guide citation of a served topic and
// leaves one of an unknown topic as text; an unknown topic's route is the daemon's 404 in the CLI's
// words; the `error` scenario's locked index leaves the guide answering, as the daemon's does.

function renderMock(scenario: Scenario, hash: string) {
  window.history.replaceState(null, "", `/?scenario=${scenario}${hash}`);
  render(<App client={new MockClient(scenario, { delayMs: 0 })} scenario={scenario === "normal" ? null : scenario} />);
}

describe("SpecEngine's guide over the mock", () => {
  it("lists the ten topics in the guide's order on Documents' guide shelf", async () => {
    renderMock("normal", "#/harbor-sim/documents");
    fireEvent.click(await screen.findByRole("button", { name: "SpecEngine guide" }));
    const list = await screen.findByRole("list", { name: "10 topics" });
    expect(within(list).getAllByRole("listitem").map((row) => row.querySelector(".mono")?.textContent)).toEqual([
      "start",
      "records",
      "features",
      "proposals",
      "cards",
      "practices",
      "auto",
      "markers",
      "hub",
      "documents",
    ]);
    expect(within(list).getByRole("link", { name: "Proposals and the one writing door" }).getAttribute("href")).toBe(guideHash("harbor-sim", "proposals"));
  });

  it("opens a topic read only, its rules each with Copy citation, a citation of another topic a link", async () => {
    renderMock("normal", guideHash("harbor-sim", "start"));
    expect(await screen.findByRole("heading", { level: 1, name: "Adopting SpecEngine" })).toBeTruthy();
    const article = document.querySelector<HTMLElement>(".guide-article") as HTMLElement;
    await within(article).findByRole("button", { name: "Copy citation guide:start#GD-START-ONE-CONFIG" });
    expect(within(article).getAllByRole("button")).toHaveLength(2);
    expect((await within(article).findByRole("link", { name: "guide:records" })).getAttribute("href")).toBe(guideHash("harbor-sim", "records"));
  });

  it("links C-0002's citation of a served topic and leaves an unknown one as text", async () => {
    renderMock("normal", "#/harbor-sim/board/C-0002");
    const dialog = await screen.findByRole("dialog", { name: "Caption the berth plans" });
    const link = await within(dialog).findByRole("link", { name: "guide:cards#GD-CARDS-HANDOFF" });
    expect(link.getAttribute("href")).toBe(guideHash("harbor-sim", "cards", "GD-CARDS-HANDOFF"));
    expect(within(dialog).queryByRole("link", { name: /charts/ })).toBeNull();
    expect(dialog.textContent).toContain("guide:charts is no topic and stays text.");
  });

  it("answers an unknown topic with the daemon's 404 in the CLI's words", async () => {
    renderMock("normal", guideHash("harbor-sim", "charts"));
    expect(await screen.findByText("The guide topic charts could not be read")).toBeTruthy();
    expect(screen.getByText(/^spec: no guide topic `charts`; topics: start, records, features/)).toBeTruthy();
  });

  it("keeps the guide answering under `error`'s locked index", async () => {
    renderMock("error", guideHash("harbor-sim", "documents"));
    expect(await screen.findByRole("heading", { level: 1, name: "The documents library" })).toBeTruthy();
  });
});
