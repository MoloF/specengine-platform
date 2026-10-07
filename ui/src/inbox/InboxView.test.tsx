import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ClientError } from "../api/client";
import type { Proposal, QueueEvent } from "../api/types";
import { aProposal, entryOf, noReview } from "../test/builders";
import { renderApp } from "../test/render";
import { argsOf, stubClient, type StubClient } from "../test/stubClient";

// AC-05, AC-07, AC-12, AC-13, AC-14 of docs/features/ui-shell.md, on a stub client.

const QUEUE: Proposal[] = [
  aProposal({ id: "PR-4", severity: "low", created_at: "2026-10-01T09:00:00Z", summary: "Low one" }),
  aProposal({
    id: "PR-1",
    severity: "high",
    kind: "discrepancy",
    target_ids: ["R-1"],
    created_at: "2026-10-01T10:00:00Z",
    summary: "High one",
    options: [
      { label: "Code to spec", effect: "Fix the code", price: "One test" },
      { label: "Spec to code", effect: "Edit the rule", price: "Balance changes" },
    ],
    recommendation: 1,
    diff: "--- base a.md\n+++ proposed a.md\n@@ -1,2 +1,2 @@\n context\n-old line\n+new line",
  }),
  aProposal({ id: "PR-2", severity: "normal", created_at: "2026-10-01T08:00:00Z", summary: "Normal one", target_ids: ["R-1"] }),
  aProposal({
    id: "PR-3",
    severity: "urgent",
    status: "escalated",
    kind: "reconcile",
    created_at: "2026-09-30T08:00:00Z",
    summary: "Urgent one",
  }),
];

async function openInbox(proposals: Proposal[] = QUEUE, hash = "#/alpha/inbox") {
  const client = stubClient(proposals);
  renderApp(client, hash);
  const list = await screen.findByRole("listbox");
  return { client, list };
}

function optionIds(list: HTMLElement): string[] {
  return within(list)
    .getAllByRole("option")
    .map((option) => option.dataset.proposal ?? "");
}

function selectedOption(list: HTMLElement): HTMLElement {
  const selected = within(list)
    .getAllByRole("option")
    .find((option) => option.getAttribute("aria-selected") === "true");
  if (selected === undefined) {
    throw new Error("no option is selected");
  }
  return selected;
}

/** The page's polite live region (src/ui/announcer.tsx), outside the app's root. */
function politeRegion(): HTMLElement {
  const region = document.querySelector<HTMLElement>('[aria-live="polite"]');
  if (region === null) {
    throw new Error("no polite live region");
  }
  return region;
}

function assertiveRegion(): HTMLElement {
  const region = document.querySelector<HTMLElement>('[aria-live="assertive"]');
  if (region === null) {
    throw new Error("no assertive live region");
  }
  return region;
}

/** Whether the element sits in an inert subtree (jsdom keeps `inert` as a plain property). */
function isInert(element: Element): boolean {
  for (let node: Element | null = element; node !== null; node = node.parentElement) {
    if (node instanceof HTMLElement && node.inert) {
      return true;
    }
  }
  return false;
}

/** Holds the stub's next decision until `release()`; then it answers as the stub would. */
function holdNextDecision(client: StubClient): { release: () => void } {
  const answer = client.decideProposal.getMockImplementation();
  if (answer === undefined) {
    throw new Error("the stub has no decideProposal implementation");
  }
  let release: () => void = () => undefined;
  client.decideProposal.mockImplementationOnce(
    (project, id, decision) =>
      new Promise((resolve, reject) => {
        release = () => {
          answer(project, id, decision).then(resolve, reject);
        };
      }),
  );
  return {
    release: () => {
      release();
    },
  };
}

/** Browser Back; jsdom traverses history in a task of its own, so wait for the hashchange it fires. */
async function goBack() {
  const traversed = new Promise<void>((resolve) => {
    window.addEventListener(
      "hashchange",
      () => {
        resolve();
      },
      { once: true },
    );
  });
  await act(async () => {
    window.history.back();
    await traversed;
  });
}

function tabbables(): HTMLElement[] {
  const all = document.body.querySelectorAll<HTMLElement>("a[href], button, input, select, textarea, [tabindex]");
  return Array.from(all).filter((element) => element.tabIndex >= 0 && !element.hasAttribute("disabled"));
}

