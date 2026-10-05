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
