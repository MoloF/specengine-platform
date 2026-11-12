import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ClientError } from "../api/client";
import { localToday } from "../board/due";
import { en } from "../i18n/en";
import { ru } from "../i18n/ru";
import { CYRILLIC_DEPARTMENT, deliveryClient, everyShape, GREEK_TASK_TITLE, noTaskDelivery, pinnedDelivery } from "../test/deliveryStub";
import { renderApp } from "../test/render";
import { argsOf } from "../test/stubClient";

// DA-12 of docs/features/delivery-analytics.md "Acceptance criteria", on the stub client: Delivery is
// the Roadmap's third tab, its own chunk, remembered per project; it sends `as_of` the viewer's own
// day and a new day reads again with it; By release labels each group, an unknown one raw; By
// department titles each department and ends in All tasks, the served total; the late tasks in
// served order, the ID to the Board, how late, their facts and each reason, the cut said per list,
// an item to the Inbox; an empty project the EmptyState; the notes verbatim; a failure in the
// daemon's words with Retry. Every figure is the served one: the tab counts nothing.

/** The viewer's calendar day now, as the tab takes it. */
function today(): string {
  return localToday(Date.now());
}

async function openDelivery(client = deliveryClient(), hash = "#/alpha/roadmap") {
  renderApp(client, hash);
  fireEvent.click(await screen.findByRole("tab", { name: "Delivery" }));
  return client;
}

async function tables(client = deliveryClient()) {
  await openDelivery(client);
  await screen.findByRole("table", { name: "By release" });
  return client;
}

/** A table's rows as text, cell by cell: its header row first, a foot row last. */
function rowsOf(name: string): string[][] {
  return Array.from(screen.getByRole("table", { name }).querySelectorAll("tr"), (row) => Array.from(row.children, (cell) => cell.textContent));
}

function lateTask(id: string): HTMLElement {
  const found = Array.from(document.querySelectorAll<HTMLElement>("[data-late]")).find((element) => element.dataset.late === id);
  if (found === undefined) {
    throw new Error(`no late task ${id}`);
  }
  return found;
}

/** A late task's facts: label and value per row. */
function factsOf(id: string): [string | null, string | null][] {
  return Array.from(lateTask(id).querySelectorAll(".property-row"), (row) => [row.querySelector("dt")?.textContent ?? null, row.querySelector("dd")?.textContent ?? null]);
}

/** A late task's reasons as text, each with what kind of reason it is (`more` for a cut). */
function reasonsOf(id: string): [string | null, string | null][] {
  return Array.from(lateTask(id).querySelectorAll(".delivery-reasons > li"), (item) => [item.getAttribute("data-reason") ?? "more", item.textContent]);
}

