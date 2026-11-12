import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { App } from "../app/App";
import { SECTIONS, sectionLabel } from "../app/sections";
import { en } from "../i18n/en";
import { enT } from "../test/text";
import { MockClient } from "./MockClient";
import type { Scenario } from "./scenario";
import { HUB_DEFAULT } from "./workspaces";

// docs/features/hub-department.md "Acceptance criteria", HD3, HD4 and HD8's steps, over the mock as
// `pnpm dev` shows it: `hub` asks as `mira`, whom harbor-sim's config lists under foundry, not the
// first department; `normal` asks with no login and is served no default. A stored choice wins over
// the default; on a hub it is the login's own key, and the browser-wide one stays unread. A department
// stays a lens: every section route renders its view, and the palette lists all nine.

const MIRA_WORKSPACE = "specengine.workspace.harbor-sim.mira";
const MIRA_MODULE = "specengine.module.harbor-sim.mira";
const BROWSER_WORKSPACE = "specengine.workspace.harbor-sim";

function renderMock(scenario: Scenario, hash: string) {
  window.history.replaceState(null, "", `/?scenario=${scenario}${hash}`);
  render(<App client={new MockClient(scenario, { delayMs: 0 })} scenario={scenario === "normal" ? null : scenario} />);
}

function switcher(): HTMLSelectElement {
  return screen.getByRole<HTMLSelectElement>("combobox", { name: en.workspace.workspaceLabel });
}

async function lands(hash: string, workspace: string) {
  await waitFor(() => {
    expect(window.location.hash).toBe(hash);
  });
  await waitFor(() => {
    expect(switcher().value).toBe(workspace);
  });
}

describe("a hub login's department (HD3, HD8)", () => {
  it("the mock serves `hub` its login's department, which is not the first; `normal` none", async () => {
    const hub = await new MockClient("hub", { delayMs: 0 }).getWorkspaces("harbor-sim");
    expect([hub.default, hub.workspaces[0]?.name === HUB_DEFAULT]).toEqual([HUB_DEFAULT, false]);
    expect((await new MockClient("normal", { delayMs: 0 }).getWorkspaces("harbor-sim")).default).toBeNull();
    expect((await new MockClient("hub", { delayMs: 0 }).getWorkspaces("ledger-api")).default).toBeNull();
  });

  it("no stored choice: `#/` lands on the default's landing on a hub, on the first's locally", async () => {
    renderMock("hub", "#/");
    await lands("#/harbor-sim", HUB_DEFAULT);
  });

  it("locally, with no default, the first department's landing", async () => {
    renderMock("normal", "#/");
    await lands("#/harbor-sim/flow", "galley");
  });

  it("a stored choice wins over the default: the login's own key", async () => {
    window.localStorage.setItem(MIRA_WORKSPACE, "galley");
    renderMock("hub", "#/");
    await lands("#/harbor-sim/flow", "galley");
  });

  it("a stored name no longer served falls to the default", async () => {
    window.localStorage.setItem(MIRA_WORKSPACE, "rigging");
    renderMock("hub", "#/");
    await lands("#/harbor-sim", HUB_DEFAULT);
  });

  it("the browser-wide key stays unread on a hub", async () => {
    window.localStorage.setItem(BROWSER_WORKSPACE, "harbourmaster");
    renderMock("hub", "#/");
    await lands("#/harbor-sim", HUB_DEFAULT);
  });

  it("choosing writes the login's key, removes the login's module key, leaves the browser-wide key alone", async () => {
    window.localStorage.setItem(BROWSER_WORKSPACE, "harbourmaster");
    window.localStorage.setItem(MIRA_MODULE, "waters");
    renderMock("hub", "#/harbor-sim/board");
    await waitFor(() => {
      expect(switcher().value).toBe(HUB_DEFAULT);
    });
    fireEvent.change(switcher(), { target: { value: "galley" } });
    expect([window.localStorage.getItem(MIRA_WORKSPACE), window.localStorage.getItem(MIRA_MODULE), window.localStorage.getItem(BROWSER_WORKSPACE)]).toEqual(["galley", null, "harbourmaster"]);
  });
});

describe("a lens, not a filter (HD4)", () => {
  it.each(SECTIONS.map((section) => section.id))("on a hub, #/harbor-sim/%s renders its view, no redirect", async (id) => {
    renderMock("hub", `#/harbor-sim/${id}`);
    await waitFor(() => {
      expect(switcher().value).toBe(HUB_DEFAULT);
    });
    expect(await screen.findByRole("heading", { level: 1, name: sectionLabel(id, enT) })).toBeTruthy();
    expect(window.location.hash).toBe(`#/harbor-sim/${id}`);
  });

  it("the nav lists the default's sections, the switcher every department", async () => {
    renderMock("hub", "#/harbor-sim/inbox");
    await waitFor(() => {
      expect(switcher().value).toBe(HUB_DEFAULT);
    });
    expect(within(switcher()).getAllByRole("option").map((option) => option.textContent)).toEqual(["Galley", "Foundry", "Harbourmaster"]);
    expect(
      within(screen.getByRole("navigation", { name: "Sections" }))
        .getAllByRole("link")
        .map((link) => link.textContent),
    ).toEqual(["Overview", "Board", "Health", "Graph", "Spec tree"]);
  });

  it("as mira the palette lists all nine sections, the Overview first, those off the default's nav too", async () => {
    renderMock("hub", "#/harbor-sim/inbox");
    await waitFor(() => {
      expect(switcher().value).toBe(HUB_DEFAULT);
    });
    fireEvent.keyDown(document.body, { key: "k", ctrlKey: true });
    const dialog = await screen.findByRole("dialog", { name: "Jump to" });
    const group = await waitFor(() => {
      const found = within(dialog)
        .getAllByRole("group")
        .find((element) => document.getElementById(element.getAttribute("aria-labelledby") ?? "")?.textContent.startsWith("Sections") === true);
      expect(found).toBeDefined();
      return found as HTMLElement;
    });
    // M: the palette's Sections cut to the default's nav.
    expect(SECTIONS).toHaveLength(9);
    expect(within(group).getAllByRole("option").map((option) => option.textContent)).toEqual(["Overview", ...SECTIONS.map((section) => sectionLabel(section.id, enT))]);
  });
});