describe("the Inbox on a stub client", () => {
  it("shows the stub's data and reads it once (AC-05)", async () => {
    const { client, list } = await openInbox();
    expect(optionIds(list)).toHaveLength(4);
    expect(client.getProjects).toHaveBeenCalledTimes(1);
    expect(client.getInbox).toHaveBeenCalledTimes(1);
    expect(argsOf(client.getInbox)).toContainEqual(["alpha"]);
    expect(await screen.findByRole("heading", { level: 2, name: "High one" })).toBeTruthy();
    await screen.findByText("Title of R-1");
    expect(argsOf(client.getNode)).toContainEqual(["alpha", "R-1"]);
    expect(client.decideProposal).not.toHaveBeenCalled();
  });

  it("orders by severity, then age, unknown severity after low, raw text in a neutral badge (AC-07)", async () => {
    const { list } = await openInbox();
    expect(optionIds(list)).toEqual(["PR-1", "PR-2", "PR-4", "PR-3"]);
    const urgent = within(list).getAllByRole("option")[3];
    if (urgent === undefined) {
      throw new Error("no fourth option");
    }
    const badge = urgent.querySelector('[data-tone="severity-unknown"]');
    expect(badge?.textContent).toContain("urgent");
    expect(within(list).getAllByRole("option")[1]?.querySelector('[data-tone="severity-normal"]')).not.toBeNull();
  });

  it("shows an unknown status and kind verbatim in a neutral badge (AC-07)", async () => {
    await openInbox(QUEUE, "#/alpha/inbox/PR-3");
    const card = await screen.findByRole("article");
    const status = card.querySelector('[data-tone="proposal-unknown"]');
    expect(status?.textContent).toContain("escalated");
    expect(within(card).getByText("reconcile")).toBeTruthy();
  });

  it("selects the hash's ID and says when it is absent", async () => {
    await openInbox(QUEUE, "#/alpha/inbox/PR-4");
    expect(await screen.findByRole("heading", { level: 2, name: "Low one" })).toBeTruthy();
    window.history.replaceState(null, "", "/#/alpha/inbox/PR-404");
    await act(async () => {
      window.dispatchEvent(new HashChangeEvent("hashchange"));
      await Promise.resolve();
    });
    expect(await screen.findByText(/PR-404 is not in this inbox/)).toBeTruthy();
    expect(screen.getByRole("heading", { level: 2, name: "High one" })).toBeTruthy();
  });

  it("renders the attached diff with its signs and says when none is attached", async () => {
    await openInbox();
    const diff = await screen.findByLabelText("Section diff");
    const lines = Array.from(diff.querySelectorAll<HTMLElement>("[data-line]"));
    expect(lines.map((line) => line.dataset.line)).toEqual(["file", "file", "hunk", "context", "removed", "added"]);
    expect(lines[4]?.textContent).toContain("-old line");
    expect(lines[5]?.textContent).toContain("+new line");
    fireEvent.click(within(screen.getByRole("listbox")).getByText("Normal one"));
    expect(await screen.findByText("No section diff attached.")).toBeTruthy();
  });

  it("lists other proposals on the same node as information", async () => {
    await openInbox();
    const card = await screen.findByRole("article");
    const others = within(card).getByRole("heading", { name: "Also open on these nodes" }).closest("section");
    expect(others?.textContent).toContain("PR-2");
  });

  it("links each target to the spec tree, its REF encoded once (ui-tree-node)", async () => {
    await openInbox([aProposal({ id: "PR-7", target_ids: ["R-1", "docs/spec/b.md"] })]);
    const card = await screen.findByRole("article");
    await within(card).findAllByText(/Title of/);
    const links = within(card).getAllByRole("link", { name: /^Open in spec tree/ });
    expect(links.map((link) => link.getAttribute("href"))).toEqual(["#/alpha/tree/R-1", "#/alpha/tree/docs%2Fspec%2Fb.md"]);
    expect(links.map((link) => link.textContent)).toEqual(["Open in spec tree: R-1", "Open in spec tree: docs/spec/b.md"]);
  });

  it("counts a proposal naming its node only by target_id as open on the same node", async () => {
    await openInbox([...QUEUE, aProposal({ id: "PR-9", target_id: "R-1", summary: "Single target" })]);
    const card = await screen.findByRole("article");
    const others = within(card).getByRole("heading", { name: "Also open on these nodes" }).closest("section");
    expect(others?.textContent).toContain("PR-9");
    expect(others?.textContent).not.toContain("PR-4");
  });

  it("shows the decision keys in lower case, as they are typed", async () => {
    const { list } = await openInbox();
    const card = await screen.findByRole("article");
    const bar = within(card).getByRole("group", { name: "Decide PR-1" });
    expect(Array.from(bar.querySelectorAll("kbd"), (key) => key.textContent)).toEqual(["a", "r", "c", "d"]);
    expect(within(bar).getAllByRole("button").map((button) => button.getAttribute("aria-keyshortcuts"))).toEqual([
      "a",
      "r",
      "c",
      "d",
    ]);
    fireEvent.keyDown(selectedOption(list), { key: "?" });
    const dialog = await screen.findByRole("dialog", { name: "Keyboard shortcuts" });
    expect(Array.from(dialog.querySelectorAll("kbd"), (key) => key.textContent)).toEqual([
      "j",
      "Down arrow",
      "k",
      "Up arrow",
      "a",
      "r",
      "c",
      "d",
      "Cmd-K",
      "Ctrl-K",
      "?",
      "Esc",
    ]);
    expect(within(dialog).getByText("Needs clarification")).toBeTruthy();
  });
});

