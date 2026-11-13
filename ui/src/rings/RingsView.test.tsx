import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { beforeAll, describe, expect, it } from "vitest";
import { ClientError } from "../api/client";
import type { RingsDocument } from "../api/types";
import { docOf } from "../app/routes";
import { rollupLook } from "../flow/labels";
import appCss from "../styles/app.css?raw";
import { aRing, aRings, aRingSector } from "../test/builders";
import { renderApp, showRings } from "../test/render";
import { stubClient, type StubClient } from "../test/stubClient";
import { enT, ruT } from "../test/text";
import { sectorKeys, sectorName } from "./RingsPicture";

// docs/features/scope-rings.md "Acceptance criteria", SR7, on the stub client at `#/alpha/graph`:
// the sectors in served order, each a link named in words; one tab stop, the arrows, Home and End;
// Enter or a 4 px press opens the document, a 6 px press nothing; the search and a state chip dim;
// the List holds the same rows by ring; each state by icon and label; no ring and notes; the wheel
// never taken. M: every sector a tab stop; the fill keyed on a state word outside labels.ts.

/** Two rings and an empty one: every rollup, a word this build does not know, a sector with no ID, title or kind. */
function answer(): RingsDocument {
  return aRings([
    aRing("Inner", [
      aRingSector({ name: "SCN-A", title: "Arrival", state: "in work" }),
      aRingSector({ name: "SCN-B", title: "Storm", state: "confirmed" }),
      aRingSector({ name: "SCN-C", title: "Calm", state: "no card", status: "draft" }),
    ]),
    aRing(
      "Outer",
      [
        aRingSector({ name: "CUE-1", kind: "cue", title: "Bell", state: "suspect" }),
        aRingSector({ name: "CUE-2", kind: "cue", title: "Horn", state: "departed" }),
        aRingSector({ name: "CUE-3", kind: "cue", title: "Gong", state: "parked" }),
        aRingSector({ name: "docs/notes/loose.md", id: null, kind: null, title: null, path: "docs/notes/loose.md" }),
      ],
      { root: "DOC-ROOT", kinds: ["scene", "cue"] },
    ),
    aRing("Empty", [], { root: "DOC-NONE", kinds: null }),
  ]);
}

function client(rings: RingsDocument = answer()): StubClient {
  const stub = stubClient();
  stub.getRings.mockImplementation(() => Promise.resolve(structuredClone(rings)));
  return stub;
}

/** The drawing's sectors, in DOM order. */
function sectors(): SVGPathElement[] {
  return Array.from(document.querySelectorAll<SVGPathElement>(".rings-svg .rings-sector"));
}

function sector(label: string): SVGPathElement {
  const found = sectors().find((element) => element.getAttribute("aria-label")?.startsWith(`${label},`));
  if (found === undefined) {
    throw new Error(`no sector ${label}`);
  }
  return found;
}

function dimmed(): string[] {
  return sectors()
    .filter((element) => element.classList.contains("is-outlined"))
    .map((element) => (element.getAttribute("aria-label") ?? "").split(",")[0] ?? "");
}

async function drawn(rings: RingsDocument = answer()): Promise<StubClient> {
  const stub = client(rings);
  renderApp(stub, "#/alpha/graph");
  await showRings();
  await screen.findByRole("group", { name: "Scope rings" });
  return stub;
}

function key(name: string) {
  const target = document.activeElement;
  if (target === null) {
    throw new Error("nothing focused");
  }
  fireEvent.keyDown(target, { key: name });
}

/** A press at (100, 100), a move by `dx`, its release and its click, as a pointer sends them. */
function press(target: Element, dx: number) {
  fireEvent.pointerDown(target, { clientX: 100, clientY: 100, button: 0 });
  fireEvent.pointerUp(target, { clientX: 100 + dx, clientY: 100, button: 0 });
  fireEvent.click(target, { clientX: 100 + dx, clientY: 100, button: 0 });
}

// The view's chunk loaded once before the first test, so its first transform never eats a query's wait.
beforeAll(async () => {
  await import("./RingsView");
});

