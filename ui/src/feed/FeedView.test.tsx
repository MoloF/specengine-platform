import { act, cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { ClientError } from "../api/client";
import type { FeedDocument, FeedEntry } from "../api/types";
import { pushHash } from "../app/location";
import { switchLanguage } from "../i18n";
import { ru } from "../i18n/ru";
import { aFeed, aFeedEntry, aFeedNeedsMe, aTaskEntry, aWorkspace, aWorkspaces } from "../test/builders";
import { renderApp } from "../test/render";
import { stubClient } from "../test/stubClient";

// docs/features/auto-feed.md "The UI" over the stub client: Activity at `#/<p>/feed` and
// `#/<p>/feed/needs-me`, the nav's second entry; its entries newest first, each with its time, type, subject
// link, title, words and By rule; the head from `auto`; Needs me's five lists, an empty one saying
// so; the filters' requests; FD5's New marks and the stored point, written on close and on Mark all
// seen, never on open, per login on a hub, and the page correct when storage refuses.

const RULE = `b3:${"abcdef0123456789".repeat(4)}`;
const COMMIT = "3f2a1c9d0b4e5f60718293a4b5c6d7e8f9a0b1c2";

/** The spec's night, oldest first: a run reported, an apply by rule, a landing, a hold. */
const NIGHT: FeedEntry[] = [
  aFeedEntry({ seq: 1201, at: "2026-11-16T01:02:03Z", type: "task.run_reported", task: "T-0042", title: "Fix the door timer", status: "done", outcome: "completed", summary: "the timer resets on close" }),
  aFeedEntry({ seq: 1204, at: "2026-11-16T01:05:40Z", type: "proposal.applied", task: "T-0042", proposal: "PR-0007", title: "R-02", status: "done", by: "rule", commit: COMMIT }),
  aFeedEntry({ seq: 1206, at: "2026-11-16T01:06:02Z", type: "auto.landed", task: "T-0042", title: "Fix the door timer", status: "done", commit: COMMIT }),
  aFeedEntry({ seq: 1207, at: "2026-11-16T04:31:10Z", type: "auto.held", task: "T-0043", title: "Sort the inventory", status: "done", reason: "check", detail: "spec check: 1 error(s); the first docs/spec/core.md:12: ref-dangling" }),
];

function onFeed(document: FeedDocument) {
  const client = stubClient();
  client.getFeed.mockImplementation(() => Promise.resolve(structuredClone(document)));
  return client;
}

async function openFeed(client: ReturnType<typeof stubClient>, hash = "#/alpha/feed") {
  renderApp(client, hash);
  await screen.findByRole("heading", { level: 1, name: "Activity" });
  return client;
}

function entries(): HTMLElement[] {
  return Array.from(document.querySelectorAll<HTMLElement>(".feed-entry"));
}

function seqs(): string[] {
  return entries().map((entry) => entry.dataset.seq ?? "");
}

function marked(): string[] {
  return entries()
    .filter((entry) => entry.dataset.new === "true")
    .map((entry) => entry.dataset.seq ?? "");
}

beforeAll(async () => {
  await import("./FeedView");
});

describe("Activity, the feed (docs/features/auto-feed.md \"The UI\")", () => {
  it("lists the entries newest first, each its type, subject link, title and words; By rule on a rule's act; second in the nav", async () => {
    const client = await openFeed(onFeed(aFeed(NIGHT, { auto: { state: "on", reason: null, why: null, rule: RULE } })));
    await waitFor(() => {
      expect(seqs()).toEqual(["1207", "1206", "1204", "1201"]);
    });
    const [held, landed, applied, reported] = entries();
    expect(held?.querySelector(".feed-type")?.textContent).toBe("Held");
    expect(within(held as HTMLElement).getByRole("link", { name: "T-0043" }).getAttribute("href")).toBe("#/alpha/tasks/T-0043");
    expect([held?.querySelector(".feed-words .feed-code")?.textContent, held?.querySelector(".feed-words .feed-code")?.getAttribute("title")]).toEqual(["spec check failed", "check"]);
    expect(held?.querySelector(".feed-text")?.getAttribute("lang")).toBe("en");
    expect(landed?.querySelector(".feed-type")?.textContent).toBe("Landed");
    expect(landed?.querySelector(".feed-commit")?.textContent).toBe("3f2a1c9d0b4e");
    expect(landed?.querySelector(".feed-commit")?.getAttribute("title")).toBe(COMMIT);
    // The apply by rule: its task the subject, its proposal beside it, By rule; no other entry says so.
    expect(within(applied as HTMLElement).getByRole("link", { name: "PR-0007" }).getAttribute("href")).toBe("#/alpha/inbox/PR-0007");
    expect(entries().map((entry) => entry.textContent.includes("By rule"))).toEqual([false, false, true, false]);
    expect(reported?.querySelector(".feed-type")?.textContent).toBe("Run reported");
    expect(within(reported as HTMLElement).getByText("Completed")).toBeTruthy();
    expect(within(reported as HTMLElement).getByText("the timer resets on close")).toBeTruthy();
    expect(reported?.querySelector(".feed-title")?.textContent).toBe("Fix the door timer");
    // The head: on, the rule's twelve shown and the whole hash read out.
    const head = document.querySelector(".feed-auto");
    expect(head?.textContent).toBe(`Auto: on, rule b3:abcdef012345${RULE}`);
    // The nav's second entry, current here.
    const nav = within(screen.getByRole("navigation", { name: "Sections" })).getAllByRole("link");
    expect(nav.slice(0, 2).map((link) => [link.textContent, link.getAttribute("href"), link.getAttribute("aria-current")])).toEqual([
      ["Overview", "#/alpha", null],
      ["Activity", "#/alpha/feed", "page"],
    ]);
    expect(client.getFeed).toHaveBeenCalledWith("alpha", { needs_me: true }, expect.any(AbortSignal));
  });

  it("says Auto is off with the daemon's why, and lists the manual events as they are (RL1)", async () => {
    await openFeed(onFeed(aFeed([aFeedEntry({ seq: 3, type: "proposal.created", proposal: "PR-0001", title: "R-1" }), aFeedEntry({ seq: 4, type: "card.moved", card: "C-0001", title: "A task" })])));
    expect((await screen.findByText(/^Auto is off/)).textContent).toBe("Auto is off: no [auto] table in specengine.toml@HEAD");
    expect(screen.getByText("no [auto] table in specengine.toml@HEAD").getAttribute("lang")).toBe("en");
    const [moved, created] = entries();
    expect(within(moved as HTMLElement).getByRole("link", { name: "C-0001" }).getAttribute("href")).toBe("#/alpha/board/C-0001");
    expect(moved?.querySelector(".feed-type")?.textContent).toBe("Moved");
    expect(created?.querySelector(".feed-type")?.textContent).toBe("Raised");
    expect(document.querySelectorAll(".feed-entry .lozenge")).toHaveLength(2);
  });

  it("shows a type no table knows as stored, under lang=en, and two hours or more with nothing between two entries", async () => {
    await openFeed(onFeed(aFeed([aFeedEntry({ seq: 7, at: "2026-11-16T01:38:00Z", type: "verify.tests" }), aFeedEntry({ seq: 8, at: "2026-11-16T04:47:00Z", type: "task.claimed", task: "T-0001" })])));
    await waitFor(() => {
      expect(seqs()).toEqual(["8", "7"]);
    });
    const raw = entries()[1]?.querySelector(".feed-type");
    expect([raw?.textContent, raw?.getAttribute("lang")]).toEqual(["verify.tests", "en"]);
    expect(document.querySelector(".feed-quiet")?.textContent).toBe("Nothing happened for 3 h 9 min");
  });

  it("says the daemon's refusal verbatim, with Retry", async () => {
    const client = stubClient();
    client.getFeed.mockRejectedValue(new ClientError({ status: 400, message: "`since` is `x`: a seq or YYYY-MM-DDTHH:MM:SSZ" }));
    await openFeed(client);
    const alert = await screen.findByRole("alert");
    expect(within(alert).getByText("`since` is `x`: a seq or YYYY-MM-DDTHH:MM:SSZ").getAttribute("lang")).toBe("en");
    expect(screen.getByRole("button", { name: "Retry" })).toBeTruthy();
  });
});

describe("Needs me (docs/features/auto-feed.md \"The UI\")", () => {
  it("lists the five lists at #/<p>/feed/needs-me, each empty one saying so, the tab counting them all", async () => {
    const needs = aFeedNeedsMe({
      held: [{ seq: 1207, at: "2026-11-16T04:31:10Z", pack: 812, task: "T-0043", line: null, proposal: null, act: "land", reason: "check", detail: "spec check: 1 error(s)" }],
      open: [{ id: "PR-0009", kind: "question", target_id: "R-02", task_id: "T-0042", created_at: "2026-11-16T01:00:00Z" }],
      failed_runs: [{ task: "T-0044", run: 1, outcome: "failed", summary: "the loader stops", ended_at: "2026-11-16T03:00:00Z" }],
    });
    await openFeed(onFeed(aFeed(NIGHT, { needs_me: needs })), "#/alpha/feed/needs-me");
    const tab = await screen.findByRole("tab", { name: "Needs me 3", selected: true });
    expect(tab).toBeTruthy();
    const lists = Array.from(document.querySelectorAll<HTMLElement>(".feed-needs"));
    expect(lists.map((list) => list.querySelector("h2 > span")?.textContent)).toEqual(["Held", "Open proposals", "Failed runs", "Uncovered writes", "Done, not landed"]);
    expect(lists.map((list) => list.querySelectorAll("li").length)).toEqual([1, 1, 1, 0, 0]);
    expect(lists.slice(3).map((list) => list.querySelector("p")?.textContent)).toEqual(["None", "None"]);
    expect(within(lists[0] as HTMLElement).getByRole("link", { name: "T-0043" }).getAttribute("href")).toBe("#/alpha/tasks/T-0043");
    expect(within(lists[1] as HTMLElement).getByRole("link", { name: "PR-0009" }).getAttribute("href")).toBe("#/alpha/inbox/PR-0009");
    expect(within(lists[2] as HTMLElement).getByText("Failed")).toBeTruthy();
    // All is the other tab, its hash the bare one.
    fireEvent.click(screen.getByRole("tab", { name: "All" }));
    await waitFor(() => {
      expect(window.location.hash).toBe("#/alpha/feed");
    });
  });
});

describe("the filters (docs/features/auto-feed.md \"The UI\")", () => {
  it("asks the agent task chosen, and the department where the project declares departments; the department select is absent without", async () => {
    const client = onFeed(aFeed(NIGHT));
    client.getTasks.mockResolvedValue({ tasks: [aTaskEntry({ id: "T-0043", title: "Sort the inventory" }), aTaskEntry({ id: "T-0042", title: "Fix the door timer" })], notes: [] });
    client.getWorkspaces.mockResolvedValue(aWorkspaces([aWorkspace({ name: "maps", title: "Maps" })]));
    await openFeed(client);
    const task = await screen.findByLabelText<HTMLSelectElement>("Agent task");
    await waitFor(() => {
      expect(Array.from(task.options).map((option) => option.textContent)).toEqual(["All agent tasks", "T-0042 Fix the door timer", "T-0043 Sort the inventory"]);
    });
    fireEvent.change(task, { target: { value: "T-0042" } });
    await waitFor(() => {
      expect(client.getFeed).toHaveBeenLastCalledWith("alpha", { needs_me: true, task: "T-0042" }, expect.any(AbortSignal));
    });
    const department = await screen.findByLabelText<HTMLSelectElement>("Show department");
    expect(Array.from(department.options).map((option) => [option.value, option.textContent])).toEqual([
      ["", "All departments"],
      ["maps", "Maps"],
      ["general", "General"],
    ]);
    fireEvent.change(department, { target: { value: "maps" } });
    await waitFor(() => {
      expect(client.getFeed).toHaveBeenLastCalledWith("alpha", { needs_me: true, task: "T-0042", department: "maps" }, expect.any(AbortSignal));
    });
  });

  it("shows no department select for the implicit department", async () => {
    await openFeed(onFeed(aFeed(NIGHT)));
    await screen.findByLabelText("Agent task");
    expect(screen.queryByLabelText("Show department")).toBeNull();
  });

  it("New since your last visit asks `since` the stored point, and says when nothing is new", async () => {
    window.localStorage.setItem("specengine.feed.seen.alpha", "1207");
    const client = stubClient();
    client.getFeed.mockImplementation((_project, options) => Promise.resolve(aFeed(options.since === undefined ? NIGHT : [], { after: options.since ?? null })));
    await openFeed(client);
    await waitFor(() => {
      expect(seqs()).toHaveLength(4);
    });
    fireEvent.click(screen.getByRole("button", { name: /New since your last visit/ }));
    expect(await screen.findByRole("heading", { level: 2, name: "Nothing new since your last visit." })).toBeTruthy();
    expect(client.getFeed).toHaveBeenLastCalledWith("alpha", { needs_me: true, since: 1207 }, expect.any(AbortSignal));
  });
});

describe("New since your last visit (FD5 of auto-feed)", () => {
  it("stored 1204: entries above it carry New; opening writes nothing; leaving stores the highest shown (M: writing seen on open)", async () => {
    window.localStorage.setItem("specengine.feed.seen.alpha", "1204");
    await openFeed(onFeed(aFeed(NIGHT)));
    await waitFor(() => {
      expect(marked()).toEqual(["1207", "1206"]);
    });
    expect(entries()[0]?.querySelector(".lozenge")?.textContent).toBe("New");
    expect(screen.getByRole("button", { name: /New since your last visit/ }).textContent).toContain("2");
    expect(window.localStorage.getItem("specengine.feed.seen.alpha")).toBe("1204");
    act(() => {
      pushHash("#/alpha/inbox");
    });
    await screen.findByRole("heading", { level: 1, name: "Inbox" });
    expect(window.localStorage.getItem("specengine.feed.seen.alpha")).toBe("1207");
  });

  it("never lowers the stored point when the view showed less", async () => {
    window.localStorage.setItem("specengine.feed.seen.alpha", "1300");
    await openFeed(onFeed(aFeed(NIGHT)));
    await waitFor(() => {
      expect(seqs()).toHaveLength(4);
    });
    expect(marked()).toEqual([]);
    act(() => {
      pushHash("#/alpha/inbox");
    });
    await screen.findByRole("heading", { level: 1, name: "Inbox" });
    expect(window.localStorage.getItem("specengine.feed.seen.alpha")).toBe("1300");
  });

  it("on a hub, keys the point by the login, never the browser-wide key", async () => {
    window.localStorage.setItem("specengine.feed.seen.alpha", "1");
    window.localStorage.setItem("specengine.feed.seen.alpha.mira", "1206");
    const client = onFeed(aFeed(NIGHT));
    client.getIdentity.mockResolvedValue({ login: "mira" });
    await openFeed(client);
    await waitFor(() => {
      expect(marked()).toEqual(["1207"]);
    });
    fireEvent.click(screen.getByRole("button", { name: "Mark all seen" }));
    expect(marked()).toEqual([]);
    expect(window.localStorage.getItem("specengine.feed.seen.alpha.mira")).toBe("1207");
    expect(window.localStorage.getItem("specengine.feed.seen.alpha")).toBe("1");
  });

  it("with no point kept, every entry is new; Mark all seen takes the marks off and keeps the highest", async () => {
    await openFeed(onFeed(aFeed(NIGHT)));
    await waitFor(() => {
      expect(marked()).toEqual(["1207", "1206", "1204", "1201"]);
    });
    fireEvent.click(screen.getByRole("button", { name: "Mark all seen" }));
    expect(marked()).toEqual([]);
    expect(window.localStorage.getItem("specengine.feed.seen.alpha")).toBe("1207");
  });

  it("storage that refuses: the entries still show, all new, and Mark all seen still takes the marks off", async () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("denied by a policy");
    });
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("denied by a policy");
    });
    await openFeed(onFeed(aFeed(NIGHT)));
    await waitFor(() => {
      expect(marked()).toEqual(["1207", "1206", "1204", "1201"]);
    });
    fireEvent.click(screen.getByRole("button", { name: "Mark all seen" }));
    expect(marked()).toEqual([]);
  });
});

