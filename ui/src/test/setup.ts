import { cleanup } from "@testing-library/react";
import { afterEach, beforeEach, vi } from "vitest";
import { recordConsole, takeConsoleCalls } from "./console";

beforeEach(() => {
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
// (docs/features/ui-graph.md "Canvas"). It never reports: jsdom measures nothing, so the layout
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