describe("the rings drawn (SR7)", () => {
  it("draws the sectors in served order, ring by ring, each a link named by title, kind, state in words and ring", async () => {
    const stub = await drawn();
    expect(stub.getRings).toHaveBeenCalledTimes(1);
    expect(stub.getRings.mock.calls[0]?.[0]).toBe("alpha");
    expect(stub.getRings.mock.calls[0]?.[1]).toBeInstanceOf(AbortSignal);
    expect(sectors().map((element) => element.getAttribute("aria-label"))).toEqual([
      "Arrival, scene, In work, ring Inner",
      "Storm, scene, Confirmed, ring Inner",
      "Calm, scene, No task, ring Inner",
      "Bell, cue, Suspect, ring Outer",
      "Horn, cue, Departed, ring Outer",
      "Gong, cue, parked, ring Outer",
      "docs/notes/loose.md, no kind, No task, ring Outer",
    ]);
    expect(sectors().every((element) => element.getAttribute("role") === "link")).toBe(true);
    // The heading under the Graph's h1, the empty ring a band with its title and 0.
    expect(screen.getByRole("heading", { level: 2, name: "Scope rings" })).toBeTruthy();
    expect(document.querySelectorAll(".rings-svg .rings-band")).toHaveLength(1);
    expect(Array.from(document.querySelectorAll(".rings-svg .rings-title textPath"), (path) => path.textContent)).toEqual(["Inner", "Outer", "Empty"]);
    expect(document.querySelector(".rings-svg .rings-count")?.textContent).toBe("0");
  });

  it("fills each sector by its state's look in src/flow/labels.ts, an unknown word neutral (M: the fill keyed on a state word outside labels.ts)", async () => {
    await drawn();
    const states = answer().rings.flatMap((ring) => ring.sectors.map((entry) => entry.state));
    expect(sectors().map((element) => element.getAttribute("data-tone"))).toEqual(states.map((state) => rollupLook(state, enT).tone));
    expect(sectors().map((element) => element.classList.contains(`tone-${element.getAttribute("data-tone") ?? ""}`))).toEqual(states.map(() => true));
    expect(sector("Gong").getAttribute("data-tone")).toBe("trace-unknown");
  });

  it("names a sector in the viewer's language, the served words as served: a title in any script, a kind, a word no table knows", () => {
    const ring = aRing("\u0412\u043d\u0443\u0442\u0440\u0438", []);
    expect(sectorName(aRingSector({ name: "SCN-A", title: "Arrival", state: "in work" }), ring, enT)).toBe("Arrival, scene, In work, ring \u0412\u043d\u0443\u0442\u0440\u0438");
    // In Russian: the state's words and the ring's word translated, the rest as served.
    expect(sectorName(aRingSector({ name: "SCN-A", title: "Arrival", state: "in work" }), ring, ruT)).toBe(
      "Arrival, scene, \u0412 \u0440\u0430\u0431\u043e\u0442\u0435, \u043a\u043e\u043b\u044c\u0446\u043e \u0412\u043d\u0443\u0442\u0440\u0438",
    );
    expect(sectorName(aRingSector({ name: "docs/a.md", id: null, kind: null, title: null, state: "parked" }), ring, ruT)).toBe(
      "docs/a.md, \u0431\u0435\u0437 \u0432\u0438\u0434\u0430, parked, \u043a\u043e\u043b\u044c\u0446\u043e \u0412\u043d\u0443\u0442\u0440\u0438",
    );
  });
});

describe("the sectors' keys", () => {
  it("key each sector by its name, a repeat by how many before it share it, so a ring holding one node twice draws both (M: the name alone)", async () => {
    expect(sectorKeys([{ sector: aRingSector({ name: "A-1" }) }, { sector: aRingSector({ name: "B-1" }) }, { sector: aRingSector({ name: "A-1" }) }])).toEqual(["A-1\u00000", "B-1\u00000", "A-1\u00001"]);
    await drawn(aRings([aRing("Twice", [aRingSector({ name: "DUP-1", title: "Twice told" }), aRingSector({ name: "DUP-1", title: "Twice told" })])]));
    expect(sectors().map((element) => element.getAttribute("aria-label"))).toEqual(["Twice told, scene, No task, ring Twice", "Twice told, scene, No task, ring Twice"]);
  });
});

