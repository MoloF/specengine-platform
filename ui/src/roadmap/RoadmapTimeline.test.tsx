import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ClientError } from "../api/client";
import type { DeliveryTimeline } from "../api/types";
import { en } from "../i18n/en";
import { ru } from "../i18n/ru";
import { aDeliveryTask } from "../test/builders";
import { CYRILLIC_DEPARTMENT, deliveryClient, emptyTimeline, everyShape, everyShapeTimeline, GREEK_TASK_TITLE, pinnedDelivery } from "../test/deliveryStub";
import { renderApp } from "../test/render";
import { argsOf } from "../test/stubClient";

// DT8 and DT9 of docs/features/delivery-timeline.md "Acceptance criteria", on the stub client: the
// Delivery tab's View group, Tables first, Timeline reading again with `timeline` true; the rows in
// served order under their groups' headings with markers in words; each ID linking the Board and
// described by its title, its dates and served lateness, the dates line hidden; the tracks, the axis and the lines hidden from
// assistive tech and placed by an inline start and width alone; Show as table the same rows; the
// department filter; an empty timeline; the daemon's refusal in its words; 200 rows, then 200 more.

const AS_OF = "2026-11-20";

async function openTimeline(client = deliveryClient()) {
  renderApp(client, "#/alpha/roadmap");
  fireEvent.click(await screen.findByRole("tab", { name: "Delivery" }));
  await screen.findByRole("table", { name: "By release" });
  fireEvent.change(screen.getByLabelText("As of"), { target: { value: AS_OF } });
  await screen.findByRole("table", { name: "By release" });
  fireEvent.click(within(screen.getByRole("group", { name: "View" })).getByRole("button", { name: "Timeline" }));
  return client;
}

async function rows(client = deliveryClient()) {
  await openTimeline(client);
  await screen.findByRole("region", { name: "Tasks over time" });
  return client;
}

function row(id: string): HTMLElement {
  const found = Array.from(document.querySelectorAll<HTMLElement>(".delivery-timeline-row")).find((element) => element.dataset.task === id);
  if (found === undefined) {
    throw new Error(`no row ${id}`);
  }
  return found;
}

function rowIds(): (string | undefined)[] {
  return Array.from(document.querySelectorAll<HTMLElement>("[data-task]"), (element) => element.dataset.task);
}

/** A mark's inline place: its start and its width, as written. */
function place(element: Element | null): [string, string] {
  const style = (element as HTMLElement | null)?.style;
  return [style?.insetInlineStart ?? "", style?.width ?? ""];
}

/** A row's link's accessible description, as assistive tech computes it from every node `aria-describedby` names. */
function described(id: string): string {
  let description = "";
  within(row(id)).getByRole("link", {
    name: id,
    description: (computed) => {
      description = computed;
      return true;
    },
  });
  return description;
}

/** What browse mode reads of a row: its text outside what is hidden or hidden from assistive tech. */
function browsed(id: string): string {
  return Array.from(row(id).children)
    .filter((child) => !(child as HTMLElement).hidden && child.getAttribute("aria-hidden") !== "true")
    .map((child) => child.textContent)
    .join(" ");
}

