import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ClientError, NOT_SERVED } from "../api/client";
import { renderApp } from "../test/render";
import { stubClient } from "../test/stubClient";

// R-n7 of the daemon-read review: a read the daemon does not serve yet is not built, not broken.
// Its screen says so with the client's message verbatim and offers no Retry; a daemon down (no
// response, status 0) on the same screen is an alert with Retry. Since ui-live-tasks the daemon
// serves every read the UI makes (`docs/features/ui-live-tasks.md` "Open", WA-5): the mark stays
// for the next missing endpoint, shown here on a stub client that refuses the tasks as a client
// would refuse such a read. The served tasks: liveScreens.test.tsx.

const NOT_BUILT = "Not built yet: the daemon has no endpoint for this read";

/** What a client says of a read it refuses unsent, the endpoint named. */
function refusedUnsent(endpoint: string): string {
  return `Not served by the daemon yet: ${endpoint} is a missing endpoint. The mock serves it: open the UI with ?scenario=normal.`;
}

/** A stub client refusing the tasks and a task unsent, as not served. */
function notServingTasks() {
  const client = stubClient();
  client.getTasks.mockImplementation((project) =>
    Promise.reject(new ClientError({ status: NOT_SERVED, message: refusedUnsent(`GET /api/projects/${project}/tasks`) }, { notServed: true })),
  );
  client.getTask.mockImplementation((project, id) =>
    Promise.reject(new ClientError({ status: NOT_SERVED, message: refusedUnsent(`GET /api/projects/${project}/tasks/${id}`) }, { notServed: true })),
  );
  return client;
}

/** The notes saying a read is not built, found by their title, in page order. */
async function notBuilt(count: number): Promise<HTMLElement[]> {
  await waitFor(() => {
    expect(screen.queryAllByText(NOT_BUILT)).toHaveLength(count);
  });
  return screen.getAllByText(NOT_BUILT).map((title) => {
    const note = title.closest<HTMLElement>(".notice");
    if (note === null) {
      throw new Error("the title sits in no notice");
    }
    return note;
  });
}

describe("a read the daemon does not serve yet (R-n7)", () => {
  it.each([
    ["Tasks", "#/alpha/tasks", ["GET /api/projects/alpha/tasks"]],
    // The list beside the task reads too: each region says it on its own.
    ["a task", "#/alpha/tasks/T-0001", ["GET /api/projects/alpha/tasks", "GET /api/projects/alpha/tasks/T-0001"]],
    ["the home's Tasks region", "#/alpha", ["GET /api/projects/alpha/tasks"]],
  ])("%s: not built, the client's message verbatim, no Retry, no alert", async (_name, hash, endpoints) => {
    renderApp(notServingTasks(), hash);
    const notes = await notBuilt(endpoints.length);
    expect(notes.map((note) => note.getAttribute("role"))).toEqual(endpoints.map(() => "status"));
    expect(notes.map((note) => within(note).getByText(/^Not served by the daemon yet: /).textContent)).toEqual(endpoints.map(refusedUnsent));
    expect(screen.queryByRole("button", { name: /^Retry/ })).toBeNull();
    expect(screen.queryAllByRole("alert").filter((alert) => alert.textContent.includes("Not served"))).toEqual([]);
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