describe("the keyboard (SR7)", () => {
  it("is one tab stop: one sector at tabindex 0, the first, every other -1, the drawing none (M: every sector a tab stop)", async () => {
    await drawn();
    expect(sectors().map((element) => element.getAttribute("tabindex"))).toEqual(["0", "-1", "-1", "-1", "-1", "-1", "-1"]);
    expect(document.querySelector(".rings-svg")?.hasAttribute("tabindex")).toBe(false);
  });

  it("walks Right along the ring, Down to the next ring's nearest sector, Up back, Home and End to the ring's ends; the tab stop follows", async () => {
    await drawn();
    act(() => {
      sector("Arrival").focus();
    });
    key("ArrowRight");
    expect(document.activeElement).toBe(sector("Storm"));
    // Storm's middle stands at 180 degrees; the outer ring's 135 degrees and 225 degrees tie, the first wins.
    key("ArrowDown");
    expect(document.activeElement).toBe(sector("Horn"));
    // No ring further out holds a sector: Down stays.
    key("ArrowDown");
    expect(document.activeElement).toBe(sector("Horn"));
    key("End");
    expect(document.activeElement).toBe(sector("docs/notes/loose.md"));
    key("ArrowRight");
    expect(document.activeElement).toBe(sector("Bell"));
    key("ArrowLeft");
    expect(document.activeElement).toBe(sector("docs/notes/loose.md"));
    key("Home");
    expect(document.activeElement).toBe(sector("Bell"));
    // Bell's middle stands at 45 degrees: the inner ring's nearest is Arrival's 60 degrees.
    key("ArrowUp");
    expect(document.activeElement).toBe(sector("Arrival"));
    key("ArrowLeft");
    expect(document.activeElement).toBe(sector("Calm"));
    expect(sectors().filter((element) => element.getAttribute("tabindex") === "0")).toEqual([sector("Calm")]);
    // Calm's middle stands at 300 degrees: the outer ring's nearest is its last, at 315, not its third.
    key("ArrowDown");
    expect(document.activeElement).toBe(sector("docs/notes/loose.md"));
  });

  it("lists the rings' walk keys and the Map's Esc with ?", async () => {
    await drawn();
    act(() => {
      sector("Arrival").focus();
    });
    key("?");
    const dialog = await screen.findByRole("dialog", { name: "Keyboard shortcuts" });
    for (const said of [
      "Rings: the next or previous sector along the ring",
      "Rings: Up to the nearest sector one ring in, Down one ring out",
      "Rings: the ring's first or last sector",
      "Rings: open the sector's document",
      "Map: clear the lit node",
    ]) {
      expect(within(dialog).getByText(said)).toBeTruthy();
    }
  });

  it("opens the focused sector's document with Enter: `doc=<name>` on the route", async () => {
    await drawn();
    act(() => {
      sector("docs/notes/loose.md").focus();
    });
    key("Enter");
    await waitFor(() => {
      expect(docOf(window.location.hash)).toBe("docs/notes/loose.md");
    });
    expect(window.location.hash.startsWith("#/alpha/graph?")).toBe(true);
  });
});

describe("a press (SR7)", () => {
  it("opens the document on a 4 px press and on none at 6 px", async () => {
    await drawn();
    press(sector("Horn"), 6);
    expect(docOf(window.location.hash)).toBeNull();
    press(sector("Horn"), 4);
    await waitFor(() => {
      expect(docOf(window.location.hash)).toBe("CUE-2");
    });
  });

  it("opens the document on a click no press began, as assistive technology sends one (M: a bare click refused)", async () => {
    await drawn();
    // An SVG element has no click(): the event HTMLElement.click() would send, and no pointerdown before it.
    act(() => {
      sector("Gong").dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
    });
    await waitFor(() => {
      expect(docOf(window.location.hash)).toBe("CUE-3");
    });
  });

  it("forgets a press that left the drawing, so a later bare click still opens; a recorded 6 px press still opens nothing", async () => {
    await drawn();
    fireEvent.pointerDown(sector("Horn"), { clientX: 100, clientY: 100, button: 0 });
    fireEvent.pointerLeave(document.querySelector(".rings-svg") as Element);
    act(() => {
      sector("Bell").dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
    });
    await waitFor(() => {
      expect(docOf(window.location.hash)).toBe("CUE-1");
    });
  });

  it("never takes the wheel: a wheel on the drawing or a sector is not prevented, and nothing pans or zooms", async () => {
    await drawn();
    for (const target of [document.querySelector(".rings-svg"), sector("Bell")]) {
      if (target === null) {
        throw new Error("no drawing");
      }
      const wheel = new WheelEvent("wheel", { bubbles: true, cancelable: true, deltaY: 120 });
      target.dispatchEvent(wheel);
      expect(wheel.defaultPrevented).toBe(false);
    }
    expect(document.querySelector(".rings-svg")?.getAttribute("viewBox")).toBe("0 0 1000 1000");
    expect(document.querySelector(".rings-svg [transform*='scale']")).toBeNull();
  });
});