describe("the View switch (DT8)", () => {
  it("offers Tables, pressed, then Timeline; Timeline reads again with `timeline` true, Tables never asks it", async () => {
    const client = deliveryClient();
    renderApp(client, "#/alpha/roadmap");
    fireEvent.click(await screen.findByRole("tab", { name: "Delivery" }));
    await screen.findByRole("table", { name: "By release" });
    const views = screen.getByRole("group", { name: "View" });
    expect(within(views).getAllByRole("button").map((button) => [button.textContent, button.getAttribute("aria-pressed")])).toEqual([
      ["Tables", "true"],
      ["Timeline", "false"],
    ]);
    expect(client.getDelivery.mock.calls.every(([, options]) => options.timeline === undefined)).toBe(true);
    fireEvent.click(within(views).getByRole("button", { name: "Timeline" }));
    await screen.findByRole("region", { name: "Tasks over time" });
    expect(within(views).getAllByRole("button").map((button) => button.getAttribute("aria-pressed"))).toEqual(["false", "true"]);
    expect(argsOf(client.getDelivery).at(-1)).toEqual(["alpha", { asOf: expect.any(String) as string, timeline: true }]);
    expect(screen.queryByRole("table", { name: "By release" })).toBeNull();
    fireEvent.click(within(views).getByRole("button", { name: "Tables" }));
    await screen.findByRole("table", { name: "By release" });
    expect(screen.queryByRole("region", { name: "Tasks over time" })).toBeNull();
  });

  it("keeps Timeline chosen while the tab stays open: a new day reads the timeline at it", async () => {
    const client = await rows();
    fireEvent.change(screen.getByLabelText("As of"), { target: { value: "2026-11-18" } });
    await waitFor(() => {
      expect(argsOf(client.getDelivery).at(-1)).toEqual(["alpha", { asOf: "2026-11-18", timeline: true }]);
    });
    expect(screen.getByRole("button", { name: "Timeline" }).getAttribute("aria-pressed")).toBe("true");
    expect(await screen.findByRole("region", { name: "Tasks over time" })).toBeTruthy();
  });
});