/** A link clicked as the browser follows it (jsdom moves the hash only a task later). */
async function follow(link: HTMLElement) {
  fireEvent.click(link);
  await act(async () => {
    window.location.hash = link.getAttribute("href") ?? "";
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

/** Every leaf of a catalog group by its dotted key. */
function leaves(group: unknown, path = ""): [string, string][] {
  if (typeof group === "string") {
    return [[path, group]];
  }
  if (typeof group !== "object" || group === null) {
    return [];
  }
  return Object.entries(group).flatMap(([key, value]) => leaves(value, path === "" ? key : `${path}.${key}`));
}

describe("the tab and its read (DA-12)", () => {
  it("is the Roadmap's third tab; it reads the delivery once, `as_of` the viewer's own day, with the query's signal; the field holds the day", async () => {
    const client = await openDelivery();
    expect(screen.getAllByRole("tab").map((tab) => [tab.textContent, tab.getAttribute("aria-selected")])).toEqual([
      ["Lanes", "false"],
      ["Chain", "false"],
      ["Delivery", "true"],
    ]);
    await screen.findByRole("table", { name: "By release" });
    expect(argsOf(client.getDelivery).at(-1)).toEqual(["alpha", { asOf: today() }]);
    expect(client.getDelivery.mock.calls.every((call) => call[1].asOf !== "")).toBe(true);
    expect(client.getDelivery.mock.calls.at(-1)?.[2]).toBeInstanceOf(AbortSignal);
    const field = screen.getByLabelText<HTMLInputElement>("As of");
    expect([field.type, field.value, field.className]).toEqual(["date", today(), "field delivery-date"]);
    expect(document.getElementById(field.getAttribute("aria-describedby") ?? "")?.textContent).toBe("An open task due before this day is overdue; the rest is as recorded.");
  });

  it("shows a skeleton while the read is out, the Delivery's own label", async () => {
    const client = deliveryClient();
    client.getDelivery.mockReturnValue(new Promise(() => undefined));
    await openDelivery(client);
    const skeleton = await screen.findByRole("status", { name: "Reading the delivery of alpha" });
    expect(skeleton.getAttribute("aria-busy")).toBe("true");
  });

  it("reads again with a new day; Today returns to the viewer's own", async () => {
    const client = await tables();
    fireEvent.change(screen.getByLabelText("As of"), { target: { value: "2026-11-12" } });
    await waitFor(() => {
      expect(argsOf(client.getDelivery).at(-1)).toEqual(["alpha", { asOf: "2026-11-12" }]);
    });
    expect(screen.getByLabelText<HTMLInputElement>("As of").value).toBe("2026-11-12");
    await screen.findByRole("table", { name: "By release" });
    fireEvent.click(screen.getByRole("button", { name: "Today" }));
    await waitFor(() => {
      expect(screen.getByLabelText<HTMLInputElement>("As of").value).toBe(today());
    });
    expect(argsOf(client.getDelivery).map(([, options]) => options)).toContainEqual({ asOf: "2026-11-12" });
    // An emptied field keeps the day asked: nothing is sent without a day.
    const calls = client.getDelivery.mock.calls.length;
    fireEvent.change(screen.getByLabelText("As of"), { target: { value: "" } });
    expect(client.getDelivery.mock.calls).toHaveLength(calls);
    expect(screen.getByLabelText<HTMLInputElement>("As of").value).toBe(today());
  });

  it("keeps Delivery chosen per project while the page lives", async () => {
    await tables();
    await follow(within(screen.getByRole("navigation", { name: "Sections" })).getByRole("link", { name: "Inbox" }));
    await screen.findByRole("heading", { level: 1, name: "Inbox" });
    await follow(within(screen.getByRole("navigation", { name: "Sections" })).getByRole("link", { name: "Roadmap" }));
    expect((await screen.findByRole("tab", { name: "Delivery" })).getAttribute("aria-selected")).toBe("true");
    expect(await screen.findByRole("table", { name: "By department" })).toBeTruthy();
  });

  it("says a failure in the daemon's words with Retry; Retry reads again", async () => {
    const client = deliveryClient();
    const message = "`as_of`: `2026-13-01` is no calendar date, YYYY-MM-DD";
    client.getDelivery.mockRejectedValueOnce(new ClientError({ status: 400, message }));
    await openDelivery(client);
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toBe(`The delivery could not be read${message}`);
    expect(alert.querySelector(".error-message")?.getAttribute("lang")).toBe("en");
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(await screen.findByRole("table", { name: "By release" })).toBeTruthy();
  });
});

describe("the two tables (DA-12)", () => {
  it("By release: the spec's rows as served, r1 1 of 1, r2's lead None delivered, No release last", async () => {
    await tables();
    expect(rowsOf("By release")).toEqual([
      ["Release", "Delivered", "Open", "Overdue", "Delivered late", "Cancelled", "Lead time"],
      ["r1", "1 of 1", "0", "0", "1", "1", "median 14, max 14 days"],
      ["r2", "0 of 1", "1", "1", "0", "0", "None delivered"],
      ["No release", "1 of 2", "1", "0", "0", "0", "median 3, max 3 days"],
    ]);
    const table = screen.getByRole("table", { name: "By release" });
    expect(Array.from(table.querySelectorAll("tbody th"), (cell) => cell.getAttribute("scope"))).toEqual(["row", "row", "row"]);
    expect(Array.from(table.querySelectorAll("thead th"), (cell) => cell.getAttribute("scope"))).toEqual(Array.from({ length: 7 }, () => "col"));
    // Scrolls in its own region, named by its caption, reachable by keyboard.
    const region = screen.getByRole("region", { name: "By release" });
    expect([region.contains(table), region.getAttribute("tabindex")]).toEqual([true, "0"]);
  });

  it("By release: Pool, Unplaced and an unknown group raw under lang=\"en\", in served order", async () => {
    await tables(deliveryClient(everyShape()));
    expect(rowsOf("By release").map((row) => row[0])).toEqual(["Release", "r1", "r2", "Pool", "Unplaced", "milestone", "No release"]);
    const labels = Array.from(screen.getByRole("table", { name: "By release" }).querySelectorAll("tbody th bdi"), (label) => [label.textContent, label.getAttribute("lang")]);
    expect(labels).toEqual([
      ["r1", null],
      ["r2", null],
      ["Pool", null],
      ["Unplaced", null],
      ["milestone", "en"],
      ["No release", null],
    ]);
  });

  it("By department: each title in a bdi, a department of zeros too, ending in All tasks, the served total", async () => {
    await tables(deliveryClient(everyShape()));
    expect(rowsOf("By department")).toEqual([
      ["Department", "Delivered", "Open", "Overdue", "Delivered late", "Cancelled", "Lead time"],
      ["Words", "2 of 2", "0", "0", "1", "0", "median 3, max 14 days"],
      ["Print", "2 of 3", "1", "1", "1", "1", "median 3, max 14 days"],
      [CYRILLIC_DEPARTMENT, "0 of 0", "0", "0", "0", "0", "None delivered"],
      ["All tasks", "2 of 4", "2", "1", "1", "1", "median 3, max 14 days"],
    ]);
    const table = screen.getByRole("table", { name: "By department" });
    expect(Array.from(table.querySelectorAll("tbody th > bdi"), (title) => title.textContent)).toEqual(["Words", "Print", CYRILLIC_DEPARTMENT]);
    expect(table.querySelector("tfoot th")?.getAttribute("scope")).toBe("row");
  });
});

describe("the late tasks (DA-12)", () => {
  it("lists them in served order, each ID to the Board, its title, how late with an icon", async () => {
    await tables();
    const list = screen.getByRole("heading", { level: 2, name: "Late tasks" }).parentElement?.querySelector("ol");
    expect(Array.from(list?.children ?? [], (item) => item.getAttribute("data-late"))).toEqual(["C-0003", "C-0001"]);
    for (const [id, title, mark, appearance] of [
      ["C-0003", "Engrave the harbor map", "Overdue by 8 days", "danger"],
      ["C-0001", "Write the tide table foreword", "Delivered 5 days late", "warning"],
    ]) {
      const task = lateTask(id ?? "");
      expect(within(task).getByRole("link", { name: id }).getAttribute("href")).toBe(`#/alpha/board/${id ?? ""}`);
      expect(task.querySelector(".delivery-late-title")?.textContent).toBe(title);
      const lozenge = task.querySelector(".lozenge");
      expect([lozenge?.textContent, lozenge?.getAttribute("data-appearance"), lozenge?.querySelector("svg.icon") !== null]).toEqual([mark, appearance, true]);
    }
  });

  it("gives each its facts: due as written, times in UTC, none taken or accepted None, the estimate as written, departments by their served titles", async () => {
    await tables();
    expect(factsOf("C-0003")).toEqual([
      ["Due", "2026-11-12"],
      ["Created", "2026-11-04 09:00 UTC"],
      ["Taken", "2026-11-05 09:00 UTC"],
      ["Accepted", "None"],
      ["Estimate", "None"],
      ["Departments", "Print"],
    ]);
    expect(factsOf("C-0001")).toEqual([
      ["Due", "2026-11-10"],
      ["Created", "2026-11-01 09:00 UTC"],
      ["Taken", "2026-11-03 09:00 UTC"],
      ["Accepted", "2026-11-15 09:00 UTC"],
      ["Estimate", "5"],
      ["Departments", "Words, Print"],
    ]);
    expect(lateTask("C-0001").querySelector("time")?.getAttribute("dateTime")).toBe("2026-11-01T09:00:00Z");
  });

  it("lists each reason: the returns with their notes, the re-estimate old to new, the cancelled subtask to the Board, the item to the Inbox, its state raw", async () => {
    await tables();
    expect(lateTask("C-0003").querySelector(".delivery-reasons")).toBeNull();
    expect(reasonsOf("C-0001")).toEqual([
      ["return", "Returned from review on 2026-11-06 09:00 UTCToo long"],
      ["return", "Returned from review on 2026-11-08 09:00 UTCWrong tone"],
      ["reestimate", "Re-estimated 3 to 5 on 2026-11-07 09:00 UTCThe tables doubled"],
      ["cancelled", "Subtask C-0002 cancelled on 2026-11-09 09:00 UTCDraw the tide platesPlates reused from last year"],
      ["item", "QuestionPR-0040applied"],
    ]);
    const task = within(lateTask("C-0001"));
    expect(task.getByRole("link", { name: "C-0002" }).getAttribute("href")).toBe("#/alpha/board/C-0002");
    expect(task.getByRole("link", { name: "PR-0040" }).getAttribute("href")).toBe("#/alpha/inbox/PR-0040");
    const item = lateTask("C-0001").querySelector('[data-reason="item"]');
    expect(Array.from(item?.children ?? [], (part) => [part.textContent, part.getAttribute("lang")])).toEqual([
      ["Question", null],
      ["PR-0040", null],
      ["applied", "en"],
    ]);
  });

  it("says each list's cut after it, a return with no note, a clear from none, an unknown item kind raw; a department no row holds as written", async () => {
    await tables(deliveryClient(everyShape()));
    expect(lateTask("C-0007").querySelector(".delivery-late-title")?.textContent).toBe(GREEK_TASK_TITLE);
    expect(lateTask("C-0007").querySelector(".lozenge")?.textContent).toBe("Overdue by 1 day");
    const reasons = reasonsOf("C-0007");
    expect(reasons.slice(0, 2)).toEqual([
      ["return", "Returned from review on 2026-11-02 09:00 UTC"],
      ["return", "Returned from review on 2026-11-03 09:00 UTCPass 2"],
    ]);
    expect(reasons.slice(16)).toEqual([
      ["more", "4 more not listed"],
      ["reestimate", "Estimate None cleared on 2026-11-10 09:00 UTCNo estimate until the survey lands"],
      ["more", "1 more not listed"],
      ["cancelled", "Subtask C-0008 cancelled on 2026-11-11 09:00 UTCSecond plateOne plate holds both"],
      ["more", "2 more not listed"],
      ["item", "DiscrepancyPR-0041open"],
      ["item", "DecisionPR-0042approved"],
      ["item", "interpretationPR-0043open"],
      ["more", "3 more not listed"],
    ]);
    const unknown = lateTask("C-0007").querySelectorAll('[data-reason="item"]')[2];
    expect(unknown?.firstElementChild?.getAttribute("lang")).toBe("en");
    expect(factsOf("C-0007").at(-1)).toEqual(["Departments", "Print, gone"]);
    // The pinned tasks follow, as served.
    expect(Array.from(document.querySelectorAll("[data-late]"), (task) => task.getAttribute("data-late"))).toEqual(["C-0007", "C-0003", "C-0001"]);
  });

  it("says no task is late when none is, the tables still drawn", async () => {
    await tables(deliveryClient({ ...pinnedDelivery(), late: [] }));
    expect(screen.getByRole("heading", { level: 2, name: "Late tasks" }).nextElementSibling?.textContent).toBe("No task is late.");
    expect(document.querySelector("[data-late]")).toBeNull();
  });
});

describe("an empty project and the notes (DA-12)", () => {
  it("shows the EmptyState at h3 for a project with no task, no table and no late list", async () => {
    await openDelivery(deliveryClient(noTaskDelivery()));
    const box = await waitFor(() => {
      const found = document.querySelector(".delivery .empty-state-box");
      expect(found).not.toBeNull();
      return found;
    });
    expect(box?.querySelector("h3.empty-state-title")?.textContent).toBe("No task to count yet");
    expect(box?.querySelector(".empty-state-description")?.textContent).toBe("Tasks made on the Board count here: delivered, open and overdue, by release and by department.");
    expect(screen.queryByRole("table")).toBeNull();
    expect(screen.queryByRole("heading", { name: "Late tasks" })).toBeNull();
  });

  it("lists the notes verbatim under lang=\"en\", last", async () => {
    await tables(deliveryClient(everyShape()));
    const heading = screen.getByRole("heading", { level: 2, name: "Notes" });
    const items = Array.from(heading.parentElement?.querySelectorAll("li") ?? [], (item) => [item.textContent, item.getAttribute("lang")]);
    expect(items).toEqual([
      ["delivery: [card] does not read: no return is counted", "en"],
      ["C-0009: its row does not read (due: no calendar date); not counted", "en"],
    ]);
    expect(heading.closest("section")?.nextElementSibling).toBeNull();
  });
});

describe("the tab's words (DA-12)", () => {
  it("never says block: no roadmap.delivery leaf in either catalog, nor the tab drawn", async () => {
    const stem = "bl" + "ock";
    // The Russian stem as escapes: this file holds no Cyrillic.
    const russianStem = "\u0431\u043b\u043e\u043a";
    expect(leaves(en.roadmap.delivery).filter(([, value]) => value.toLowerCase().includes(stem))).toEqual([]);
    expect(leaves(ru.roadmap.delivery).filter(([, value]) => value.toLowerCase().includes(russianStem) || value.toLowerCase().includes(stem))).toEqual([]);
    expect(leaves(en.roadmap.delivery).length).toBeGreaterThan(40);
    await tables(deliveryClient(everyShape()));
    expect(document.querySelector(".delivery")?.textContent).not.toMatch(new RegExp(stem, "i"));
  });
});