describe("the card (SR7)", () => {
  it("shows a hovered sector's title, kind, state by icon and label, raw status in English and place; a focused one's too", async () => {
    await drawn();
    fireEvent.pointerOver(sector("Calm"));
    const card = document.querySelector<HTMLElement>(".rings-card");
    expect(card?.querySelector(".rings-card-title")?.textContent).toBe("Calm");
    expect(card?.querySelector(".mono")?.textContent).toBe("scene");
    expect(card?.querySelector(".badge .icon")).not.toBeNull();
    expect(card?.querySelector(".badge-label")?.textContent).toBe("No task");
    expect(card?.querySelector(".rings-card-status")?.getAttribute("lang")).toBe("en");
    expect(card?.querySelector(".rings-card-status")?.textContent).toBe("draft");
    expect(card?.querySelector(".rings-card-place")?.textContent).toBe("docs/spec/scn-c.md:1");
    fireEvent.pointerOut(sector("Calm"));
    expect(document.querySelector(".rings-card")).toBeNull();
    act(() => {
      sector("Bell").focus();
    });
    expect(document.querySelector(".rings-card-title")?.textContent).toBe("Bell");
  });

  it("shows the card over the drawing's corner, out of the page's flow: the legend's column and place never change (M: the card in the legend's column)", async () => {
    await drawn();
    const side = document.querySelector<HTMLElement>(".rings-side");
    const column = () => Array.from(side?.children ?? [], (child) => child.className);
    expect(column()).toEqual(["rings-legend"]);
    fireEvent.pointerOver(sector("Storm"));
    const overlay = document.querySelector<HTMLElement>(".rings-overlay");
    expect(overlay?.querySelector(".rings-card-title")?.textContent).toBe("Storm");
    expect(overlay?.parentElement?.className).toBe("rings-picture");
    expect(overlay?.parentElement).toBe(document.querySelector(".rings-svg")?.parentElement);
    expect(column()).toEqual(["rings-legend"]);
    const rule = (selector: string) => new RegExp(`(^|\\n)${selector.replace(/[.]/g, "\\.")} \\{([^}]*)\\}`).exec(appCss)?.[2] ?? "";
    expect(rule(".rings-picture")).toMatch(/position:\s*relative/);
    expect(rule(".rings-overlay")).toMatch(/position:\s*absolute/);
    expect(rule(".rings-overlay")).toMatch(/pointer-events:\s*none/);
  });
});

