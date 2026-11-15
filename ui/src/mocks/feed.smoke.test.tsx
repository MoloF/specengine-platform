import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeAll, describe, expect, it, vi } from "vitest";
import type { QueueEvent } from "../api/types";
import { App } from "../app/App";
import { HARBOR_RULE } from "./feed";
import { MockClient, MORNING_MS } from "./MockClient";
import type { Scenario } from "./scenario";

// docs/features/auto-feed.md "The UI" over the real mock, as `pnpm dev` shows it at
// `?scenario=normal#/harbor-sim/feed`: harbor-sim's night under auto mode (applies and approvals by
// rule, a held land landed after, holds, a failed run after a usage limit's three quiet hours, an
// uncovered write), its Needs me, a department's cut, a delegated apply opened from the feed;
// ledger-api's manual events with auto off; `empty`, nothing yet.

/** The mock's clock: the night's times are relative to it. */
const NOW = Date.parse("2026-11-16T08:00:00Z");

async function renderMock(scenario: Scenario, hash: string, heading = "Activity") {
  window.history.replaceState(null, "", `/?scenario=${scenario}${hash}`);
  const client = new MockClient(scenario, { delayMs: 0, now: () => NOW });
  render(<App client={client} scenario={scenario === "normal" ? null : scenario} />);
  await screen.findByRole("heading", { level: 1, name: heading });
  return client;
}

function seqs(): string[] {
  return Array.from(document.querySelectorAll<HTMLElement>(".feed-entry")).map((entry) => entry.dataset.seq ?? "");
}

function entry(seq: number): HTMLElement {
  const found = document.querySelector<HTMLElement>(`.feed-entry[data-seq="${String(seq)}"]`);
  if (found === null) {
    throw new Error(`no entry ${String(seq)}`);
  }
  return found;
}

beforeAll(async () => {
  await import("../feed/FeedView");
});

