import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { App } from "../app/App";
import { SECTIONS } from "../app/sections";
import { en } from "../i18n/en";
import type * as EnModule from "../i18n/en";
import { MockClient } from "./MockClient";
import type { Scenario } from "./scenario";
import { HARBOR_NOTES, workspacesOf } from "./workspaces";

// WS-15 of docs/features/workspaces.md (D12), on the catalog of docs/features/ui-languages.md: with
// the English catalog's `workspace` group replaced by sentinels, the switcher, the module bar, the
// config notes, the nav's wait and the Board's line render nothing but sentinels, served titles,
// notes and numbers: no word of the slice is spelled in a component. Section labels are the
// catalog's `app.sections`.

vi.mock("../i18n/en", async (importOriginal) => {
  const actual = await importOriginal<typeof EnModule>();
  return {
    en: {
      ...actual.en,
      workspace: {
        workspaceLabel: "[workspaceLabel]",
        workspaceHint: "[workspaceHint]",
        modulesLabel: "[modulesLabel]",
        allModules: "[allModules]",
        notesTitle: "[notesTitle]",
        notesButton: "[notesButton {{n}}]",
        readFailed: "[readFailed]",
        readFailedKept: "[readFailedKept]",
        navPending: "[navPending]",
        flowsShown_one: "[flowsShown {{shown}} {{count}}]",
        flowsShown_other: "[flowsShown {{shown}} {{count}}]",
        showAllFlows: "[showAllFlows]",
        showWorkspaceFlows: "[showWorkspaceFlows]",
      },
    },
  };
});

function renderMock(scenario: Scenario, hash: string, prepare: (client: MockClient) => void = () => undefined) {
  window.history.replaceState(null, "", `/?scenario=${scenario}${hash}`);
  const client = new MockClient(scenario, { delayMs: 0 });
  prepare(client);
  render(<App client={client} scenario={scenario === "normal" ? null : scenario} />);
  return client;
}

const SERVED = workspacesOf("harbor-sim", "config-notes");

/** What is left of an element's text once every sentinel, served title, note and number is taken out. */
function leftOver(element: Element): string {
  let text = element.textContent;
  const allowed = [
    ...SERVED.workspaces.map((workspace) => workspace.title),
    ...SERVED.modules.map((module) => module.title),
    ...SERVED.notes,
  ].sort((a, b) => b.length - a.length);
  text = text.replace(/\[[A-Za-z]+(?: \d+)*\]/g, "");
  for (const word of allowed) {
    text = text.replaceAll(word, "");
  }
  return text.replace(/\d+/g, "").trim();
}

function region(selector: string): HTMLElement {
  const found = document.querySelector<HTMLElement>(selector);
  if (found === null) {
    throw new Error(`no ${selector}`);
  }
  return found;
}

describe("one place for the strings (WS-15)", () => {
  it("the switcher, the notes, the Board's line and the bar draw only sentinels, served titles, notes and numbers", async () => {
    renderMock("config-notes", "#/harbor-sim/board");
    const select = await screen.findByRole("combobox", { name: "[workspaceLabel]" });
    expect(document.getElementById(select.getAttribute("aria-describedby") ?? "")?.textContent).toBe("[workspaceHint]");
    expect(leftOver(region(".workspace-switcher"))).toBe("");

    const notes = await screen.findByRole("button", { name: "[notesButton 2]" });
    fireEvent.click(notes);
    const list = screen.getByRole("list", { name: "[notesTitle]" });
    expect(within(list).getAllByRole("listitem").map((item) => item.textContent)).toEqual([...HARBOR_NOTES]);
    expect(leftOver(region(".config-notes"))).toBe("");

    expect(await screen.findByText("[flowsShown 1 2]")).toBeTruthy();
    expect(leftOver(region(".board-flows-line"))).toBe("");
    fireEvent.click(screen.getByRole("button", { name: "[showAllFlows]" }));
    expect(screen.getByRole("button", { name: "[showWorkspaceFlows]" })).toBeTruthy();
    expect(leftOver(region(".board-flows-line"))).toBe("");

    window.location.hash = "#/harbor-sim/tree";
    const bar = await screen.findByRole("group", { name: "[modulesLabel]" });
    expect(within(bar).getAllByRole("button").map((button) => button.textContent)).toEqual(["[allModules]", ...SERVED.modules.map((module) => module.title)]);
    expect(leftOver(bar)).toBe("");
  });

  it("a failed read and the nav's wait draw their sentinels too", async () => {
    renderMock("error", "#/harbor-sim/inbox");
    fireEvent.click(await screen.findByRole("button", { name: "[notesButton 1]" }));
    expect(within(region(".config-notes")).getByText("[readFailed]")).toBeTruthy();

    cleanup();
    renderMock("normal", "#/harbor-sim/inbox", (client) => {
      vi.spyOn(client, "getWorkspaces").mockReturnValue(new Promise(() => undefined));
    });
    await waitFor(() => {
      expect(within(screen.getByRole("navigation", { name: "Sections" })).getByRole("status", { name: "[navPending]" })).toBeTruthy();
    });
  });

  it("keeps every section label in the catalog's app.sections", () => {
    expect(SECTIONS.map((section) => en.app.sections[section.id].label)).toEqual(["Inbox", "Tasks", "Board", "Roadmap", "Flow", "Spec tree", "Graph", "Health", "Questions"]);
  });
});