describe("the search and the chips (SR7)", () => {
  it("dims every sector the search misses, title or name, any case; cleared, none", async () => {
    await drawn();
    fireEvent.change(screen.getByLabelText("Search the rings"), { target: { value: "STORM" } });
    expect(dimmed()).toEqual(["Arrival", "Calm", "Bell", "Horn", "Gong", "docs/notes/loose.md"]);
    fireEvent.change(screen.getByLabelText("Search the rings"), { target: { value: "cue-" } });
    expect(dimmed()).toEqual(["Arrival", "Storm", "Calm", "docs/notes/loose.md"]);
    fireEvent.change(screen.getByLabelText("Search the rings"), { target: { value: "" } });
    expect(dimmed()).toEqual([]);
    // Dimmed sectors stay where they are, links still.
    expect(sectors()).toHaveLength(7);
  });

  it("offers one chip per state present in ROLLUP_WORDS' order, the unknown last, each with icon, label and count; released, its state's fill goes", async () => {
    await drawn();
    const group = screen.getByRole("group", { name: "States" });
    const chips = within(group).getAllByRole("button");
    expect(chips.map((chip) => chip.textContent.replace(/\s+/g, " ").trim())).toEqual(["Confirmed 1", "In work 1", "Suspect 1", "Departed 1", "No task 2", "parked 1"]);
    expect(chips.every((chip) => chip.getAttribute("aria-pressed") === "true")).toBe(true);
    expect(chips.every((chip) => chip.querySelectorAll("svg.icon").length >= 1)).toBe(true);
    fireEvent.click(within(group).getByRole("button", { name: /^No task/ }));
    expect(within(group).getByRole("button", { name: /^No task/ }).getAttribute("aria-pressed")).toBe("false");
    expect(dimmed()).toEqual(["Calm", "docs/notes/loose.md"]);
    fireEvent.click(within(group).getByRole("button", { name: /^No task/ }));
    expect(dimmed()).toEqual([]);
  });
});

describe("the drawing's styles (SR7)", () => {
  it("fills a sector by its tone's colour, an outlined one with nothing but the control border, the labels primary text on a halo of the surface", () => {
    const rule = (selector: string) => new RegExp(`(^|\\n)${selector.replace(/[.]/g, "\\.")} \\{([^}]*)\\}`).exec(appCss)?.[2] ?? "";
    expect(rule(".rings-sector")).toContain("fill: var(--badge-color);");
    expect(rule(".rings-sector.is-outlined")).toContain("fill: transparent;");
    expect(rule(".rings-sector.is-outlined")).toContain("stroke: var(--border-control);");
    expect(rule(".rings-focus")).toContain("stroke: var(--focus-ring);");
    // One ring on a focused sector, the path's: the box outline transparent, never removed.
    expect(rule(".rings-sector:focus-visible")).toContain("outline-color: transparent;");
    const words = /\n\.rings-label,\n\.rings-title,\n\.rings-count \{([^}]*)\}/.exec(appCss)?.[1] ?? "";
    expect(words).toContain("fill: var(--text-primary);");
    expect(words).toContain("stroke: var(--surface-canvas);");
    expect(words).toContain("paint-order: stroke;");
  });
});

describe("the legend (SR7)", () => {
  it("names each state by swatch, icon and label, never colour alone, then the rings innermost first with their counts", async () => {
    await drawn();
    const legend = screen.getByRole("region", { name: "Legend" });
    const states = within(legend).getByRole("list", { name: "States" });
    const items = within(states).getAllByRole("listitem");
    expect(items.map((item) => item.textContent)).toEqual(["Confirmed", "In work", "Suspect", "Departed", "No task", "parked"]);
    for (const item of items) {
      expect(item.querySelector(".rings-swatch")).not.toBeNull();
      expect(item.querySelector("svg.icon")).not.toBeNull();
    }
    expect(items.at(-1)?.querySelector("[lang='en']")?.textContent).toBe("parked");
    const rings = within(legend).getByRole("list", { name: "Scope rings" });
    expect(within(rings).getAllByRole("listitem").map((item) => item.textContent)).toEqual(["Inner 3 sectors", "Outer 4 sectors", "Empty 0 sectors"]);
  });
});