describe("the timeline's rows (DT8)", () => {
  it("rows each task in served order under its group's heading, the markers in words; the span and the day asked as text", async () => {
    await rows();
    expect(rowIds()).toEqual(["C-0001", "C-0003", "C-0004", "C-0005"]);
    const groups = Array.from(document.querySelectorAll(".delivery-timeline-group"), (group) => [
      group.querySelector("h2")?.textContent,
      group.querySelector(".delivery-timeline-markers")?.textContent,
      Array.from(group.querySelectorAll<HTMLElement>("[data-task]"), (element) => element.dataset.task),
    ]);
    expect(groups).toEqual([
      ["r1", "First taken 3 Nov 2026, last accepted 15 Nov 2026", ["C-0001"]],
      ["r2", "First taken 5 Nov 2026, nothing accepted yet", ["C-0003"]],
      ["No release", "First taken 3 Nov 2026, last accepted 5 Nov 2026", ["C-0004", "C-0005"]],
    ]);
    expect(document.querySelector(".delivery-timeline-span")?.textContent).toBe("From 1 Nov 2026 to 20 Nov 2026, as of 20 Nov 2026");
    // Each group's heading names its section.
    expect(screen.getByRole("region", { name: "r2" }).querySelectorAll("[data-task]")).toHaveLength(1);
  });

  it("links each ID to the Board, the title in bdi, the served late mark beside it", async () => {
    await rows();
    expect(within(row("C-0001")).getByRole("link", { name: "C-0001" }).getAttribute("href")).toBe("#/alpha/board/C-0001");
    expect(within(row("C-0005")).getByRole("link", { name: "C-0005" }).getAttribute("href")).toBe("#/alpha/board/C-0005");
    expect(row("C-0001").querySelector("bdi.delivery-timeline-title")?.textContent).toBe("Write the tide table foreword");
    expect(Array.from(document.querySelectorAll(".delivery-timeline-row"), (element) => element.querySelector(".lozenge")?.textContent ?? null)).toEqual([
      "Delivered 5 days late",
      "Overdue by 8 days",
      null,
      null,
    ]);
  });

  it("describes each link by its title, then its dates in words, the missing ones left out, then its served lateness", async () => {
    await rows();
    expect(described("C-0001")).toBe("Write the tide table foreword Created 1 Nov 2026. Taken 3 Nov 2026. Accepted 15 Nov 2026. Due 10 Nov 2026. Delivered 5 days late.");
    expect(described("C-0003")).toBe("Engrave the harbor map Created 4 Nov 2026. Taken 5 Nov 2026. Due 12 Nov 2026. Overdue by 8 days.");
    expect(described("C-0004")).toBe("Fix the index Created 2 Nov 2026. Taken 3 Nov 2026. Accepted 5 Nov 2026.");
    expect(described("C-0005")).toBe("Record the foghorn Created 6 Nov 2026.");
    const link = within(row("C-0005")).getByRole("link");
    const [titleId, datesId] = (link.getAttribute("aria-describedby") ?? "").split(" ");
    expect([titleId === row("C-0005").querySelector("bdi.delivery-timeline-title")?.id, datesId === row("C-0005").querySelector("[hidden]")?.id]).toEqual([true, true]);
  });

  it("hides the dates line, so browse mode reads the lateness once, from the mark (M: the dates line read in browse mode)", async () => {
    await rows();
    const dates = row("C-0001").querySelector<HTMLElement>("[hidden]");
    expect([dates?.hidden, dates?.textContent]).toEqual([true, "Created 1 Nov 2026. Taken 3 Nov 2026. Accepted 15 Nov 2026. Due 10 Nov 2026. Delivered 5 days late."]);
    expect(browsed("C-0001")).toBe("C-0001 Write the tide table foreword Delivered 5 days late");
    expect(browsed("C-0003").split("Overdue by 8 days")).toHaveLength(2);
  });

  it("hides the tracks, the axis and the lines from assistive tech, and places each mark between the served ends", async () => {
    await rows();
    for (const hidden of document.querySelectorAll(".delivery-timeline-track, .delivery-timeline-axis, .delivery-timeline-lines")) {
      expect(hidden.getAttribute("aria-hidden")).toBe("true");
    }
    expect(document.querySelectorAll(".delivery-timeline-track")).toHaveLength(4);
    // C-0001: waiting 11-01 to 11-03, work to 11-15, accepted, due 11-10; from 11-01, to 11-20.
    expect(place(row("C-0001").querySelector(".delivery-timeline-waiting"))).toEqual(["0%", "10.53%"]);
    expect(place(row("C-0001").querySelector(".delivery-timeline-work"))).toEqual(["10.53%", "63.16%"]);
    expect(row("C-0001").querySelector(".delivery-timeline-work")?.getAttribute("data-accepted")).toBe("true");
    expect(place(row("C-0001").querySelector(".delivery-timeline-due"))).toEqual(["47.37%", ""]);
    // C-0003: open, its work to the day asked, no accept mark.
    expect(place(row("C-0003").querySelector(".delivery-timeline-work"))).toEqual(["21.05%", "78.95%"]);
    expect(row("C-0003").querySelector(".delivery-timeline-work")?.getAttribute("data-accepted")).toBe("false");
    // C-0004: waiting 11-02 to 11-03, taken by its move to `review`, work to 11-05, accepted.
    expect(place(row("C-0004").querySelector(".delivery-timeline-waiting"))).toEqual(["5.26%", "5.26%"]);
    expect(place(row("C-0004").querySelector(".delivery-timeline-work"))).toEqual(["10.53%", "10.53%"]);
    expect(row("C-0004").querySelector(".delivery-timeline-work")?.getAttribute("data-accepted")).toBe("true");
    // C-0005: never taken: waiting to the day asked, no work, no due.
    expect(place(row("C-0005").querySelector(".delivery-timeline-waiting"))).toEqual(["26.32%", "73.68%"]);
    expect(row("C-0005").querySelector(".delivery-timeline-work, .delivery-timeline-due")).toBeNull();
    // r1's first take and last accept, and the day asked, across its rows.
    const lines = document.querySelector(".delivery-timeline-group .delivery-timeline-lines");
    expect(["first", "last", "as-of"].map((mark) => place(lines?.querySelector(`.delivery-timeline-${mark}`) ?? null)[0])).toEqual(["10.53%", "73.68%", "100%"]);
    // Every inline style is a start or a width.
    const styled = Array.from(document.querySelectorAll<HTMLElement>(".delivery-timeline [style]"), (element) => Array.from(element.style).sort().join(","));
    expect(new Set(styled)).toEqual(new Set(["inset-inline-start", "inset-inline-start,width"]));
  });

  it("ticks the axis on the month's first and on Mondays a tenth of the axis from it, in words, the As of mark at the day asked", async () => {
    await rows();
    const axis = document.querySelector(".delivery-timeline-axis");
    // The 2nd, a Monday 5.26% from November's first, is left out: their labels would sit on one another.
    expect(Array.from(axis?.querySelectorAll(".delivery-timeline-tick") ?? [], (tick) => [tick.textContent, place(tick)[0], tick.getAttribute("data-month")])).toEqual([
      ["Nov 2026", "0%", "true"],
      ["9", "42.11%", "false"],
      ["16", "78.95%", "false"],
    ]);
    expect([axis?.querySelector(".delivery-timeline-as-of-mark")?.textContent, place(axis?.querySelector(".delivery-timeline-as-of-mark") ?? null)[0]]).toEqual(["As of", "100%"]);
  });

  it("keeps the rows of one department: Print keeps C-0001, C-0003, C-0004; Words C-0001, C-0004; All every row", async () => {
    await rows();
    const field = screen.getByLabelText<HTMLSelectElement>("Department");
    expect(Array.from(field.options, (option) => [option.value, option.textContent])).toEqual([
      ["", "All"],
      ["words", "Words"],
      ["print", "Print"],
    ]);
    fireEvent.change(field, { target: { value: "print" } });
    expect(rowIds()).toEqual(["C-0001", "C-0003", "C-0004"]);
    fireEvent.change(field, { target: { value: "words" } });
    expect(rowIds()).toEqual(["C-0001", "C-0004"]);
    expect(Array.from(document.querySelectorAll(".delivery-timeline-group h2"), (heading) => heading.textContent)).toEqual(["r1", "No release"]);
    fireEvent.change(field, { target: { value: "" } });
    expect(rowIds()).toEqual(["C-0001", "C-0003", "C-0004", "C-0005"]);
  });

  it("shows the same rows as a table, Show as timeline back; a filter kept across", async () => {
    await rows();
    fireEvent.change(screen.getByLabelText("Department"), { target: { value: "print" } });
    fireEvent.click(screen.getByRole("button", { name: "Show as table" }));
    const table = screen.getByRole("table", { name: "Timeline" });
    expect(Array.from(table.querySelectorAll("tr"), (line) => Array.from(line.children, (cell) => cell.textContent))).toEqual([
      ["Task", "Release", "Created", "Taken", "Accepted", "Due", "Late"],
      ["C-0001 Write the tide table foreword", "r1", "2026-11-01 09:00 UTC", "2026-11-03 09:00 UTC", "2026-11-15 09:00 UTC", "2026-11-10", "Delivered 5 days late"],
      ["C-0003 Engrave the harbor map", "r2", "2026-11-04 09:00 UTC", "2026-11-05 09:00 UTC", "None", "2026-11-12", "Overdue by 8 days"],
      ["C-0004 Fix the index", "No release", "2026-11-02 09:00 UTC", "2026-11-03 09:00 UTC", "2026-11-05 09:00 UTC", "None", "None"],
    ]);
    expect(within(table).getByRole("link", { name: "C-0003" }).getAttribute("href")).toBe("#/alpha/board/C-0003");
    expect(table.closest("[role=region]")?.getAttribute("tabindex")).toBe("0");
    expect(screen.queryByRole("region", { name: "Tasks over time" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Show as timeline" }));
    expect(screen.getByRole("region", { name: "Tasks over time" }).getAttribute("tabindex")).toBe("0");
    expect(rowIds()).toEqual(["C-0001", "C-0003", "C-0004"]);
  });
});

describe("every shape (DT8)", () => {
  it("labels the pool, the unplaced items and an unknown word raw in lang=en; says a marker with nothing taken", async () => {
    await rows(deliveryClient(everyShape(), everyShapeTimeline()));
    const headings = Array.from(document.querySelectorAll(".delivery-timeline-group h2 bdi"), (heading) => [heading.textContent, heading.getAttribute("lang")]);
    expect(headings).toEqual([
      ["r1", null],
      ["r2", null],
      ["Pool", null],
      ["Unplaced", null],
      ["milestone", "en"],
      ["No release", null],
    ]);
    expect(Array.from(document.querySelectorAll(".delivery-timeline-markers"), (line) => line.textContent)[3]).toBe("Nothing taken yet, nothing accepted yet");
  });

  it("shows the served lateness, never one its dates would give (M: lateness computed from `due` in the UI)", async () => {
    await rows(deliveryClient(everyShape(), everyShapeTimeline()));
    // C-0010: due 11-06, accepted 11-09 by its dates, three days; served two.
    expect(row("C-0010").querySelector(".lozenge")?.textContent).toBe("Delivered 2 days late");
    expect(described("C-0010")).toMatch(/ Delivered 2 days late\.$/);
    // C-0009 is served no lateness: no mark, whatever its dates.
    expect(row("C-0009").querySelector(".lozenge")).toBeNull();
  });

  it("draws a take stored before its creation as an empty waiting stretch; a Greek title and a Cyrillic department as written", async () => {
    await rows(deliveryClient(everyShape(), everyShapeTimeline()));
    expect(place(row("C-0007").querySelector(".delivery-timeline-waiting"))).toEqual(["26.32%", "0%"]);
    expect(row("C-0007").querySelector("bdi.delivery-timeline-title")?.textContent).toBe(GREEK_TASK_TITLE);
    const field = screen.getByLabelText<HTMLSelectElement>("Department");
    expect(Array.from(field.options, (option) => option.textContent)).toContain(CYRILLIC_DEPARTMENT);
    fireEvent.change(field, { target: { value: "sound" } });
    expect(rowIds()).toEqual(["C-0007"]);
  });

  it("says a department holding no row in the same line, no chart, no table", async () => {
    await rows(deliveryClient(everyShape()));
    fireEvent.change(screen.getByLabelText("Department"), { target: { value: "sound" } });
    expect(document.querySelector(".delivery-timeline-shown")?.textContent).toBe("No task to place in time");
    expect(screen.queryByRole("region", { name: "Tasks over time" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Show as table" }));
    expect(screen.queryByRole("table", { name: "Timeline" })).toBeNull();
    fireEvent.change(screen.getByLabelText("Department"), { target: { value: "" } });
    expect(screen.getByRole("table", { name: "Timeline" }).querySelectorAll("tbody tr")).toHaveLength(4);
  });

  it("says an empty timeline in its line, no region, no row", async () => {
    await openTimeline(deliveryClient(pinnedDelivery(), emptyTimeline()));
    expect((await screen.findByRole("heading", { name: "No task to place in time" })).tagName).toBe("H3");
    expect(screen.queryByRole("region", { name: "Tasks over time" })).toBeNull();
    expect(document.querySelector("[data-task]")).toBeNull();
  });

  it("says the daemon's refusal in its words with Retry; Retry reads the timeline again", async () => {
    const client = deliveryClient();
    const message = "unknown query parameter `timeline`: the delivery route takes `as_of`";
    client.getDelivery.mockImplementation((_project, options) =>
      options.timeline === true ? Promise.reject(new ClientError({ status: 400, message })) : Promise.resolve({ ...pinnedDelivery(), as_of: options.asOf }),
    );
    await openTimeline(client);
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toBe(`The delivery could not be read${message}`);
    const calls = client.getDelivery.mock.calls.length;
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    await waitFor(() => {
      expect(client.getDelivery.mock.calls.length).toBeGreaterThan(calls);
    });
    expect(argsOf(client.getDelivery).at(-1)?.[1]).toEqual({ asOf: AS_OF, timeline: true });
  });
});

/** A timeline of `count` tasks in five releases, as `large` serves one: a department each of four. */
function largeTimeline(count: number): DeliveryTimeline {
  const releases = ["keel", "hull", "sails", "masts", "rigging"];
  const tasks = Array.from({ length: count }, (_, index) => {
    const release = releases[Math.floor((index * releases.length) / count)] ?? "keel";
    const made = `2026-${String(7 + Math.floor((index % 120) / 30)).padStart(2, "0")}-${String((index % 28) + 1).padStart(2, "0")}T09:00:00Z`;
    return aDeliveryTask({
      id: `C-${String(index + 1).padStart(4, "0")}`,
      group: "release",
      release,
      departments: [["words", "print", "sound", "rope"][index % 4] ?? "words"],
      created_at: made,
      taken_at: index % 10 === 0 ? null : made,
      accepted_at: index % 2 === 0 ? null : "2026-11-10T09:00:00Z",
      due: index % 3 === 0 ? null : "2026-11-05",
      days_late: index % 7 === 0 ? 3 : null,
    });
  });
  return {
    from: "2026-07-01",
    to: "2026-11-20",
    markers: releases.map((release) => ({ group: "release", release, first_taken_at: "2026-07-01T09:00:00Z", last_accepted_at: "2026-11-10T09:00:00Z" })),
    tasks,
  };
}

describe("200 rows at a time (DT9)", () => {
  it("mounts exactly 200 of 2 000 rows, at most 12 elements a row; Show 200 more 400, the focus on the first new row (M: every row mounted at once)", async () => {
    await rows(deliveryClient(pinnedDelivery(), largeTimeline(2000)));
    expect(document.querySelectorAll(".delivery-timeline-row")).toHaveLength(200);
    const sizes = Array.from(document.querySelectorAll(".delivery-timeline-row"), (element) => 1 + element.querySelectorAll("*").length);
    expect(Math.max(...sizes)).toBeLessThanOrEqual(12);
    const more = screen.getByRole("button", { name: "Show 200 more (1800 left)" });
    fireEvent.click(more);
    await waitFor(() => {
      expect(document.querySelectorAll(".delivery-timeline-row")).toHaveLength(400);
    });
    expect(document.activeElement?.textContent).toBe("C-0201");
    expect(screen.getByRole("button", { name: "Show 200 more (1600 left)" })).toBeTruthy();
  });

  it("pages the table alike, and starts again at 200 on a new department", async () => {
    await rows(deliveryClient(pinnedDelivery(), largeTimeline(450)));
    fireEvent.click(screen.getByRole("button", { name: "Show 200 more (250 left)" }));
    await waitFor(() => {
      expect(document.querySelectorAll(".delivery-timeline-row")).toHaveLength(400);
    });
    fireEvent.click(screen.getByRole("button", { name: "Show 50 more (50 left)" }));
    await waitFor(() => {
      expect(document.querySelectorAll(".delivery-timeline-row")).toHaveLength(450);
    });
    expect(screen.queryByRole("button", { name: /^Show \d+ more/ })).toBeNull();
    fireEvent.change(screen.getByLabelText("Department"), { target: { value: "print" } });
    expect(document.querySelectorAll(".delivery-timeline-row")).toHaveLength(113);
    fireEvent.click(screen.getByRole("button", { name: "Show as table" }));
    expect(screen.getByRole("table", { name: "Timeline" }).querySelectorAll("tbody tr")).toHaveLength(113);
    fireEvent.change(screen.getByLabelText("Department"), { target: { value: "" } });
    expect(screen.getByRole("table", { name: "Timeline" }).querySelectorAll("tbody tr")).toHaveLength(200);
  });
});

describe("the timeline's words (DT8)", () => {
  it("holds every timeline leaf in both catalogs, Russian plural forms for more, none saying block", () => {
    const stem = "bl" + "ock";
    const russianStem = "\u0431\u043b\u043e\u043a";
    const keysOf = (group: object) => Object.keys(group).sort();
    expect(keysOf(ru.roadmap.delivery.timeline)).toEqual(
      keysOf(en.roadmap.delivery.timeline)
        .filter((key) => key !== "more_one" && key !== "more_other")
        .concat(["more_few", "more_many", "more_one", "more_other"])
        .sort(),
    );
    expect(keysOf(ru.roadmap.delivery.views)).toEqual(keysOf(en.roadmap.delivery.views));
    const values = [...Object.values(en.roadmap.delivery.timeline), ...Object.values(en.roadmap.delivery.views)];
    expect(values.filter((value) => value.toLowerCase().includes(stem))).toEqual([]);
    expect(Object.values(ru.roadmap.delivery.timeline).filter((value) => value.toLowerCase().includes(russianStem) || value.toLowerCase().includes(stem))).toEqual([]);
  });
});