// The coordinator's report of 2026-11-15: a first visit's New since your last visit read 0. With no point
// kept every entry is new and the chip counts them all, in both languages; a reload closes the view and
// keeps the point (`pagehide`), so the next visit counts only what came after it.
describe("the first visit's count (FD5 of auto-feed)", () => {
  function chip(name: string): HTMLElement {
    return screen.getByRole("button", { name: new RegExp(`^${name}`) });
  }

  it("counts every entry on a first visit, in English and in Russian; nothing is stored until the view closes", async () => {
    await openFeed(onFeed(aFeed(NIGHT)));
    await waitFor(() => {
      expect(chip("New since your last visit").querySelector(".chip-count")?.textContent).toBe("4");
    });
    act(() => {
      switchLanguage("ru");
    });
    expect(chip(ru.feed.newOnly).querySelector(".chip-count")?.textContent).toBe("4");
    expect(window.localStorage.getItem("specengine.feed.seen.alpha")).toBeNull();
  });

  it("a reload keeps the highest shown, so the page after it counts nothing new from before", async () => {
    await openFeed(onFeed(aFeed(NIGHT)));
    await waitFor(() => {
      expect(marked()).toHaveLength(4);
    });
    act(() => {
      window.dispatchEvent(new Event("pagehide"));
    });
    expect(window.localStorage.getItem("specengine.feed.seen.alpha")).toBe("1207");
    cleanup();
    await openFeed(onFeed(aFeed([...NIGHT, aFeedEntry({ seq: 1210, type: "task.claimed", task: "T-0044" })])));
    await waitFor(() => {
      expect(marked()).toEqual(["1210"]);
    });
    expect(chip("New since your last visit").querySelector(".chip-count")?.textContent).toBe("1");
  });
});

