import { screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import appCss from "../styles/app.css?raw";
import { aWorkspace, aWorkspaces } from "../test/builders";
import { renderApp } from "../test/render";
import { stubClient } from "../test/stubClient";

// DF-14 of docs/features/design-foundation.md ("The header"): one row 48 px tall from 68.75rem, its
// selects named as ever (Project, Workspace, Language, Theme) with their labels visually hidden and
// their values shown with an icon in front; Jump to and Shortcuts keep their names; the brand, then
// Project, Workspace, the config notes, Language, Theme, the mock indicator, Jump to and Shortcuts.

/** The text of each `@media (<query>)` block, braces matched. */
function mediaBlocks(query: string): string {
  const found: string[] = [];
  let from = 0;
  for (;;) {
    const at = appCss.indexOf(`@media (${query})`, from);
    if (at < 0) {
      return found.join("\n");
    }
    const open = appCss.indexOf("{", at);
    let depth = 0;
    let end = open;
    for (; end < appCss.length; end += 1) {
      depth += appCss[end] === "{" ? 1 : appCss[end] === "}" ? -1 : 0;
      if (depth === 0) {
        break;
      }
    }
    found.push(appCss.slice(open + 1, end));
    from = end;
  }
}

function ruleOf(selector: string, css: string): string {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return new RegExp(`(^|\\n)\\s*${escaped}\\s*\\{([^}]*)\\}`).exec(css)?.[2] ?? "";
}

async function header(): Promise<HTMLElement> {
  const client = stubClient();
  client.getWorkspaces.mockResolvedValue(aWorkspaces([aWorkspace({ name: "galley", title: "Galley" }), aWorkspace({ name: "foundry", title: "Foundry" })], [], { notes: ["config: a note"] }));
  renderApp(client, "#/alpha/inbox", "config-notes");
  const shown = document.querySelector<HTMLElement>(".app-header");
  if (shown === null) {
    throw new Error("no header");
  }
  await within(shown).findByRole("combobox", { name: "Department" });
  return shown;
}

describe("the one-line header (DF-14)", () => {
  it("names its selects Project, Department, Language and Theme, each label in the DOM and visually hidden", async () => {
    const shown = await header();
    const selects = within(shown).getAllByRole<HTMLSelectElement>("combobox");
    expect(selects.map((select) => (select.labels[0]?.textContent ?? ""))).toEqual(["Project", "Department", "Language", "Theme"]);
    for (const select of selects) {
      const label = select.labels[0];
      expect([label?.textContent, label?.classList.contains("sr-only")]).toEqual([label?.textContent, true]);
      // The value is shown with an icon in front.
      expect(select.parentElement?.querySelector(":scope > svg.icon")?.getAttribute("aria-hidden")).toBe("true");
      expect(select.classList.contains("header-field")).toBe(true);
    }
  });

  it("keeps Jump to and Shortcuts by their names, their labels' text the names below 80rem too", async () => {
    const shown = await header();
    const jump = within(shown).getByRole("button", { name: "Jump to" });
    const shortcuts = within(shown).getByRole("button", { name: "Keyboard shortcuts" });
    for (const button of [jump, shortcuts]) {
      expect(button.querySelector(".header-button-label")?.textContent).toBe(button === jump ? "Jump to" : "Keyboard shortcuts");
    }
    const narrow = mediaBlocks("max-width: 79.99rem");
    expect(ruleOf(".app-header .header-button-label", narrow)).toMatch(/clip-path:\s*inset\(50%\);/);
  });

  it("orders the brand, Project, Workspace, the config notes, Language, Theme, the mock indicator, Jump to and Shortcuts", async () => {
    const shown = await header();
    await within(shown).findByRole("button", { name: /^Config notes/ });
    const order = Array.from(shown.children, (child) => child.className.split(" ").find((name) => ["brand", "project-switcher", "workspace-switcher", "config-notes", "language-switcher", "theme-switcher", "data-source", "jump-button", "shortcuts-button"].includes(name)) ?? child.className);
    expect(order).toEqual(["brand", "project-switcher", "workspace-switcher", "config-notes", "language-switcher", "theme-switcher", "data-source", "jump-button", "shortcuts-button"]);
    expect(screen.getByText("Mock data")).toBeTruthy();
  });

  // The review of design-foundation (iteration 2): at 1100 px in Russian the selects were squeezed to
  // stubs while the notes and the mock indicator kept their full width. Each select now holds its
  // value, and below 80rem the notes and the indicator take a short form, their names kept.
  it("never squeezes a select below its value; below 100rem the config notes show their icon and count, the mock indicator its icon and Mock data", async () => {
    const select = ruleOf(".header-select", appCss);
    expect(select).toMatch(/flex:\s*none;/);
    expect(select).not.toMatch(/min-width|flex-shrink:\s*[1-9]|flex:\s*0 1/);
    const field = ruleOf(".header-select .header-field", appCss);
    expect(field).toMatch(/field-sizing:\s*content;/);
    expect(field).toMatch(/width:\s*auto;/);
    const shortForms = mediaBlocks("max-width: 99.99rem");
    expect(shortForms).toMatch(/\.app-header \.config-notes-button \.header-button-label,\s*\.app-header \.data-source-scenario\s*\{[^}]*clip-path:\s*inset\(50%\);/);
    expect(ruleOf(".app-header .config-notes-count", shortForms)).toMatch(/display:\s*inline;/);
    expect(ruleOf(".config-notes-count", appCss)).toMatch(/display:\s*none;/);
    const shown = await header();
    const notes = await within(shown).findByRole("button", { name: "Config notes (1)" });
    expect([notes.querySelector(".header-button-label")?.textContent, notes.querySelector(".config-notes-count")?.textContent, notes.querySelector(".config-notes-count")?.getAttribute("aria-hidden")]).toEqual(["Config notes (1)", "1", "true"]);
    expect(shown.querySelector(".data-source-scenario")?.textContent).toBe("scenario: config-notes");
  });

  it("is 48 px tall at least, its line drawn inside; it never wraps from 68.75rem", () => {
    const base = ruleOf(".app-header", appCss);
    expect(base).toMatch(/min-height:\s*var\(--nav-height\);/);
    expect(base).not.toMatch(/border-bottom/);
    expect(base).toMatch(/flex-wrap:\s*wrap;/);
    const wide = mediaBlocks("min-width: 68.75rem");
    expect(ruleOf(".app-header", wide)).toMatch(/flex-wrap:\s*nowrap;/);
    expect(ruleOf(".app-header", wide)).not.toMatch(/flex-wrap:\s*wrap;/);
  });
});
