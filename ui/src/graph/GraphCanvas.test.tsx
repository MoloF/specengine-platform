import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { graphClient } from "../test/graphStub";
import { mouse, pointer, pressAndMove, stubCanvas, stubFocusVisible, viewOf } from "../test/press";
import { renderApp } from "../test/render";
import { BOX_WIDTH } from "./layout";

// docs/features/canvas-clicks.md "Acceptance criteria", CK1 and CK5: on Graph, a press that moves
// a few px is a click, and a press that focuses a box moves nothing; a focus from the keyboard still
// brings a box out of view into it. jsdom's presses carry their view (`test/press.ts`), so d3-zoom
// pans past the click distance as it does in a browser.

function boxes(): HTMLElement[] {
  return Array.from(document.querySelectorAll<HTMLElement>(".react-flow__node"));
}

function box(id: string): HTMLElement {
  const found = boxes().find((element) => element.dataset.id === id);
  if (found === undefined) {
    throw new Error(`no box ${id}`);
  }
  return found;
}

async function drawn(): Promise<void> {
  await waitFor(() => {
    expect(boxes().length).toBeGreaterThan(0);
  });
}

/** The details' heading: the selected box's name. */
function detailsHeading(): string {
  return within(screen.getByRole("complementary")).getByRole("heading", { level: 2 }).textContent;
}

/** The canvas's pan and zoom as numbers. */
function viewport(): { x: number; y: number; zoom: number } {
  const match = /translate\(\s*(-?[\d.]+)px,\s*(-?[\d.]+)px\)\s*scale\(([\d.]+)\)/.exec(viewOf());
  return { x: Number(match?.[1]), y: Number(match?.[2]), zoom: Number(match?.[3]) };
}

/** A box's left edge in the flow, from where React Flow placed it. */
function leftOf(element: HTMLElement): number {
  return Number(/translate\(\s*(-?[\d.]+)px/.exec(element.style.transform)?.[1]);
}

describe("a slight move is a click on Graph (CK1)", () => {
  it("takes a press that moves 3 px for a click, box after box; one that moves 8 px pans and selects nothing", async () => {
    renderApp(graphClient(), "#/alpha/graph/R-0");
    await drawn();
    // M: `paneClickDistance` removed from GraphCanvas; at React Flow's 1 px the 3 px press selects nothing.
    pressAndMove(box("n:A1"), 3, 1);
    await waitFor(() => {
      expect(detailsHeading()).toMatch(/^A1/);
    });
    expect(within(box("n:A1")).getByText("Selected")).toBeTruthy();
    // The selection lives in the view, as before: the hash keeps the REF.
    expect(window.location.hash).toBe("#/alpha/graph/R-0");
    pressAndMove(box("n:B1"), 3, 1);
    await waitFor(() => {
      expect(detailsHeading()).toMatch(/^B1/);
    });
    expect(within(box("n:B1")).getByText("Selected")).toBeTruthy();
    expect(within(box("n:A1")).queryByText("Selected")).toBeNull();
    const still = viewOf();
    pressAndMove(box("n:R-0"), 8, 0);
    expect(viewOf()).not.toBe(still);
    expect(detailsHeading()).toMatch(/^B1/);
    expect(within(box("n:R-0")).queryByText("Selected")).toBeNull();
    expect(window.location.hash).toBe("#/alpha/graph/R-0");
  });
});

describe("a press moves nothing on Graph (CK5)", () => {
  it("leaves the view where it was when a pointer press focuses a box half out of view, the box selected and roving; a keyboard focus out of view centres it", async () => {
    const restore = stubCanvas(500, 400);
    const focus = stubFocusVisible();
    try {
      renderApp(graphClient(), "#/alpha/graph/R-0");
      await drawn();
      // The fit: zoom 0.6, the focus at the margin; the third column stands across the right edge.
      await waitFor(() => {
        expect(viewport().zoom).toBe(0.6);
      });
      const view = viewOf();
      const target = box("n:B1");
      const { x, zoom } = viewport();
      const left = leftOf(target) * zoom + x;
      expect([left < 500, left + BOX_WIDTH * zoom > 500]).toEqual([true, true]);
      // A mouse press: pointerdown, the focus it brings, pointerup, click. M: centring on every focus.
      pointer(target, "pointerdown");
      act(() => {
        target.focus();
      });
      pointer(target, "pointerup");
      mouse(target, "click", 100, 100);
      await waitFor(() => {
        expect(detailsHeading()).toMatch(/^B1/);
      });
      expect(viewOf()).toBe(view);
      expect(box("n:B1").getAttribute("tabindex")).toBe("0");
      expect(boxes().filter((element) => element.getAttribute("tabindex") === "0")).toHaveLength(1);
      // A touch tap on another box half out of view focuses after its pointerup: still nothing moves.
      pointer(box("n:B2"), "pointerup", "touch");
      act(() => {
        box("n:B2").focus();
      });
      expect(viewOf()).toBe(view);
      expect(box("n:B2").getAttribute("tabindex")).toBe("0");
      // Tab to A1, in view: nothing moves. Right brings focus to B1, out of view: centred.
      focus.keyboard = true;
      act(() => {
        box("n:A1").focus();
      });
      expect(viewOf()).toBe(view);
      fireEvent.keyDown(box("n:A1"), { key: "ArrowRight" });
      expect(document.activeElement).toBe(box("n:B1"));
      await waitFor(() => {
        expect(viewOf()).not.toBe(view);
      });
      expect(viewport().zoom).toBe(0.6);
    } finally {
      focus.restore();
      restore();
    }
  });
});