describe("the codes said in words (the coordinator's report of 2026-11-15)", () => {
  it("says a hold's act and reason and an open proposal's kind in words, each code its title; an unknown one raw; Needs me newest first", async () => {
    const needs = aFeedNeedsMe({
      held: [
        { seq: 1190, at: "2026-11-16T00:10:00Z", pack: 812, task: "T-0041", line: null, proposal: null, act: "approve", reason: "excluded-kind", detail: null },
        { seq: 1207, at: "2026-11-16T04:31:10Z", pack: 812, task: "T-0043", line: null, proposal: null, act: "land", reason: "check", detail: "spec check: 1 error(s)" },
        { seq: 1208, at: "2026-11-16T04:40:00Z", pack: 812, task: "T-0045", line: null, proposal: null, act: "leap", reason: "a-new-reason", detail: null },
      ],
      open: [
        { id: "PR-0009", kind: "question", target_id: "R-02", task_id: null, created_at: "2026-11-16T01:00:00Z" },
        { id: "PR-0012", kind: "update", target_id: "R-03", task_id: null, created_at: "2026-11-16T03:00:00Z" },
        { id: "PR-0010", kind: "reconcile", target_id: null, task_id: null, created_at: "2026-11-16T01:00:00Z" },
      ],
    });
    await openFeed(onFeed(aFeed(NIGHT, { needs_me: needs })), "#/alpha/feed/needs-me");
    await waitFor(() => {
      expect(document.querySelectorAll(".feed-needs")).toHaveLength(5);
    });
    const [held, open] = Array.from(document.querySelectorAll<HTMLElement>(".feed-needs"));
    const rows = (list: HTMLElement | undefined) => Array.from(list?.querySelectorAll("li") ?? []);
    expect(rows(held).map((row) => row.querySelector("a")?.textContent)).toEqual(["T-0045", "T-0043", "T-0041"]);
    const said = (row: Element) => Array.from(row.querySelectorAll(".feed-code, code")).map((element) => [element.textContent, element.getAttribute("title")]);
    expect(rows(held).map(said)).toEqual([
      [
        ["leap", null],
        ["a-new-reason", null],
      ],
      [
        ["merge", "land"],
        ["spec check failed", "check"],
      ],
      [
        ["approval", "approve"],
        ["kind excluded by the rule", "excluded-kind"],
      ],
    ]);
    expect(rows(open).map((row) => row.querySelector("a")?.textContent)).toEqual(["PR-0012", "PR-0010", "PR-0009"]);
    expect(rows(open).map((row) => row.querySelector(".feed-code, code[lang]")?.textContent)).toEqual(["Spec change", "reconcile", "Question"]);
    act(() => {
      switchLanguage("ru");
    });
    expect(rows(held)[1]?.querySelector(".feed-code")?.textContent).toBe(ru.feed.act.land);
    expect(rows(held)[2]?.querySelectorAll(".feed-code")[1]?.textContent).toBe(ru.feed.reason.excluded_kind);
  });

  it("says an entry's reason in words, the code its title", async () => {
    await openFeed(onFeed(aFeed(NIGHT)));
    await waitFor(() => {
      expect(seqs()).toHaveLength(4);
    });
    const reason = document.querySelector('.feed-entry[data-seq="1207"] .feed-code');
    expect([reason?.textContent, reason?.getAttribute("title")]).toEqual(["spec check failed", "check"]);
  });
});

