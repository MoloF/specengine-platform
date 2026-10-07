import { screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ClientError, NOT_SERVED } from "../api/client";
import { HttpClient } from "../api/http";
import { errorAnswer, jsonAnswer, stubEventSource, stubFetch, urlsOf } from "../test/daemonStub";
import { renderApp } from "../test/render";
import { stubClient } from "../test/stubClient";

// R-n7 of the daemon-read review: a read the daemon does not serve yet (the tasks, a task;
// `docs/features/ui-live.md` "Out of scope", AC-11) is not built, not broken. Its screen says so
// with the client's message verbatim and offers no Retry; a daemon down (no response, status 0) on
// the same screen is an alert with Retry. The graph and the check are served: liveScreens.test.tsx.

const NOT_BUILT = "Not built yet: the daemon has no endpoint for this read";

/** The daemon: one project with an empty inbox; anything else its 404. */
function daemon() {
  return stubFetch((url) => {
    if (url === "/api/projects") {
      return jsonAnswer(200, [{ slug: "alpha", name: "Alpha", root: "/work/alpha", branch: "main" }]);
    }
    if (url === "/api/projects/alpha/inbox") {
      return jsonAnswer(200, { proposals: [], notes: [] });
    }
    return errorAnswer(404, `no route ${url}`);
  });
}

/** The note saying the read is not built, found by its title. */
async function notBuilt(): Promise<HTMLElement> {
  const title = await screen.findByText(NOT_BUILT);
  const note = title.closest<HTMLElement>(".notice");
  if (note === null) {
    throw new Error("the title sits in no notice");
  }
  return note;
}

beforeEach(() => {
  stubEventSource();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("a read the daemon does not serve yet (R-n7)", () => {
  it.each([
    ["Tasks", "#/alpha/tasks", "GET /api/projects/alpha/tasks"],
    ["a task", "#/alpha/tasks/T-0001", "GET /api/projects/alpha/tasks/T-0001"],
    ["the home's Tasks region", "#/alpha", "GET /api/projects/alpha/tasks"],
  ])("%s: not built, the client's message verbatim, no Retry, no alert", async (_name, hash, endpoint) => {
    const fetchStub = daemon();
    renderApp(new HttpClient(), hash);
    const note = await notBuilt();
    expect(note.getAttribute("role")).toBe("status");
    expect(within(note).getByText(/^Not served by the daemon yet: /).textContent).toBe(
      `Not served by the daemon yet: ${endpoint} is a missing endpoint (docs/features/ui-live.md "Out of scope"). The mock serves it: open the UI with ?scenario=normal.`,
    );
    expect(screen.queryByRole("button", { name: /^Retry/ })).toBeNull();
    expect(screen.queryAllByRole("alert").filter((alert) => alert.textContent.includes("Not served"))).toEqual([]);
    expect(urlsOf(fetchStub).filter((url) => url.includes("/tasks"))).toEqual([]);
  });

  it("the same screen, the daemon down: an alert with Retry, never the not-built note", async () => {
    const client = stubClient();
    client.getTasks.mockImplementation(() =>
      Promise.reject(new ClientError({ status: 0, message: "GET /api/projects/alpha/tasks: no response from the daemon (Failed to fetch)" })),
    );
    renderApp(client, "#/alpha/tasks");
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toBe("The tasks could not be loadedGET /api/projects/alpha/tasks: no response from the daemon (Failed to fetch)");
    expect(screen.getByRole("button", { name: "Retry" })).toBeTruthy();
    expect(screen.queryByText(NOT_BUILT)).toBeNull();
  });

  it("goes by the client's flag, not by the status or the words", async () => {
    const client = stubClient();
    client.getTasks.mockImplementation(() =>
      Promise.reject(new ClientError({ status: NOT_SERVED, message: "Not served by the daemon yet: a 501 the daemon itself sent" })),
    );
    renderApp(client, "#/alpha/tasks");
    expect((await screen.findByRole("alert")).textContent).toContain("a 501 the daemon itself sent");
    expect(screen.getByRole("button", { name: "Retry" })).toBeTruthy();
    expect(screen.queryByText(NOT_BUILT)).toBeNull();
  });
});
