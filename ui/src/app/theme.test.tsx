import { act, fireEvent, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import mainSource from "../main.tsx?raw";
import { en } from "../i18n/en";
import { ru } from "../i18n/ru";
import { renderApp } from "../test/render";
import { stubClient } from "../test/stubClient";
import { applyTheme, chooseTheme, storedTheme, THEMES } from "./theme";

// DF-12 of docs/features/design-foundation.md ("Themes", "Theme choice"): System, Light or Dark, per
// viewer; with nothing stored no `data-theme`, so the page follows the system; a choice is stored
// under `specengine.theme` and pins `data-theme` at once, and the bootstrap applies it before the
// first render; System removes both; another value or a storage that throws reads System, silently
// (the setup fails a test that writes to the console).

const KEY = "specengine.theme";

function shownTheme(): string | null {
  return document.documentElement.getAttribute("data-theme");
}

function themeSelect(name: string = en.app.theme.label): HTMLSelectElement {
  return within(document.querySelector(".app-header") as HTMLElement).getByRole<HTMLSelectElement>("combobox", { name });
}

afterEach(() => {
  document.documentElement.removeAttribute("data-theme");
});

describe("the theme (DF-12)", () => {
  it("with nothing stored, sets no data-theme: the page follows the system", () => {
    expect(storedTheme()).toBe("system");
    applyTheme(storedTheme());
    expect(shownTheme()).toBeNull();
    renderApp(stubClient(), "#/alpha/inbox");
    expect(themeSelect().value).toBe("system");
    expect(shownTheme()).toBeNull();
  });

  it("choosing Dark in the header's select sets data-theme and stores it; a fresh load restores it before the first render; System removes both", () => {
    renderApp(stubClient(), "#/alpha/inbox");
    const select = themeSelect();
    select.focus();
    fireEvent.change(select, { target: { value: "dark" } });
    expect([shownTheme(), window.localStorage.getItem(KEY)]).toEqual(["dark", "dark"]);
    expect(document.activeElement).toBe(select);
    // A fresh page: the attribute gone, the bootstrap's own call restores it.
    document.documentElement.removeAttribute("data-theme");
    applyTheme(storedTheme());
    expect(shownTheme()).toBe("dark");
    fireEvent.change(select, { target: { value: "light" } });
    expect([shownTheme(), window.localStorage.getItem(KEY)]).toEqual(["light", "light"]);
    fireEvent.change(select, { target: { value: "system" } });
    expect([shownTheme(), window.localStorage.getItem(KEY)]).toEqual([null, null]);
  });

  it("applies the stored choice in the bootstrap before the root is made", () => {
    const applied = mainSource.indexOf("applyTheme(storedTheme());");
    expect(applied).toBeGreaterThan(0);
    expect(applied).toBeLessThan(mainSource.indexOf("createRoot(container)"));
  });

  it("reads another value, or a storage that throws, as System, and says nothing", () => {
    window.localStorage.setItem(KEY, "sepia");
    expect(storedTheme()).toBe("system");
    applyTheme(storedTheme());
    expect(shownTheme()).toBeNull();
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("storage refused");
    });
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("storage refused");
    });
    expect(storedTheme()).toBe("system");
    // A refused write still shows the choice for this page.
    chooseTheme("dark");
    expect(shownTheme()).toBe("dark");
  });

  it("names the select Theme and offers System, Light and Dark, in THEMES' order, in both languages", async () => {
    expect(THEMES).toEqual(["system", "light", "dark"]);
    renderApp(stubClient(), "#/alpha/inbox");
    expect(Array.from(themeSelect().options, (option) => [option.value, option.textContent])).toEqual([
      ["system", "System"],
      ["light", "Light"],
      ["dark", "Dark"],
    ]);
    fireEvent.change(within(document.querySelector(".app-header") as HTMLElement).getByRole("combobox", { name: en.language.label }), { target: { value: "ru" } });
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 30));
    });
    expect(Array.from(themeSelect(ru.app.theme.label).options, (option) => [option.value, option.textContent])).toEqual([
      ["system", ru.app.theme.system],
      ["light", ru.app.theme.light],
      ["dark", ru.app.theme.dark],
    ]);
    expect(screen.queryByText(en.app.theme.label)).toBeNull();
  });
});
