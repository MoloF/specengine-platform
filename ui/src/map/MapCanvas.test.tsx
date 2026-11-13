import { act, fireEvent, render, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { pathsOf, pensPerCanvas, visibleAndBase, type RecordingPen } from "../test/pen";
import { labelledOf, MAP_DIM_ALPHA, type MapPicture, type MapView } from "./draw";
import type { Kept, Lit } from "./filters";
import { gridOf } from "./hit";
import { BASE_SETTLE_MS, MapCanvas, PAN_STEP, UNMEASURED, ZOOM_STEP, type MapCommand } from "./MapCanvas";

// docs/features/corpus-map.md "Acceptance criteria", CM7, on the canvas alone, each 2D context a
// recording pen (the page's canvas and the offscreen base apart): each node filled with its slot's role as the hidden swatch resolves it, read again
// when the theme changes; a press moving 4 px opens the node under it, one moving 6 px pans and opens
// nothing; the wheel is never prevented and moves nothing; `+`, `=`, `-`, `0`, the arrows and Esc act
// only with the region focused. M: the wheel zooming; CLICK_DISTANCE 0.

/** Three nodes on a row through the map's middle, 100 units apart; slots 1, 2 and the grey. */
function picture(): MapPicture {
  const positions = Float64Array.from([-100, 0, 0, 0, 100, 0]);
  const radii = Float64Array.from([10, 10, 10]);
  const ghost = new Uint8Array(3);
  return {
    positions,
    radii,
    slots: Uint8Array.from([1, 2, 0]),
    ghost,
    orphan: new Uint8Array(3),
    labels: ["A", "B", "C"],
    labelled: labelledOf(radii, ghost),
    lineFrom: Int32Array.from([0]),
    lineTo: Int32Array.from([1]),
    lineStyle: ["strong"],
  };
}

const KEPT: Kept = { points: Uint8Array.from([1, 1, 1]), lines: Uint8Array.from([1]) };
const VIEW: MapView = { x: 0, y: 0, zoom: 1 };

/** Where a map point sits on the screen in a box the test DOM leaves unmeasured. */
function screenOf(x: number, y: number) {
  return { clientX: UNMEASURED.width / 2 + x, clientY: UNMEASURED.height / 2 + y };
}

let pens: Map<HTMLCanvasElement, RecordingPen>;

beforeEach(() => {
  const made = pensPerCanvas();
  pens = made.pens;
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockImplementation(made.getContext);
});

/** The offscreen base's pen: every kept node and line, drawn once per base. */
function basePen(): RecordingPen {
  const { base } = visibleAndBase(pens);
  if (base === undefined) {
    throw new Error("no base drawn");
  }
  return base;
}

/** The visible canvas's pen: the frames composed from the base. */
function visiblePen(): RecordingPen {
  const { visible } = visibleAndBase(pens);
  if (visible === undefined) {
    throw new Error("no frame drawn");
  }
  return visible;
}

/** How many times a pen began a drawing: its surface fills. */
function drawings(pen: RecordingPen): number {
  return pen.calls.filter((call) => call.op === "fillRect").length;
}

function mount(view: MapView = VIEW) {
  const onView = vi.fn<(view: MapView) => void>();
  const onCommand = vi.fn<(command: MapCommand) => void>();
  const onOpen = vi.fn<(point: number, opener: HTMLElement) => void>();
  const onHover = vi.fn<(point: number | null) => void>();
  const shown = picture();
  const grid = gridOf(shown.positions, 3);
  const element = (shownView: MapView, lit: Lit | null) => (
    <MapCanvas
      picture={shown}
      kept={KEPT}
      lit={lit}
      focus={null}
      view={shownView}
      grid={grid}
      label="Map picture"
      describedBy="count"
      onView={onView}
      onCommand={onCommand}
      onHover={onHover}
      onOpen={onOpen}
      onBox={() => undefined}
    />
  );
  const result = render(element(view, null));
  const region = result.container.querySelector<HTMLElement>(".map-canvas");
  if (region === null) {
    throw new Error("no region");
  }
  const rerender = (shownView: MapView, lit: Lit | null) => {
    result.rerender(element(shownView, lit));
  };
  return { region, onView, onCommand, onOpen, onHover, rerender, unmount: result.unmount };
}

/** A node lit with its one neighbour and their line: A and B. */
const LIT_A: Lit = { points: Uint8Array.from([1, 1, 0]), lines: Uint8Array.from([1]), pointList: [0, 1], lineList: [0] };

describe("the canvas (CM7)", () => {
  it("is one tab stop, a labelled region described by the count line, the picture aria-hidden", () => {
    const { region } = mount();
    expect([region.getAttribute("role"), region.getAttribute("aria-label"), region.getAttribute("aria-describedby"), region.tabIndex]).toEqual(["region", "Map picture", "count", 0]);
    expect(region.querySelector("canvas")?.getAttribute("aria-hidden")).toBe("true");
  });

  it("fills each node with its slot's role as its hidden swatch resolves it, and reads the roles again when the theme changes", async () => {
    const { region } = mount();
    await waitFor(() => {
      expect(basePen().calls.some((call) => call.op === "fill")).toBe(true);
    });
    const pen = basePen();
    const resolved = (role: string) => {
      const swatch = region.querySelector<HTMLElement>(`[data-role="${role}"]`);
      return swatch === null ? "" : window.getComputedStyle(swatch).color || role;
    };
    const fillOf = (x: number) => pathsOf(pen.calls).find((path) => path.op === "fill" && path.arcs.some((arc) => arc.x === x))?.fillStyle;
    expect([fillOf(-100), fillOf(0), fillOf(100)]).toEqual([resolved("--map-kind-1"), resolved("--map-kind-2"), resolved("--map-kind-other")]);
    expect(new Set([fillOf(-100), fillOf(0), fillOf(100)]).size).toBe(3);
    const reads = vi.spyOn(window, "getComputedStyle");
    const before = drawings(pen);
    act(() => {
      document.documentElement.setAttribute("data-theme", "dark");
    });
    await waitFor(() => {
      expect(reads).toHaveBeenCalled();
    });
    // The roles read again, the base is drawn again in them.
    await waitFor(() => {
      expect(drawings(pen)).toBe(before + 1);
    });
    document.documentElement.removeAttribute("data-theme");
  });

  it("opens the node under a press that moved 4 px, and pans on one that moved 6 px, opening nothing (M: CLICK_DISTANCE 0)", () => {
    const { region, onOpen, onView } = mount();
    const at = screenOf(0, 0);
    fireEvent.pointerDown(region, { ...at, button: 0, pointerId: 1 });
    fireEvent.pointerMove(region, { clientX: at.clientX + 4, clientY: at.clientY, pointerId: 1 });
    fireEvent.pointerUp(region, { clientX: at.clientX + 4, clientY: at.clientY, pointerId: 1 });
    expect(onOpen.mock.calls.map(([point]) => point)).toEqual([1]);
    expect(onView).not.toHaveBeenCalled();
    onOpen.mockClear();
    fireEvent.pointerDown(region, { ...at, button: 0, pointerId: 1 });
    fireEvent.pointerMove(region, { clientX: at.clientX + 6, clientY: at.clientY, pointerId: 1 });
    fireEvent.pointerUp(region, { clientX: at.clientX + 6, clientY: at.clientY, pointerId: 1 });
    expect(onOpen).not.toHaveBeenCalled();
    expect(onView.mock.calls.at(-1)?.[0]).toEqual({ x: -6, y: 0, zoom: 1 });
  });

  it("opens nothing on a click off every node, and hovers the node under the pointer", () => {
    const { region, onOpen, onHover } = mount();
    const empty = screenOf(50, 50);
    fireEvent.pointerDown(region, { ...empty, button: 0, pointerId: 1 });
    fireEvent.pointerUp(region, { ...empty, button: 0, pointerId: 1 });
    expect(onOpen).not.toHaveBeenCalled();
    fireEvent.pointerMove(region, { ...screenOf(100, 3), pointerId: 1 });
    fireEvent.pointerMove(region, { ...empty, pointerId: 1 });
    expect(onHover.mock.calls.map(([point]) => point)).toEqual([2, null]);
  });

  it("never takes the wheel: a wheel on the region or the canvas is not prevented and moves nothing (M: the wheel zooming)", () => {
    const { region, onView } = mount();
    for (const target of [region, region.querySelector("canvas")]) {
      const wheel = new WheelEvent("wheel", { bubbles: true, cancelable: true, deltaY: 120 });
      target?.dispatchEvent(wheel);
      expect(wheel.defaultPrevented).toBe(false);
    }
    expect(onView).not.toHaveBeenCalled();
  });

  it("zooms by + = and -, fits by 0, pans 64 px by the arrows and clears by Esc, with the region focused and unmodified", () => {
    const view = { x: 10, y: 20, zoom: 2 };
    const { region, onView, onCommand } = mount(view);
    region.focus();
    for (const key of ["+", "=", "-", "ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"]) {
      fireEvent.keyDown(region, { key });
    }
    expect(onView.mock.calls.map(([next]) => next)).toEqual([
      { ...view, zoom: 2 * ZOOM_STEP },
      { ...view, zoom: 2 * ZOOM_STEP },
      { ...view, zoom: 2 / ZOOM_STEP },
      { ...view, x: 10 - PAN_STEP / 2 },
      { ...view, x: 10 + PAN_STEP / 2 },
      { ...view, y: 20 - PAN_STEP / 2 },
      { ...view, y: 20 + PAN_STEP / 2 },
    ]);
    fireEvent.keyDown(region, { key: "0" });
    fireEvent.keyDown(region, { key: "Escape" });
    expect(onCommand.mock.calls.map(([command]) => command)).toEqual(["fit", "clear"]);
    onView.mockClear();
    fireEvent.keyDown(region, { key: "+", ctrlKey: true });
    fireEvent.keyDown(region, { key: "-", metaKey: true });
    // A key whose target is not the region itself (a child, never focused) does nothing.
    fireEvent.keyDown(region.querySelector("canvas") ?? region, { key: "+" });
    expect(onView).not.toHaveBeenCalled();
  });

  it("keeps the zoom between 0.2 and 4", () => {
    const { region, onView } = mount({ x: 0, y: 0, zoom: 3.9 });
    fireEvent.keyDown(region, { key: "+" });
    expect(onView.mock.calls[0]?.[0].zoom).toBe(4);
  });
});

// docs/features/corpus-map.md "Acceptance criteria", CM12: the base is drawn once into the offscreen
// canvas; a hover composes it, dimmed, with the lit nodes and their line alone, never drawing the
// base again; a view change composes the moved base at once and draws the base again only once the
// view has held, however many steps a held key or a drag makes. M: a hover redrawing everything.
describe("the cached base (CM12)", () => {
  it("draws a hover as the base image dimmed and the lit subset on top; the base is not drawn again (M: a hover redrawing everything)", async () => {
    const { rerender } = mount();
    await waitFor(() => {
      expect(drawings(basePen())).toBe(1);
    });
    expect(basePen().calls.filter((call) => call.op === "arc")).toHaveLength(3);
    const frames = visiblePen().calls.length;
    rerender(VIEW, LIT_A);
    await waitFor(() => {
      expect(visiblePen().calls.length).toBeGreaterThan(frames);
    });
    const frame = visiblePen().calls.slice(frames);
    expect(frame.filter((call) => call.op === "drawImage").map((call) => call.globalAlpha)).toEqual([MAP_DIM_ALPHA]);
    expect(frame.filter((call) => call.op === "arc").map((call) => call.args[0])).toEqual([-100, 0]);
    expect(frame.filter((call) => call.op === "lineTo")).toHaveLength(1);
    // The base holds: drawn once, whatever is lit.
    rerender(VIEW, null);
    await waitFor(() => {
      expect(visiblePen().calls.filter((call) => call.op === "drawImage").at(-1)?.globalAlpha).toBe(1);
    });
    expect(drawings(basePen())).toBe(1);
  });

  it("composes the moved base on each view step and draws the base again once, after the view has held", async () => {
    const { rerender } = mount();
    await waitFor(() => {
      expect(drawings(basePen())).toBe(1);
    });
    for (let step = 1; step <= 5; step += 1) {
      rerender({ x: step * 10, y: 0, zoom: 1 }, null);
      await act(async () => {
        await new Promise((resolve) => setTimeout(resolve, 20));
      });
    }
    // Five frames, each the base image moved by its step; no new base yet.
    const moved = visiblePen().calls.filter((call) => call.op === "drawImage").map((call) => call.args[0]);
    expect(moved.slice(-5)).toEqual([-10, -20, -30, -40, -50]);
    expect(drawings(basePen())).toBe(1);
    await waitFor(
      () => {
        expect(drawings(basePen())).toBe(2);
      },
      { timeout: BASE_SETTLE_MS * 10 },
    );
    // Drawn for the view it settled on: the image lands where it is drawn.
    expect(visiblePen().calls.filter((call) => call.op === "drawImage").at(-1)?.args[0]).toBe(0);
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, BASE_SETTLE_MS * 2));
    });
    expect(drawings(basePen())).toBe(2);
  });
});

describe("the offscreen base on unmount", () => {
  it("gives the base's pixels back at once: its size zero, never left to the collector (M: the base kept)", async () => {
    const { unmount } = mount();
    await waitFor(() => {
      expect(drawings(basePen())).toBeGreaterThan(0);
    });
    const layer = [...pens.keys()].find((canvas) => !canvas.isConnected);
    expect([layer?.width, layer?.height]).toEqual([UNMEASURED.width, UNMEASURED.height]);
    unmount();
    expect([layer?.width, layer?.height]).toEqual([0, 0]);
  });
});
