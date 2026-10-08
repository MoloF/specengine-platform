import { act, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import source from "./main.tsx?raw";
import { takeConsoleCalls } from "./test/console";
import { errorAnswer, FakeEventSource, jsonAnswer, stubEventSource, stubFetch, urlsOf } from "./test/daemonStub";

// AC-09 of docs/features/daemon-read.md: the bootstrap serves the daemon by default, without "Mock
// data"; `?scenario=` serves the mock with the flag and the scenario named. Each case imports
// src/main.tsx afresh into a root element of its own and unmounts it after. The mock client loads
// in a chunk of its own, only under `?scenario=` (kind-interpretation's pre-shipping pass: the main
// bundle stays under Vite's 500 kB warning).

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
  vi.doUnmock("./mocks/MockClient");
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

  it("imports the mock client only on demand: no static import, no await on any line of code", () => {
    // Comments may say the word; code, indented or not, may not.
    const code = source.split("\n").filter((line) => !/^\s*(\/\/|\/\*|\*)/.test(line));
    expect(code.filter((line) => /from\s+["']\.\/mocks\/MockClient["']/.test(line))).toEqual([]);
    // The dynamic import spelled in pieces, so this file imports no mock itself (the seam's scan).
    const dynamic = ["import(", '"./mocks/MockClient")'].join("");
    expect(code.filter((line) => line.includes(dynamic))).toHaveLength(1);
    expect(code.filter((line) => /\bawait\b/.test(line))).toEqual([]);
  });

  it("says why the page is empty when the mock's chunk cannot be loaded", async () => {
    stubEventSource();
    const fetchStub = stubFetch(() => errorAnswer(500, "the mock must not read the network"));
    vi.doMock("./mocks/MockClient", () => {
      throw new Error("chunk MockClient failed to load");
    });
    await boot("/?scenario=normal#/harbor-sim/inbox");
    const alert = await screen.findByRole("alert");
    // The loader's reason as it gave it (Vitest words a failed mock factory its own way).
    expect(alert.textContent).toMatch(/^The mock data could not be loaded: \S.*\. Reload the page to try again\.$/);
    expect(screen.queryByText("Mock data")).toBeNull();
    expect(fetchStub).not.toHaveBeenCalled();
    expect(takeConsoleCalls()).toEqual([]);
  });

  it("says the same when the mock's client cannot be built: a throw in its constructor", async () => {
    stubEventSource();
    stubFetch(() => errorAnswer(500, "the mock must not read the network"));
    // A constructor that throws, as `new MockClient(scenario)` would on a broken fixture.
    vi.doMock("./mocks/MockClient", () => ({
      MockClient: function MockClient() {
        throw new Error("the mock's projects could not be built");
      },
    }));
    await boot("/?scenario=normal#/harbor-sim/inbox");
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toBe("The mock data could not be loaded: the mock's projects could not be built. Reload the page to try again.");
    expect(screen.queryByText("Loading the mock data")).toBeNull();
    expect(takeConsoleCalls()).toEqual([]);
  });

  it("shows one busy line in the root until the mock's chunk arrives, then the app over the mock", async () => {
    stubEventSource();
    const fetchStub = stubFetch(() => errorAnswer(500, "the mock must not read the network"));
    let arrive: () => void = () => undefined;
    const arrived = new Promise<void>((resolve) => {
      arrive = resolve;
    });
    vi.doMock("./mocks/MockClient", async (importOriginal) => {
      await arrived;
      return importOriginal();
    });
    await boot("/?scenario=normal#/harbor-sim/inbox");
    const loading = screen.getByText("Loading the mock data");
    expect([loading.tagName, loading.getAttribute("role"), loading.getAttribute("aria-busy")]).toEqual(["P", "status", "true"]);
    expect(document.getElementById("root")?.textContent).toBe("Loading the mock data");
    arrive();
    expect(await screen.findByText("Mock data")).toBeTruthy();
    expect(screen.queryByText("Loading the mock data")).toBeNull();
    expect(fetchStub).not.toHaveBeenCalled();
  });
});
