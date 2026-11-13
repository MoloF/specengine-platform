import { cleanup } from "@testing-library/react";
import { afterEach, beforeEach, vi } from "vitest";
import { recordConsole, takeConsoleCalls } from "./console";

beforeEach(async () => {
  // The viewer's choices kept in storage (the tree's Show drift, docs/features/ui-tree-drift.md) start each test cleared.
  window.localStorage.clear();
  // Each test starts in English, `<html lang>` with it, Russian's chunk loaded once per file so a
  // switch is at once (docs/canon/ui-languages.md "Tests"). Read here, not imported above: a test
  // file mocking the catalog loads it first.
  const { loadLanguage, switchLanguage } = await import("../i18n");
  await loadLanguage("ru");
  switchLanguage("en");
  takeConsoleCalls();
  vi.spyOn(console, "error").mockImplementation((...values: unknown[]) => {
    recordConsole("error", values);
  });
  vi.spyOn(console, "warn").mockImplementation((...values: unknown[]) => {
    recordConsole("warn", values);
  });
});

afterEach(() => {
  cleanup();
  window.history.replaceState(null, "", "/");
  vi.restoreAllMocks();
  vi.useRealTimers();
  const calls = takeConsoleCalls();
  if (calls.length > 0) {
    throw new Error(`The test wrote to the console:\n${calls.join("\n")}`);
  }
});

// jsdom lays nothing out and has no scrollIntoView; the views call it to bring a row or a line
// into sight. A no-op keeps them honest about calling it without a layout to scroll.
Element.prototype.scrollIntoView = function scrollIntoView() {
  return undefined;
};

// jsdom has no ResizeObserver; React Flow constructs one for its pane unguarded
// (ui/README.md "Screen rules", Canvas). It never reports: jsdom measures nothing, so the layout
// gives every box its size and handles.
class StillResizeObserver implements ResizeObserver {
  observe(): void {
    return undefined;
  }

  unobserve(): void {
    return undefined;
  }

  disconnect(): void {
    return undefined;
  }
}
globalThis.ResizeObserver = StillResizeObserver;

// jsdom draws nothing and reports `getContext` as not implemented on the console; the Map's canvas
// asks for a 2D context and draws nothing without one
// (docs/features/corpus-map.md "Acceptance criteria", CM7). A test that reads the drawing stubs a
// recording context of its own.
HTMLCanvasElement.prototype.getContext = function getContext() {
  return null;
} as typeof HTMLCanvasElement.prototype.getContext;