describe("the card reads the review document (daemon-read \"Data\")", () => {
  it("lists entries from the inbox and reads the selected one's review by ID, once", async () => {
    const { client } = await openInbox(QUEUE, "#/alpha/inbox/PR-2");
    const card = await screen.findByRole("article");
    await within(card).findByRole("heading", { level: 3, name: "Provenance" });
    expect(argsOf(client.getProposal)).toEqual([["alpha", "PR-2"]]);
    fireEvent.click(within(screen.getByRole("listbox")).getByText("High one"));
    await within(screen.getByRole("article")).findByRole("heading", { level: 3, name: "Options" });
    expect(argsOf(client.getProposal)).toEqual([
      ["alpha", "PR-2"],
      ["alpha", "PR-1"],
    ]);
  });

  it("shows what only the review holds: a question's price, a linked patch, distinct items, the decision record", async () => {
    await openInbox(
      [
        aProposal({
          id: "PR-8",
          kind: "question",
          summary: "Does regeneration wait for rest?",
          working_answer: "Yes, 1.5 s",
          price_of_other: "R-28 rebalanced",
          linked: "PR-9",
          distinct_from: ["DEC-0023", "PR-0003"],
          options: [
            { label: "Wait", effect: "Regeneration waits", price: "None" },
            { label: "Never wait", effect: "Regeneration runs", price: "Balance" },
          ],
          record_id: "DEC-0031",
          record_title: "Regeneration waits for rest",
          record_path: "docs/records/DEC/DEC-0031.md",
          choice: { option: 1 },
        }),
      ],
      "#/alpha/inbox/PR-8",
    );
    const card = await screen.findByRole("article");
    await within(card).findByRole("heading", { level: 3, name: "Provenance" });
    expect(within(card).getByText("R-28 rebalanced")).toBeTruthy();
    expect(within(card).getByRole("link", { name: "PR-9" }).getAttribute("href")).toBe("#/alpha/inbox/PR-9");
    expect(within(card).getByText("DEC-0023, PR-0003")).toBeTruthy();
    const record = within(card).getByRole("heading", { level: 3, name: "Decision record" }).closest("section");
    expect(record?.textContent).toContain("DEC-0031");
    expect(record?.textContent).toContain("docs/records/DEC/DEC-0031.md");
    expect(record?.textContent).toContain("Option 1: Never wait");
    expect(within(card).queryByText(/^T-/)).toBeNull();
  });

  it("does nothing on a decision key or button until the review is read", async () => {
    const client = stubClient(QUEUE);
    let release: () => void = () => undefined;
    const read = client.getProposal.getMockImplementation();
    client.getProposal.mockImplementationOnce(
      (project, id) =>
        new Promise((resolve) => {
          release = () => {
            void read?.(project, id).then(resolve);
          };
        }),
    );
    renderApp(client, "#/alpha/inbox/PR-1");
    const list = await screen.findByRole("listbox");
    const card = await screen.findByRole("article");
    expect(await within(card).findByLabelText("Loading the review of PR-1")).toBeTruthy();
    const accept = within(card).getByRole("button", { name: "Accept" });
    expect(accept.getAttribute("aria-disabled")).toBe("true");
    fireEvent.click(accept);
    fireEvent.keyDown(selectedOption(list), { key: "a" });
    expect(screen.queryByRole("dialog")).toBeNull();
    await act(async () => {
      release();
      await Promise.resolve();
    });
    await waitFor(() => {
      expect(accept.getAttribute("aria-disabled")).toBe("false");
    });
    fireEvent.keyDown(selectedOption(list), { key: "a" });
    expect(await screen.findByRole("dialog", { name: "Accept PR-1" })).toBeTruthy();
  });

  it("shows a review that cannot be read in the daemon's words, Retry reading it again", async () => {
    const client = stubClient(QUEUE);
    const message = "spec: `PR-1` belongs to the repository /work/other (worktree /work/other), which no longer exists";
    client.getProposal.mockRejectedValueOnce(new ClientError({ status: 503, message }));
    renderApp(client, "#/alpha/inbox/PR-1");
    const card = await screen.findByRole("article");
    const alert = await within(card).findByRole("alert");
    expect(alert.textContent).toContain("The review of PR-1 could not be read");
    expect(alert.textContent).toContain(message);
    fireEvent.click(within(card).getByRole("button", { name: "Retry" }));
    await within(card).findByRole("heading", { level: 3, name: "Provenance" });
    expect(client.getProposal).toHaveBeenCalledTimes(2);
  });

  it("shows the exit-1 document's reason when the queue no longer holds the proposal", async () => {
    const client = stubClient(QUEUE);
    client.getProposal.mockResolvedValueOnce(noReview("PR-1"));
    renderApp(client, "#/alpha/inbox/PR-1");
    const card = await screen.findByRole("article");
    expect(await within(card).findByText("PR-1 has no review document in this repository")).toBeTruthy();
    expect(within(card).getByText("no proposal `PR-1` in this project's queue")).toBeTruthy();
    expect(within(card).getByRole("button", { name: "Accept" }).getAttribute("aria-disabled")).toBe("true");
  });

  it("keeps the daemon's 403 in the dialog, its terminal command verbatim (AC-11)", async () => {
    const { client } = await openInbox(QUEUE, "#/alpha/inbox/PR-2");
    const message = "decisions are made on a terminal: `spec approve PR-2` or `spec reject PR-2 --reason …` in /work/alpha; nothing changed";
    client.decideProposal.mockRejectedValueOnce(new ClientError({ status: 403, message }));
    const card = await screen.findByRole("article");
    await within(card).findByRole("heading", { level: 3, name: "Provenance" });
    fireEvent.click(within(card).getByRole("button", { name: "Accept" }));
    const dialog = await screen.findByRole("dialog", { name: "Accept PR-2" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Accept" }));
    const refusal = await within(dialog).findByRole("alert");
    expect(refusal.textContent).toContain("The daemon refused the decision; nothing changed.");
    expect(within(refusal).getByText(message)).toBeTruthy();
    expect(screen.getByRole("dialog", { name: "Accept PR-2" })).toBe(dialog);
  });
});

/** The card's facts as shown: each `dt` with its `dd`'s text. */
function facts(card: HTMLElement): [string, string][] {
  return Array.from(card.querySelectorAll(".facts .fact"), (fact) => [fact.querySelector("dt")?.textContent ?? "", fact.querySelector("dd")?.textContent ?? ""]);
}

describe("the card's Task fact, from the review document (AC-12 of ui-live-tasks, AC-08 of ui-tasks)", () => {
  const BOUND = aProposal({ id: "PR-0041", kind: "discrepancy", summary: "Entry only inside the tide window, see T-0999", task_id: "T-0107" });
  const UNBOUND = aProposal({ id: "PR-0044", summary: "Unbound" });

  it("links a bound proposal's task by the review's task_id, never by its summary; an inbox entry names none", async () => {
    const { client } = await openInbox([BOUND, UNBOUND], "#/alpha/inbox/PR-0041");
    const card = await screen.findByRole("article");
    await within(card).findByRole("heading", { level: 3, name: "Provenance" });
    expect(facts(card).map(([name]) => name)).toEqual(["Kind", "Gap type", "Severity", "Task", "Status"]);
    const task = within(card).getByRole("link", { name: "T-0107" });
    expect(task.getAttribute("href")).toBe("#/alpha/tasks/T-0107");
    expect(task.closest(".fact")?.querySelector("dt")?.textContent).toBe("Task");
    expect(within(card).queryByRole("link", { name: "T-0999" })).toBeNull();
    // InboxEntry stays the daemon's 11 keys: the task comes only from the review document.
    const inbox = await client.getInbox("alpha");
    expect(inbox.proposals.map((entry) => [entry.id, Object.keys(entry).length, "task_id" in entry])).toEqual([
      ["PR-0041", 11, false],
      ["PR-0044", 11, false],
    ]);
  });

  it("says No task for an unbound proposal", async () => {
    await openInbox([BOUND, UNBOUND], "#/alpha/inbox/PR-0044");
    const card = await screen.findByRole("article");
    await within(card).findByRole("heading", { level: 3, name: "Provenance" });
    expect(facts(card)).toContainEqual(["Task", "No task"]);
    expect(within(card).queryByRole("link", { name: /^T-/ })).toBeNull();
  });

  it("shows no Task fact before the review arrives, nor for the exit-1 document", async () => {
    const client = stubClient([BOUND, UNBOUND]);
    let release: () => void = () => undefined;
    const read = client.getProposal.getMockImplementation();
    client.getProposal.mockImplementationOnce(
      (project, id) =>
        new Promise((resolve) => {
          release = () => {
            void read?.(project, id).then(resolve);
          };
        }),
    );
    renderApp(client, "#/alpha/inbox/PR-0041");
    const card = await screen.findByRole("article");
    expect(await within(card).findByLabelText("Loading the review of PR-0041")).toBeTruthy();
    expect(facts(card).map(([name]) => name)).toEqual(["Kind", "Severity", "Status"]);
    await act(async () => {
      release();
      await Promise.resolve();
    });
    expect(await within(card).findByRole("link", { name: "T-0107" })).toBeTruthy();

    client.getProposal.mockResolvedValueOnce(noReview("PR-0044"));
    fireEvent.click(within(screen.getByRole("listbox")).getByText("Unbound"));
    const gone = await screen.findByRole("article");
    expect(await within(gone).findByText("PR-0044 has no review document in this repository")).toBeTruthy();
    expect(facts(gone).map(([name]) => name)).not.toContain("Task");
  });
});

describe("loading, empty and error states (AC-12)", () => {
  it("shows a busy skeleton while the inbox loads", async () => {
    const client = stubClient(QUEUE);
    client.getInbox.mockImplementation(() => new Promise(() => undefined));
    renderApp(client, "#/alpha/inbox");
    const skeleton = await screen.findByLabelText("Loading the inbox of alpha");
    expect(skeleton.getAttribute("aria-busy")).toBe("true");
    expect(screen.getByRole("region", { name: "Inbox" }).getAttribute("aria-busy")).toBe("true");
  });

  it("explains an empty queue and names the next step", async () => {
    renderApp(stubClient([]), "#/alpha/inbox");
    expect(await screen.findByRole("heading", { name: "The queue is clear" })).toBeTruthy();
    expect(screen.getByText(/waits for your decision/)).toBeTruthy();
    const next = screen.getByRole("link", { name: /approve the tasks/ });
    expect(next.getAttribute("href")).toBe("#/alpha/tasks");
  });

  it("shows the daemon's message verbatim and Retry reads the inbox again", async () => {
    const client = stubClient(QUEUE);
    const message = "spec index unavailable: the database is locked (code 5)";
    client.getInbox.mockRejectedValue(new ClientError({ status: 503, message }));
    renderApp(client, "#/alpha/inbox");
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toBe(`The inbox could not be loaded${message}`);
    expect(client.getInbox).toHaveBeenCalledTimes(1);
    const retry = screen.getByRole("button", { name: "Retry" });
    expect(alert.contains(retry)).toBe(false);
    fireEvent.click(retry);
    await waitFor(() => {
      expect(client.getInbox).toHaveBeenCalledTimes(2);
    });
    expect(await screen.findByText(message)).toBeTruthy();
  });

  it("ignores Retry while retrying, and a failed retry leaves focus on Retry", async () => {
    const client = stubClient(QUEUE);
    let fail: () => void = () => undefined;
    client.getInbox
      .mockRejectedValueOnce(new ClientError({ status: 503, message: "index locked" }))
      .mockImplementationOnce(
        () =>
          new Promise((_resolve, reject) => {
            fail = () => {
              reject(new ClientError({ status: 503, message: "index still locked" }));
            };
          }),
      );
    renderApp(client, "#/alpha/inbox");
    const alert = await screen.findByRole("alert");
    const retry = screen.getByRole("button", { name: "Retry" });
    retry.focus();
    fireEvent.click(retry);
    await waitFor(() => {
      expect(retry.getAttribute("aria-disabled")).toBe("true");
    });
    expect(client.getInbox).toHaveBeenCalledTimes(2);
    fireEvent.click(retry);
    fireEvent.click(retry);
    expect(client.getInbox).toHaveBeenCalledTimes(2);
    expect(document.activeElement).toBe(retry);
    // Retrying: the button says so outside the alert, which stays the same node, not spoken again.
    expect(retry.textContent).toBe("Retrying");
    expect(screen.getByRole("alert")).toBe(alert);
    expect(alert.textContent).toBe("The inbox could not be loadedindex locked");
    await act(async () => {
      fail();
      await Promise.resolve();
    });
    expect(await screen.findByText("index still locked")).toBeTruthy();
    expect(retry.getAttribute("aria-disabled")).toBe("false");
    expect(document.activeElement).toBe(retry);
    // The failed retry is a new alert, spoken again.
    expect(alert.isConnected).toBe(false);
    expect(screen.getByRole("alert").textContent).toBe("The inbox could not be loadedindex still locked");
  });

  it("puts focus on the selected item after a successful Retry, where the queue's keys act", async () => {
    const client = stubClient(QUEUE);
    client.getInbox.mockRejectedValueOnce(new ClientError({ status: 503, message: "index locked" }));
    renderApp(client, "#/alpha/inbox");
    await screen.findByRole("alert");
    const retry = screen.getByRole("button", { name: "Retry" });
    retry.focus();
    fireEvent.click(retry);
    const list = await screen.findByRole("listbox");
    await waitFor(() => {
      expect(document.activeElement).toBe(selectedOption(list));
    });
    expect(selectedOption(list).dataset.proposal).toBe("PR-1");
    fireEvent.keyDown(selectedOption(list), { key: "j" });
    await waitFor(() => {
      expect(selectedOption(list).dataset.proposal).toBe("PR-2");
    });
  });

  it("puts focus on the Inbox heading after a successful Retry finds the queue clear", async () => {
    const client = stubClient([]);
    client.getInbox.mockRejectedValueOnce(new ClientError({ status: 503, message: "index locked" }));
    renderApp(client, "#/alpha/inbox");
    await screen.findByRole("alert");
    const retry = screen.getByRole("button", { name: "Retry" });
    retry.focus();
    fireEvent.click(retry);
    expect(await screen.findByRole("heading", { name: "The queue is clear" })).toBeTruthy();
    expect(document.activeElement).toBe(screen.getByRole("heading", { level: 1, name: "Inbox" }));
  });

  it("puts focus on the target's head after a successful Retry of its section", async () => {
    const client = stubClient(QUEUE);
    client.getNode.mockRejectedValueOnce(new ClientError({ status: 503, message: "node index locked" }));
    renderApp(client, "#/alpha/inbox/PR-1");
    const card = await screen.findByRole("article");
    const alert = await within(card).findByRole("alert");
    expect(alert.textContent).toBe("The current section of R-1 could not be readnode index locked");
    const retry = within(card).getByRole("button", { name: "Retry" });
    expect(alert.contains(retry)).toBe(false);
    retry.focus();
    fireEvent.click(retry);
    const head = (await within(card).findByText("Title of R-1")).closest("p");
    await waitFor(() => {
      expect(document.activeElement).toBe(head);
    });
    if (head === null) {
      throw new Error("no target head");
    }
    fireEvent.keyDown(head, { key: "d" });
    expect(await screen.findByRole("dialog", { name: "Defer PR-1" })).toBeTruthy();
  });
});

describe("keyboard (AC-13)", () => {
  it("tabs through skip link, header, nav, list as one stop, then the card", async () => {
    await openInbox();
    await screen.findByRole("article");
    const order = tabbables();
    const at = (element: HTMLElement) => order.indexOf(element);
    const skip = screen.getByRole("link", { name: "Skip to content" });
    const project = screen.getByLabelText("Project");
    const shortcuts = screen.getByRole("button", { name: "Keyboard shortcuts" });
    const nav = within(screen.getByRole("navigation", { name: "Sections" })).getAllByRole("link");
    const filter = screen.getByLabelText("Filter");
    const options = within(screen.getByRole("listbox")).getAllByRole("option");
    const accept = within(screen.getByRole("article")).getByRole("button", { name: "Accept" });
    expect(at(skip)).toBe(0);
    expect(at(project)).toBeGreaterThan(at(skip));
    expect(at(shortcuts)).toBeGreaterThan(at(project));
    const navAt = nav.map(at);
    expect(navAt).toEqual([...navAt].sort((a, b) => a - b));
    expect(navAt[0]).toBeGreaterThan(at(shortcuts));
    expect(at(filter)).toBeGreaterThan(navAt[navAt.length - 1] ?? Infinity);
    const tabbableOptions = options.filter((option) => order.includes(option));
    expect(tabbableOptions).toHaveLength(1);
    const listStop = tabbableOptions[0];
    if (listStop === undefined) {
      throw new Error("no tabbable option");
    }
    expect(at(listStop)).toBeGreaterThan(at(filter));
    expect(at(accept)).toBeGreaterThan(at(listStop));
  });

  it("moves with the arrows and with j and k", async () => {
    const { list } = await openInbox();
    selectedOption(list).focus();
    fireEvent.keyDown(document.activeElement ?? list, { key: "ArrowDown" });
    await waitFor(() => {
      expect(selectedOption(list).dataset.proposal).toBe("PR-2");
    });
    expect(document.activeElement).toBe(selectedOption(list));
    fireEvent.keyDown(selectedOption(list), { key: "j" });
    await waitFor(() => {
      expect(selectedOption(list).dataset.proposal).toBe("PR-4");
    });
    fireEvent.keyDown(selectedOption(list), { key: "k" });
    await waitFor(() => {
      expect(selectedOption(list).dataset.proposal).toBe("PR-2");
    });
    fireEvent.keyDown(selectedOption(list), { key: "ArrowUp" });
    await waitFor(() => {
      expect(selectedOption(list).dataset.proposal).toBe("PR-1");
    });
    expect(document.activeElement).toBe(selectedOption(list));
    expect(window.location.hash).toBe("#/alpha/inbox/PR-1");
  });

  it("opens nothing for a letter typed in the filter, or with a modifier", async () => {
    const { list } = await openInbox();
    const filter = screen.getByLabelText("Filter");
    filter.focus();
    fireEvent.keyDown(filter, { key: "a" });
    expect(screen.queryByRole("dialog")).toBeNull();
    fireEvent.keyDown(selectedOption(list), { key: "a", ctrlKey: true });
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("acts only with focus in the queue: a key pressed in the header or the nav opens nothing", async () => {
    await openInbox();
    const shortcuts = screen.getByRole("button", { name: "Keyboard shortcuts" });
    shortcuts.focus();
    fireEvent.keyDown(shortcuts, { key: "a" });
    const inbox = within(screen.getByRole("navigation", { name: "Sections" })).getByRole("link", { name: "Inbox" });
    fireEvent.keyDown(inbox, { key: "r" });
    fireEvent.keyDown(document.body, { key: "d" });
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("opens a decision with its key; Esc closes it and focus returns; Tab stays inside", async () => {
    const { list } = await openInbox();
    const trigger = selectedOption(list);
    trigger.focus();
    fireEvent.keyDown(trigger, { key: "a" });
    const dialog = await screen.findByRole("dialog", { name: "Accept PR-1" });
    expect(dialog.getAttribute("aria-modal")).toBe("true");
    await waitFor(() => {
      expect(dialog.contains(document.activeElement)).toBe(true);
    });
    const recommended = within(dialog).getByRole("radio", { name: /Spec to code/ });
    expect((recommended as HTMLInputElement).checked).toBe(true);

    const first = recommended;
    const buttons = within(dialog).getAllByRole("button");
    const last = buttons[buttons.length - 1];
    if (last === undefined) {
      throw new Error("the dialog holds no button");
    }
    expect(last.textContent).toBe("Cancel");
    last.focus();
    fireEvent.keyDown(last, { key: "Tab" });
    expect(document.activeElement).toBe(first);
    fireEvent.keyDown(first, { key: "Tab", shiftKey: true });
    expect(document.activeElement).toBe(last);

    fireEvent.keyDown(last, { key: "Escape" });
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect(document.activeElement).toBe(trigger);
  });

  it("lists the shortcuts on ?", async () => {
    const { list } = await openInbox();
    fireEvent.keyDown(selectedOption(list), { key: "?" });
    expect(await screen.findByRole("dialog", { name: "Keyboard shortcuts" })).toBeTruthy();
  });

  it("closes a dialog on Esc pressed anywhere in it, and a press on the scrim keeps focus inside", async () => {
    const { list } = await openInbox();
    const trigger = selectedOption(list);
    trigger.focus();
    fireEvent.keyDown(trigger, { key: "d" });
    const dialog = await screen.findByRole("dialog", { name: "Defer PR-1" });
    const scrim = document.querySelector("[data-dialog-scrim]");
    const host = document.querySelector("[data-dialog-host]");
    if (scrim === null || host === null) {
      throw new Error("no dialog host or scrim");
    }
    const focused = document.activeElement;
    expect(dialog.contains(focused)).toBe(true);
    expect(fireEvent.mouseDown(scrim)).toBe(false);
    fireEvent.keyDown(host, { key: "Escape" });
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect(document.activeElement).toBe(trigger);
  });
});

describe("decisions (AC-14)", () => {
  async function openDialog(name: string, id = "PR-1") {
    const opened = await openInbox(QUEUE, `#/alpha/inbox/${id}`);
    const card = await screen.findByRole("article");
    fireEvent.click(within(card).getByRole("button", { name }));
    const dialog = await screen.findByRole("dialog");
    return { ...opened, dialog };
  }

  it("refuses an empty reason without calling, with a message", async () => {
    const { client, dialog } = await openDialog("Reject");
    const submit = within(dialog).getByRole("button", { name: "Reject" });
    fireEvent.click(submit);
    const missing = await within(dialog).findByText(/Write a reason/);
    const reason = within(dialog).getByLabelText("Reason (required)");
    expect(reason.getAttribute("aria-invalid")).toBe("true");
    expect(client.decideProposal).not.toHaveBeenCalled();
    // The alert speaks the message; the focused field does not repeat it until focus comes back.
    expect(missing.getAttribute("role")).toBe("alert");
    expect(document.activeElement).toBe(reason);
    expect(reason.getAttribute("aria-describedby")).toBeNull();
    act(() => {
      submit.focus();
    });
    act(() => {
      reason.focus();
    });
    expect(reason.getAttribute("aria-describedby")).toBe(missing.id);
  });

  it("refuses an empty clarification note without calling", async () => {
    const { client, dialog } = await openDialog("Needs clarification");
    expect(dialog.querySelector("h2")?.textContent).toBe("Needs clarification: PR-1");
    fireEvent.change(within(dialog).getByRole("textbox"), { target: { value: "   " } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Needs clarification" }));
    expect(await within(dialog).findByText(/Write what the author should clarify/)).toBeTruthy();
    expect(client.decideProposal).not.toHaveBeenCalled();
  });

  it("sends one call for a double click and for two submits in one tick", async () => {
    const { client, dialog } = await openDialog("Reject");
    const held = holdNextDecision(client);
    fireEvent.change(within(dialog).getByRole("textbox"), { target: { value: "Duplicate of PR-2" } });
    const submit = within(dialog).getByRole("button", { name: "Reject" });
    const form = submit.closest("form");
    if (form === null) {
      throw new Error("no form");
    }
    act(() => {
      fireEvent.submit(form);
      fireEvent.submit(form);
    });
    fireEvent.click(submit);
    fireEvent.click(submit);
    await waitFor(() => {
      expect(client.decideProposal).toHaveBeenCalled();
    });
    expect(submit.getAttribute("aria-disabled")).toBe("true");
    expect(submit.textContent).toBe("Sending");
    fireEvent.click(submit);
    await act(async () => {
      held.release();
      await Promise.resolve();
    });
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    // Everything the extra submits could have started has run by now: still the one call.
    expect(client.decideProposal).toHaveBeenCalledTimes(1);
    expect(client.decideProposal).toHaveBeenCalledWith("alpha", "PR-1", { decision: "reject", reason: "Duplicate of PR-2" });
  });

  it("keeps the dialog open while a decision is sent: Esc, Cancel and the scrim do nothing", async () => {
    const { client, dialog } = await openDialog("Defer");
    const held = holdNextDecision(client);
    const submit = within(dialog).getByRole("button", { name: "Defer" });
    submit.focus();
    fireEvent.click(submit);
    await waitFor(() => {
      expect(client.decideProposal).toHaveBeenCalledTimes(1);
    });
    const cancel = within(dialog).getByRole("button", { name: "Cancel" });
    expect(submit.getAttribute("aria-disabled")).toBe("true");
    expect(cancel.getAttribute("aria-disabled")).toBe("true");
    expect(document.activeElement).toBe(submit);
    fireEvent.keyDown(submit, { key: "Escape" });
    const host = document.querySelector("[data-dialog-host]");
    const scrim = document.querySelector("[data-dialog-scrim]");
    if (host === null || scrim === null) {
      throw new Error("no dialog host or scrim");
    }
    fireEvent.keyDown(host, { key: "Escape" });
    fireEvent.click(cancel);
    fireEvent.mouseDown(scrim);
    fireEvent.click(scrim);
    expect(screen.getByRole("dialog")).toBe(dialog);
    expect(document.activeElement).toBe(submit);
    expect(politeRegion().textContent).toBe("");
    await act(async () => {
      held.release();
      await Promise.resolve();
    });
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect(politeRegion().textContent).toBe("Deferred PR-1.");
    expect(client.decideProposal).toHaveBeenCalledTimes(1);
  });

  it("opens no second decision while one is sent: the keys and the decision bar do nothing", async () => {
    const { client, dialog, list } = await openDialog("Defer");
    const held = holdNextDecision(client);
    const bar = within(screen.getByRole("article")).getByRole("group", { name: "Decide PR-1" });
    fireEvent.click(within(bar).getByRole("button", { name: "Reject" }));
    expect(screen.getAllByRole("dialog")).toEqual([dialog]);
    fireEvent.click(within(dialog).getByRole("button", { name: "Defer" }));
    await waitFor(() => {
      expect(client.decideProposal).toHaveBeenCalledTimes(1);
    });
    expect(within(bar).getByRole("button", { name: "Reject" }).getAttribute("aria-disabled")).toBe("true");
    fireEvent.keyDown(selectedOption(list), { key: "c" });
    fireEvent.keyDown(selectedOption(list), { key: "a" });
    fireEvent.click(within(bar).getByRole("button", { name: "Reject" }));
    fireEvent.click(within(bar).getByRole("button", { name: "Accept" }));
    expect(screen.getAllByRole("dialog")).toEqual([dialog]);
    expect(dialog.querySelector("h2")?.textContent).toBe("Defer PR-1");
    await act(async () => {
      held.release();
      await Promise.resolve();
    });
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect(client.decideProposal).toHaveBeenCalledTimes(1);
    expect(client.decideProposal).toHaveBeenCalledWith("alpha", "PR-1", { decision: "defer", note: null });
  });

  it("speaks the result from a live region the open dialog leaves out of the inert page", async () => {
    const { dialog, list } = await openDialog("Defer");
    expect(isInert(list)).toBe(true);
    expect(isInert(politeRegion())).toBe(false);
    expect(isInert(assertiveRegion())).toBe(false);
    expect(dialog.closest("[data-dialog-host]")?.contains(politeRegion())).toBe(false);
    fireEvent.click(within(dialog).getByRole("button", { name: "Defer" }));
    await waitFor(() => {
      expect(politeRegion().textContent).toBe("Deferred PR-1.");
    });
    expect(isInert(politeRegion())).toBe(false);
    expect(isInert(list)).toBe(false);
  });

  it("announces an accepted proposal with the commit's sha and subject and focuses the next item", async () => {
    const { client, dialog, list } = await openDialog("Accept");
    fireEvent.change(within(dialog).getByLabelText("Note for the record (optional)"), { target: { value: "ok" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Accept" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect(client.decideProposal).toHaveBeenCalledWith("alpha", "PR-1", { decision: "accept", option: 1, note: "ok" });
    expect(politeRegion().textContent).toBe('Accepted PR-1: committed c0ffee1 "spec: apply PR-1".');
    await waitFor(() => {
      expect(optionIds(list)).not.toContain("PR-1");
    });
    expect(document.activeElement).toBe(selectedOption(list));
    expect(selectedOption(list).dataset.proposal).toBe("PR-2");
  });

  it("drops an accepted proposal from the list at once, before the inbox is read again", async () => {
    const { client, dialog, list } = await openDialog("Accept");
    client.getInbox.mockImplementation(() => new Promise(() => undefined));
    fireEvent.click(within(dialog).getByRole("button", { name: "Accept" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    await waitFor(() => {
      expect(optionIds(list)).toEqual(["PR-2", "PR-4", "PR-3"]);
    });
    expect(client.getInbox).toHaveBeenCalledTimes(2);
    expect(list.querySelector('[data-tone="proposal-applied"]')).toBeNull();
  });

  it("keeps the dialog and the typed text when the daemon refuses, showing its words", async () => {
    const { client, dialog } = await openDialog("Reject");
    const message = "PR-1 cannot be rejected: the worktree is gone (exit 2)";
    client.decideProposal.mockRejectedValueOnce(new ClientError({ status: 422, message }));
    const reason = within(dialog).getByRole("textbox");
    fireEvent.change(reason, { target: { value: "Out of scope for T-7" } });
    const submit = within(dialog).getByRole("button", { name: "Reject" });
    submit.focus();
    fireEvent.click(submit);
    expect(await within(dialog).findByText(message)).toBeTruthy();
    expect(screen.getByRole("dialog")).toBe(dialog);
    expect((reason as HTMLTextAreaElement).value).toBe("Out of scope for T-7");
    expect(submit.getAttribute("aria-disabled")).toBe("false");
    expect(submit.textContent).toBe("Reject");
  });

  it("puts focus back in the text field after a refusal, where Esc still closes the dialog", async () => {
    const { client, dialog } = await openDialog("Reject");
    const message = "PR-1 cannot be rejected: the index is being rebuilt";
    client.decideProposal.mockRejectedValueOnce(new ClientError({ status: 503, message }));
    const reason = within(dialog).getByRole("textbox");
    fireEvent.change(reason, { target: { value: "Out of scope" } });
    const submit = within(dialog).getByRole("button", { name: "Reject" });
    submit.focus();
    fireEvent.click(submit);
    const refusal = (await within(dialog).findByText(message)).closest('[role="alert"]');
    expect(document.activeElement).toBe(reason);
    // Spoken once, by the alert: the field the focus moved to leaves it out of its description...
    expect(reason.getAttribute("aria-describedby")).toBeNull();
    act(() => {
      submit.focus();
    });
    act(() => {
      reason.focus();
    });
    // ...until the owner comes back to the field.
    expect(reason.getAttribute("aria-describedby")?.split(" ")).toContain(refusal?.id);
    fireEvent.keyDown(reason, { key: "Escape" });
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect(client.decideProposal).toHaveBeenCalledTimes(1);
  });

  it("on 409 closes the dialog, shows the message, reads the inbox again and the proposal is gone", async () => {
    const { client, dialog, list } = await openDialog("Defer");
    const message = "PR-1 is no longer open: another session applied it as 4be1f0c";
    client.decideProposal.mockImplementationOnce(() => {
      client.state.proposals = client.state.proposals.filter((proposal) => proposal.id !== "PR-1");
      return Promise.reject(new ClientError({ status: 409, message }));
    });
    fireEvent.click(within(dialog).getByRole("button", { name: "Defer" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect(await screen.findByText(message)).toBeTruthy();
    expect(assertiveRegion().textContent).toBe(`PR-1 was decided elsewhere; the inbox is read again. ${message}`);
    await waitFor(() => {
      expect(client.getInbox).toHaveBeenCalledTimes(2);
    });
    await waitFor(() => {
      expect(optionIds(list)).not.toContain("PR-1");
    });
  });

  it("keeps a deferred proposal in the queue with its new status", async () => {
    const { client, dialog } = await openDialog("Defer");
    fireEvent.click(within(dialog).getByRole("button", { name: "Defer" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect(client.decideProposal).toHaveBeenCalledWith("alpha", "PR-1", { decision: "defer", note: null });
    expect(politeRegion().textContent).toBe("Deferred PR-1.");
    const list = screen.getByRole("listbox");
    await waitFor(() => {
      const deferred = within(list)
        .getAllByRole("option")
        .find((option) => option.dataset.proposal === "PR-1");
      expect(deferred?.querySelector('[data-tone="proposal-deferred"]')).not.toBeNull();
    });
  });

  /** From Tasks into the Inbox (a history entry), a Defer on PR-1 sent, then browser Back to Tasks. */
  async function leaveWhileSending(client: StubClient) {
    renderApp(client, "#/alpha/tasks");
    await screen.findByRole("heading", { level: 1, name: "Tasks" });
    const inboxLink = within(screen.getByRole("navigation", { name: "Sections" })).getByRole("link", { name: "Inbox" });
    fireEvent.click(inboxLink);
    const card = await screen.findByRole("article");
    // A decision needs the review document: its sections are read before the buttons act.
    await within(card).findByRole("heading", { level: 3, name: "Provenance" });
    fireEvent.click(within(card).getByRole("button", { name: "Defer" }));
    const dialog = await screen.findByRole("dialog", { name: "Defer PR-1" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Defer" }));
    await waitFor(() => {
      expect(client.decideProposal).toHaveBeenCalledTimes(1);
    });
    expect(window.location.hash).toBe("#/alpha/inbox");
    await goBack();
    const tasks = await screen.findByRole("heading", { level: 1, name: "Tasks" });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(window.location.hash).toBe("#/alpha/tasks");
    await waitFor(() => {
      expect(document.activeElement).toBe(tasks);
    });
    return tasks;
  }

  it("speaks a decision answered after the owner went Back, and neither moves the hash nor focus", async () => {
    const client = stubClient(QUEUE);
    const held = holdNextDecision(client);
    const tasks = await leaveWhileSending(client);
    await act(async () => {
      held.release();
      await Promise.resolve();
    });
    await waitFor(() => {
      expect(politeRegion().textContent).toBe("Deferred PR-1.");
    });
    expect(window.location.hash).toBe("#/alpha/tasks");
    expect(screen.getByRole("heading", { level: 1, name: "Tasks" })).toBe(tasks);
    expect(document.activeElement).toBe(tasks);
    expect(client.decideProposal).toHaveBeenCalledTimes(1);
  });

  it("speaks a refusal answered after the owner went Back, in the daemon's words", async () => {
    const client = stubClient(QUEUE);
    const message = "PR-1 cannot be deferred: the index is being rebuilt";
    let refuse: () => void = () => undefined;
    client.decideProposal.mockImplementationOnce(
      () =>
        new Promise((_resolve, reject) => {
          refuse = () => {
            reject(new ClientError({ status: 503, message }));
          };
        }),
    );
    const tasks = await leaveWhileSending(client);
    await act(async () => {
      refuse();
      await Promise.resolve();
    });
    await waitFor(() => {
      expect(assertiveRegion().textContent).toBe(`The daemon refused the decision on PR-1; nothing changed. ${message}`);
    });
    expect(window.location.hash).toBe("#/alpha/tasks");
    expect(document.activeElement).toBe(tasks);
  });

  it("takes a revision that arrives while the dialog is open: says so, keeps the text, waits for a new submit", async () => {
    const client = stubClient(QUEUE);
    // The live tail: an event on PR-1 reads its review document again while the dialog is open.
    let emit: (event: QueueEvent) => void = () => undefined;
    client.subscribe = (_project, onEvent) => {
      emit = onEvent;
      return () => undefined;
    };
    renderApp(client, "#/alpha/inbox/PR-1");
    const list = await screen.findByRole("listbox");
    const card = await screen.findByRole("article");
    await within(card).findByRole("heading", { level: 3, name: "Provenance" });
    fireEvent.click(within(card).getByRole("button", { name: "Defer" }));
    fireEvent.click(within(await screen.findByRole("dialog")).getByRole("button", { name: "Defer" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    await waitFor(() => {
      expect(document.activeElement).toBe(selectedOption(list));
    });
    expect(client.getInbox).toHaveBeenCalledTimes(2);
    fireEvent.keyDown(selectedOption(list), { key: "k" });
    await waitFor(() => {
      expect(selectedOption(list).dataset.proposal).toBe("PR-1");
    });
    await waitFor(() => {
      expect(within(screen.getByRole("article")).getByRole("button", { name: "Accept" }).getAttribute("aria-disabled")).toBe("false");
    });
    fireEvent.keyDown(selectedOption(list), { key: "a" });
    const dialog = await screen.findByRole("dialog", { name: "Accept PR-1" });
    const opened = within(dialog).getByRole("radio", { name: /Spec to code/ });
    await waitFor(() => {
      expect(document.activeElement).toBe(opened);
    });
    const note = within(dialog).getByLabelText("Note for the record (optional)");
    fireEvent.change(note, { target: { value: "Matches the tide table" } });

    // Elsewhere the author revised PR-1: a new updated_at, the options in a new order.
    client.state.proposals = client.state.proposals.map((proposal) =>
      proposal.id === "PR-1"
        ? {
            ...proposal,
            updated_at: "2026-10-01T11:00:00Z",
            summary: "High one, revised",
            options: [
              { label: "Spec to code", effect: "Edit the rule", price: "Balance changes" },
              { label: "Code to spec", effect: "Fix the code", price: "One test" },
              { label: "Drop the rule", effect: "Remove it", price: "A review" },
            ],
            recommendation: 2,
          }
        : proposal,
    );
    act(() => {
      emit({ seq: 7, type: "proposal.apply_failed", payload: { id: "PR-1", step: 3, reason: "elsewhere" } });
    });
    const notice = (await within(dialog).findByText("This proposal changed since you opened it")).closest('[role="alert"]');
    expect(notice).not.toBeNull();
    expect(within(dialog).getByText("High one, revised")).toBeTruthy();
    expect(within(dialog).getAllByRole("radio")).toHaveLength(3);
    expect((note as HTMLTextAreaElement).value).toBe("Matches the tide table");
    // The old choice was index 1 of the old list; the new list starts again from its recommendation.
    const recommended = within(dialog).getByRole("radio", { name: /Drop the rule/ });
    expect((recommended as HTMLInputElement).checked).toBe(true);
    expect(document.activeElement).toBe(recommended);
    expect(client.decideProposal).toHaveBeenCalledTimes(1);

    fireEvent.click(within(dialog).getByRole("button", { name: "Accept" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect(client.decideProposal).toHaveBeenCalledTimes(2);
    expect(client.decideProposal).toHaveBeenLastCalledWith("alpha", "PR-1", {
      decision: "accept",
      option: 2,
      note: "Matches the tide table",
    });
  });

  it("shows no change notice for a re-read that leaves the open proposal as it was", async () => {
    const { client, list } = await openInbox(QUEUE, "#/alpha/inbox/PR-2");
    let answer: () => void = () => undefined;
    client.getInbox.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          answer = () => {
            // Equal proposals, new objects; the note shows that the read has landed.
            resolve({ proposals: client.state.proposals.map(entryOf), notes: ["Read again."] });
          };
        }),
    );
    const card = await screen.findByRole("article");
    fireEvent.click(within(card).getByRole("button", { name: "Defer" }));
    fireEvent.click(within(await screen.findByRole("dialog")).getByRole("button", { name: "Defer" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    await waitFor(() => {
      expect(document.activeElement).toBe(selectedOption(list));
    });
    fireEvent.keyDown(selectedOption(list), { key: "r" });
    const dialog = await screen.findByRole("dialog", { name: "Reject PR-4" });
    await act(async () => {
      answer();
      await Promise.resolve();
    });
    expect(await screen.findByText("Read again.")).toBeTruthy();
    expect(within(dialog).queryByText("This proposal changed since you opened it")).toBeNull();
    expect(dialog.querySelector('[role="alert"]')).toBeNull();
  });
});
