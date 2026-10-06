import { act, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { errorAnswer, FakeEventSource, jsonAnswer, stubEventSource, stubFetch, urlsOf } from "./test/daemonStub";

// AC-09 of docs/features/daemon-read.md: the bootstrap serves the daemon by default, without "Mock
// data"; `?scenario=` serves the mock with the flag and the scenario named. Each case imports
// src/main.tsx afresh into a root element of its own and unmounts it after.

let unmount: (() => void) | null = null;

/** Imports the bootstrap at `url` into a fresh `#root`. */
async function boot(url: string) {
  window.history.replaceState(null, "", url);
  const container = document.createElement("div");
  container.id = "root";
  document.body.append(container);
  vi.resetModules();
  const { root } = await act(() => import("./main"));
  unmount = () => {
    act(() => {
      root.unmount();
    });
    container.remove();
  };
}

afterEach(() => {
  unmount?.();
  unmount = null;
  vi.unstubAllGlobals();
});

describe("the bootstrap (AC-09)", () => {
  it("serves the daemon by default: its projects, its live tail, no Mock data", async () => {
    stubEventSource();
    const fetchStub = stubFetch((url) => {
      if (url === "/api/projects") {
        return jsonAnswer(200, [{ slug: "alpha", name: "Alpha", root: "/work/alpha", branch: "main" }]);
      }
      if (url === "/api/projects/alpha/inbox") {
        return jsonAnswer(200, { proposals: [], notes: [] });
      }
      return errorAnswer(404, `no route ${url}`);
    });
    await boot("/#/alpha/inbox");
    expect(await screen.findByRole("option", { name: "Alpha" })).toBeTruthy();
    expect(await screen.findByRole("heading", { level: 2, name: "The queue is clear" })).toBeTruthy();
    expect(screen.queryByText("Mock data")).toBeNull();
    expect(urlsOf(fetchStub)).toContain("/api/projects");
    await waitFor(() => {
      expect(FakeEventSource.instances.map((source) => source.url)).toContain("/api/projects/alpha/events");
    });
  });

  it("serves the mock under ?scenario=empty: Mock data and the scenario shown, the network untouched", async () => {
    stubEventSource();
    const fetchStub = stubFetch(() => errorAnswer(500, "the mock must not read the network"));
    await boot("/?scenario=empty#/harbor-sim/inbox");
    expect(await screen.findByText("Mock data")).toBeTruthy();
    expect(screen.getByText("scenario: empty")).toBeTruthy();
    expect(await screen.findByRole("heading", { level: 2, name: "The queue is clear" })).toBeTruthy();
    expect(fetchStub).not.toHaveBeenCalled();
    expect(FakeEventSource.instances).toEqual([]);
  });

  it("serves the mock under ?scenario=normal, the flag without a scenario name", async () => {
    stubEventSource();
    const fetchStub = stubFetch(() => errorAnswer(500, "the mock must not read the network"));
    await boot("/?scenario=normal#/harbor-sim/inbox");
    expect(await screen.findByText("Mock data")).toBeTruthy();
    expect(screen.queryByText(/^scenario:/)).toBeNull();
    expect(await screen.findByRole("listbox")).toBeTruthy();
    expect(fetchStub).not.toHaveBeenCalled();
  });
});