// The review of 2026-11-15: a key the CLI may serve null never throws (`feed.rs`: a hold's act and
// reason, a gate write's path, an open proposal's kind and time, an entry's `at` with its payload
// unparsed); Mark all seen never lowers the point and says so; a cap of 100 new says 100+.
describe("nulls, the kept point and the cap (the review of 2026-11-15)", () => {
  it("draws every null the CLI may serve, saying nothing for it", async () => {
    const needs = aFeedNeedsMe({
      held: [{ seq: 7, at: null, pack: null, task: "T-0001", line: null, proposal: null, act: null, reason: null, detail: null }],
      open: [{ id: "PR-0001", kind: null, target_id: null, task_id: null, created_at: null }],
      failed_runs: [{ task: "T-0002", run: 1, outcome: null, summary: null, ended_at: null }],
      gate_observed: [{ seq: 8, at: null, path: null, zone: null, branch: null }],
    });
    const unparsed = aFeedEntry({ seq: 9, at: null, type: "task.claimed" });
    await openFeed(onFeed(aFeed([aFeedEntry({ seq: 6, at: "2026-11-16T01:00:00Z" }), unparsed], { needs_me: needs, notes: ["event 9: its payload is not JSON"] })), "#/alpha/feed/needs-me");
    await waitFor(() => {
      expect(document.querySelectorAll(".feed-needs")).toHaveLength(5);
    });
    const lists = Array.from(document.querySelectorAll<HTMLElement>(".feed-needs"));
    expect(lists.map((list) => list.querySelectorAll("li").length)).toEqual([1, 1, 1, 1, 0]);
    expect(lists.flatMap((list) => Array.from(list.querySelectorAll("time, .feed-code, code")))).toEqual([]);
    expect(screen.getByText("event 9: its payload is not JSON")).toBeTruthy();
    fireEvent.click(screen.getByRole("tab", { name: /^All/ }));
    await waitFor(() => {
      expect(seqs()).toEqual(["9", "6"]);
    });
    expect(entries()[0]?.querySelector("time")).toBeNull();
    expect(document.querySelectorAll(".feed-quiet")).toHaveLength(0);
  });

  it("Mark all seen keeps a higher point another tab kept, and says All marked seen", async () => {
    window.localStorage.setItem("specengine.feed.seen.alpha", "1204");
    await openFeed(onFeed(aFeed(NIGHT)));
    await waitFor(() => {
      expect(marked()).toEqual(["1207", "1206"]);
    });
    // Another tab marked further meanwhile.
    window.localStorage.setItem("specengine.feed.seen.alpha", "1300");
    fireEvent.click(screen.getByRole("button", { name: "Mark all seen" }));
    expect(marked()).toEqual([]);
    expect(window.localStorage.getItem("specengine.feed.seen.alpha")).toBe("1300");
    await waitFor(() => {
      expect(Array.from(document.querySelectorAll("[data-live-region]")).map((region) => region.textContent)).toContain("All marked seen");
    });
  });

  it("says 100+ when the newest hundred alone were read and all are new; the count as it is once a point cuts it", async () => {
    const hundred = Array.from({ length: 100 }, (_, index) => aFeedEntry({ seq: index + 1 }));
    await openFeed(onFeed(aFeed(hundred)));
    const chip = () => screen.getByRole("button", { name: /^New since your last visit/ }).querySelector(".chip-count")?.textContent;
    await waitFor(() => {
      expect(chip()).toBe("100+");
    });
    cleanup();
    window.localStorage.setItem("specengine.feed.seen.alpha", "0");
    await openFeed(onFeed(aFeed(hundred.slice(0, 99))));
    await waitFor(() => {
      expect(chip()).toBe("99");
    });
  });

  it("draws Needs me while its tab shows alone, its count on the tab either way", async () => {
    const needs = aFeedNeedsMe({ open: [{ id: "PR-0001", kind: "question", target_id: null, task_id: null, created_at: "2026-11-16T01:00:00Z" }] });
    await openFeed(onFeed(aFeed(NIGHT, { needs_me: needs })));
    await waitFor(() => {
      expect(seqs()).toHaveLength(4);
    });
    expect(screen.getByRole("tab", { name: "Needs me 1" })).toBeTruthy();
    expect(document.querySelectorAll(".feed-needs")).toHaveLength(0);
    // The quiet line is no list item: the list counts its entries alone.
    expect(within(document.querySelector(".feed-list") as HTMLElement).getAllByRole("listitem")).toHaveLength(4);
  });
});