describe("the List (SR7)", () => {
  it("holds the same rows by ring: a section per ring, its filters in its own words, each row a document link, its kind, state and status", async () => {
    await drawn();
    fireEvent.click(screen.getByRole("tab", { name: "List" }));
    const list = document.querySelector<HTMLElement>(".rings-list");
    if (list === null) {
      throw new Error("no List");
    }
    expect(within(list).getAllByRole("heading", { level: 3 }).map((heading) => heading.textContent)).toEqual(["Inner", "Outer", "Empty"]);
    expect(Array.from(list.querySelectorAll(".rings-list-where"), (line) => [line.textContent, line.getAttribute("lang")])).toEqual([
      ["kinds scene", "en"],
      ["root DOC-ROOT \u00b7 kinds scene, cue", "en"],
      ["root DOC-NONE", "en"],
    ]);
    const links = within(list).getAllByRole("link");
    expect(links.map((link) => link.textContent)).toEqual(["Arrival", "Storm", "Calm", "Bell", "Horn", "Gong", "docs/notes/loose.md"]);
    expect(links[0]?.getAttribute("href")).toBe("#/alpha/tree/SCN-A/page");
    const calm = links[2]?.closest("li");
    expect(calm?.querySelector(".badge-label")?.textContent).toBe("No task");
    expect(calm?.querySelector(".rings-list-status")?.textContent).toBe("draft");
    expect(calm?.querySelector(".rings-list-status")?.getAttribute("lang")).toBe("en");
    // The search and the chips apply to the List alike.
    fireEvent.change(screen.getByLabelText("Search the rings"), { target: { value: "o" } });
    expect(within(list).getAllByRole("link").map((link) => link.textContent)).toEqual(["Storm", "Horn", "Gong", "docs/notes/loose.md"]);
    fireEvent.click(within(screen.getByRole("group", { name: "States" })).getByRole("button", { name: /^Departed/ }));
    expect(within(list).getAllByRole("link").map((link) => link.textContent)).toEqual(["Storm", "Gong", "docs/notes/loose.md"]);
    // A link opens the document modal.
    fireEvent.click(within(list).getByRole("link", { name: "Storm" }));
    await waitFor(() => {
      expect(docOf(window.location.hash)).toBe("SCN-B");
    });
  });
});

describe("the states (SR7)", () => {
  it("with no ring and no note, says none is configured, the next step and the example in mono", async () => {
    renderApp(client(aRings([])), "#/alpha/graph");
    await showRings();
    expect(await screen.findByRole("heading", { level: 3, name: "No rings configured" })).toBeTruthy();
    expect(screen.getByText(/each with a/).textContent).toBe("Add [[map.ring]] tables to specengine.toml, each with a title and a root, kinds or both.");
    const example = document.querySelector(".rings-example");
    expect(example?.classList.contains("mono")).toBe(true);
    expect(example?.textContent).toBe('[[map.ring]]\ntitle = "<title>"\nroot = "<REF>"\nkinds = ["<kind>"]');
    expect(document.querySelector(".rings-svg")).toBeNull();
  });

  it("with notes and no ring, shows the notes verbatim in English, then the next step", async () => {
    const note = "specengine.toml:12: at most 12 `[[map.ring]]` tables; no ring is served";
    renderApp(client(aRings([], { notes: [note] })), "#/alpha/graph");
    await showRings();
    const notes = await screen.findByRole("list", { name: "Config notes" });
    expect(Array.from(notes.querySelectorAll("li"), (item) => [item.textContent, item.getAttribute("lang")])).toEqual([[note, "en"]]);
    expect(document.querySelector(".rings-example")).not.toBeNull();
    expect(screen.queryByRole("heading", { name: "No rings configured" })).toBeNull();
  });

  it("with notes and rings, shows the notes above the rings", async () => {
    const note = "rings: ring 2 `Outer`: `DOC-ROOT` resolves to no node";
    await drawn({ ...answer(), notes: [note] });
    expect(screen.getByRole("list", { name: "Config notes" }).textContent).toBe(note);
  });

  it("shows a skeleton while reading, then a refusal in the daemon's words with Retry, and the rings after it", async () => {
    const stub = client();
    let fail: (reason: unknown) => void = () => undefined;
    stub.getRings.mockImplementationOnce(
      () =>
        new Promise((_resolve, reject) => {
          fail = reject;
        }),
    );
    renderApp(stub, "#/alpha/graph");
    await showRings();
    expect(await screen.findByRole("status", { name: "Reading the rings" })).toBeTruthy();
    const message = "spec rings: the queue's schema 14 is newer than this binary's 13";
    act(() => {
      fail(new ClientError({ status: 503, message }));
    });
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("The rings could not be read");
    expect(alert.querySelector(".error-message")?.textContent).toBe(message);
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(await screen.findByRole("group", { name: "Scope rings" })).toBeTruthy();
    expect(stub.getRings).toHaveBeenCalledTimes(2);
  });
});