describe("the mock's feed (docs/features/auto-feed.md \"The UI\")", () => {
  it("normal, harbor-sim: the night newest first, auto on with its rule, the applies by rule, the holds, the failed run and the three quiet hours", async () => {
    await renderMock("normal", "#/harbor-sim/feed");
    await waitFor(() => {
      expect(seqs()).toHaveLength(26);
    });
    expect(seqs().slice(0, 3)).toEqual(["1206", "1205", "1204"]);
    expect(document.querySelector(".feed-auto")?.textContent).toBe(`Auto: on, rule ${HARBOR_RULE.slice(0, 15)}${HARBOR_RULE}`);
    // Three approvals of agent tasks and the two proposals' approvals and applies, each by rule.
    const byRule = Array.from(document.querySelectorAll<HTMLElement>(".feed-entry")).filter((element) => element.textContent.includes("By rule"));
    expect(byRule.map((element) => element.dataset.seq)).toEqual(["1203", "1197", "1196", "1195", "1194", "1191", "1188"]);
    expect(within(entry(1195)).getByRole("link", { name: "PR-0035" }).getAttribute("href")).toBe("#/harbor-sim/inbox/PR-0035");
    expect([entry(1199).querySelector(".feed-words .feed-code")?.textContent, entry(1199).querySelector(".feed-words .feed-code")?.getAttribute("title")]).toEqual(["spec check failed", "check"]);
    expect(entry(1205).querySelector(".feed-type")?.textContent).toBe("Landed");
    expect(within(entry(1201)).getByText("Failed")).toBeTruthy();
    // The usage limit's pause, then the evening before the pack started.
    expect(Array.from(document.querySelectorAll(".feed-quiet")).map((quiet) => quiet.textContent)).toEqual(["Nothing happened for 3 h 6 min", "Nothing happened for 5 h 20 min"]);
    // Titles are the queue's now: the agent task's.
    expect(entry(1205).querySelector(".feed-title")?.textContent).toBe("Mooring crew of four");
  });

  it("normal, harbor-sim's Needs me: four holds, the open proposals, the failed run, the uncovered write, nothing unlanded", async () => {
    await renderMock("normal", "#/harbor-sim/feed/needs-me");
    await waitFor(() => {
      expect(document.querySelectorAll(".feed-needs")).toHaveLength(5);
    });
    const lists = Array.from(document.querySelectorAll<HTMLElement>(".feed-needs"));
    const counts = lists.map((list) => list.querySelectorAll("li").length);
    expect([counts[0], counts[2], counts[3], counts[4]]).toEqual([4, 1, 1, 0]);
    expect(counts[1]).toBeGreaterThan(5);
    // Newest first, as All is; each act and reason in words.
    expect(Array.from(lists[0]?.querySelectorAll("a") ?? []).map((link) => link.textContent)).toEqual(["T-0112", "PR-0051", "T-0109", "T-0107"]);
    expect(Array.from(lists[0]?.querySelectorAll(".feed-code") ?? []).map((code) => code.textContent)).toEqual([
      "run",
      "the pack's time ran out",
      "apply",
      "zone excluded by the rule",
      "run",
      "the agent did not finish",
      "approval",
      "kind excluded by the rule",
    ]);
    // Open proposals' kinds in words: none raw.
    expect(lists[1]?.querySelectorAll("code[lang]")).toHaveLength(0);
    expect(lists[4]?.querySelector("p")?.textContent).toBe("None");
  });

  it("normal, harbor-sim: a department's cut asks its lane alone", async () => {
    await renderMock("normal", "#/harbor-sim/feed");
    const department = await screen.findByLabelText<HTMLSelectElement>("Show department");
    expect(Array.from(department.options).map((option) => option.value)).toEqual(["", "galley", "foundry", "harbourmaster", "general"]);
    fireEvent.change(department, { target: { value: "galley" } });
    await waitFor(() => {
      expect(seqs()).toEqual(["1206", "1203"]);
    });
  });

  it("normal: a delegated apply, opened from the feed, reads Applied by rule with its commit and the rule's hash", async () => {
    await renderMock("normal", "#/harbor-sim/inbox/PR-0035", "Inbox");
    const line = await waitFor(() => {
      const found = document.querySelector<HTMLElement>("[data-delegated]");
      expect(found).not.toBeNull();
      return found as HTMLElement;
    });
    expect(line.textContent).toContain("Applied by rule");
    expect(line.querySelector(".sr-only")?.textContent).toBe(HARBOR_RULE);
  });

  it("normal, ledger-api: auto off with the daemon's why, the manual events listed", async () => {
    await renderMock("normal", "#/ledger-api/feed");
    expect((await screen.findByText(/^Auto is off/)).textContent).toBe("Auto is off: no [auto] table in specengine.toml@HEAD");
    await waitFor(() => {
      expect(seqs()).toEqual(["304", "303", "302", "301"]);
    });
    expect(document.querySelectorAll(".feed-entry .lozenge-label")).toHaveLength(4);
    expect(screen.queryByText("By rule")).toBeNull();
  });

  it("empty: nothing has happened yet", async () => {
    await renderMock("empty", "#/harbor-sim/feed");
    expect(await screen.findByRole("heading", { level: 2, name: "Nothing has happened yet." })).toBeTruthy();
  });
});

// The `feed` scenario: harbor-sim's tail says its morning, one event every MORNING_MS, which the feed
// then serves past the night; every other scenario's tail says nothing.
describe("the feed scenario's live morning", () => {
  it("says the morning one event at a time, served by the next read since the night's last; stops when unsubscribed", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval", "Date"], now: NOW });
    const client = new MockClient("feed", { delayMs: 0 });
    const said: QueueEvent[] = [];
    const stop = client.subscribe("harbor-sim", (event) => said.push(event));
    vi.advanceTimersByTime(MORNING_MS * 2);
    expect(said.map((event) => [event.seq, event.type])).toEqual([
      [1207, "task.claimed"],
      [1208, "task.run_reported"],
    ]);
    const later = await client.getFeed("harbor-sim", { since: 1206, needs_me: true });
    expect(later.entries.map((entry) => entry.seq)).toEqual([1207, 1208]);
    expect(later.entries[1]?.at).toBe(new Date(NOW + 2 * MORNING_MS).toISOString().replace(/\.\d{3}Z$/, "Z"));
    stop();
    vi.advanceTimersByTime(MORNING_MS * 4);
    expect(said).toHaveLength(2);
    // Another project's tail, and normal's, say nothing.
    const quiet: QueueEvent[] = [];
    client.subscribe("ledger-api", (event) => quiet.push(event))();
    new MockClient("normal", { delayMs: 0 }).subscribe("harbor-sim", (event) => quiet.push(event));
    vi.advanceTimersByTime(MORNING_MS * 4);
    expect(quiet).toEqual([]);
  });
});
