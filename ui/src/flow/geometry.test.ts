import { describe, expect, it } from "vitest";
import { FIT_MARGIN } from "../graph/geometry";
import { curveMiddle, curvePath, curvePoints, durationOf, EDGE_HIT, fitOf, FLOW_CLICK_DISTANCE, FLOW_FIT_MIN_ZOOM, inView, loopPath, LOOP_DEPTH, pointInside, roundedPath, stretchCrosses } from "./geometry";

// The Flow's drawing geometry (docs/features/flow-canvas.md "Layout"; docs/features/handoff-progress.md
// "Acceptance criteria", F-4 and C-1): pure numbers.

describe("a forward edge's path", () => {
  it("is straight when its runs share a height, an S-curve when they do not", () => {
    expect(
      curvePath([
        { from: 0, to: 10, y: 5 },
        { from: 50, to: 50, y: 5 },
      ]),
    ).toBe("M0,5 L10,5 L50,5");
    expect(
      curvePath([
        { from: 0, to: 10, y: 5 },
        { from: 50, to: 80, y: 25 },
      ]),
    ).toBe("M0,5 L10,5 C30,5 30,25 50,25 L80,25");
  });

  it("keeps its runs' ends, and its label on the first curve", () => {
    const runs = [
      { from: 0, to: 10, y: 0 },
      { from: 110, to: 200, y: 40 },
      { from: 300, to: 300, y: 40 },
    ];
    expect(curvePoints(runs).map((point) => point.x)).toEqual([0, 10, 110, 200, 300, 300]);
    expect(curveMiddle(runs)).toEqual({ x: 60, y: 20 });
  });
});

describe("an arc's path", () => {
  it("rounds each corner, less on a short stretch, and ends at its last point", () => {
    expect(
      roundedPath([
        { x: 0, y: 0 },
        { x: 100, y: 0 },
        { x: 100, y: 100 },
      ]),
    ).toBe("M0,0 L92,0 Q100,0 100,8 L100,100");
    expect(
      roundedPath([
        { x: 0, y: 0 },
        { x: 6, y: 0 },
        { x: 6, y: 100 },
      ]),
    ).toBe("M0,0 L3,0 Q6,0 6,3 L6,100");
  });

  it("skips a corner with no length on one side", () => {
    expect(
      roundedPath([
        { x: 0, y: 0 },
        { x: 0, y: 0 },
        { x: 0, y: 50 },
      ]),
    ).toBe("M0,0 L0,50");
  });
});

describe("a box's edge to itself", () => {
  it("leaves and enters its underside, and runs under it", () => {
    const box = { x: 100, y: 10, width: 200, height: 70 };
    const loop = loopPath(box);
    expect(loop.points.every((point) => point.y === 80)).toBe(true);
    expect(loop.points.every((point) => point.x > box.x && point.x < box.x + box.width)).toBe(true);
    expect(loop.middle).toEqual({ x: 200, y: 80 + LOOP_DEPTH });
    expect(pointInside(loop.middle, box)).toBe(false);
  });
});

describe("whether a line touches a box", () => {
  const box = { x: 10, y: 10, width: 20, height: 20 };

  it("calls a port on the border outside, the middle inside", () => {
    expect(pointInside({ x: 10, y: 20 }, box)).toBe(false);
    expect(pointInside({ x: 20, y: 20 }, box)).toBe(true);
  });

  it("sees a stretch through the box, not one along its side or beside it", () => {
    expect(stretchCrosses({ x: 0, y: 20 }, { x: 40, y: 20 }, box)).toBe(true);
    expect(stretchCrosses({ x: 20, y: 0 }, { x: 20, y: 15 }, box)).toBe(true);
    expect(stretchCrosses({ x: 0, y: 10 }, { x: 40, y: 10 }, box)).toBe(false);
    expect(stretchCrosses({ x: 40, y: 0 }, { x: 40, y: 40 }, box)).toBe(false);
  });
});

describe("the view", () => {
  it("fits a picture that stays readable whole, centred", () => {
    const fit = fitOf({ width: 800, height: 400 }, 1000, 800);
    expect(fit?.zoom).toBe(1);
    expect(fit?.x).toBe(100);
    expect(fit?.y).toBe(200);
  });

  it("starts a picture too big to read whole at its top left, at Flow's own floor, 0.92 (FS-14)", () => {
    expect(FLOW_FIT_MIN_ZOOM).toBe(0.92);
    const fit = fitOf({ width: 5000, height: 3000 }, 1000, 800);
    expect(fit?.zoom).toBe(0.92);
    expect([fit?.x, fit?.y]).toEqual([32, 32]);
    expect(fitOf({ width: 4000, height: 300 }, 832, 600)?.zoom).toBe(0.92);
  });

  it("gives no fit on a canvas with no size", () => {
    expect(fitOf({ width: 800, height: 400 }, 0, 0)).toBeNull();
  });

  // F-4: on a 1100 x 700 canvas, the lesser of the two sides' ratios, within 0.92 and 1.
  describe("the fit by both sides (F-4)", () => {
    it("fits a 900 x 400 picture at 1, centred both ways", () => {
      expect(fitOf({ width: 900, height: 400 }, 1100, 700)).toEqual({ x: 100, y: 150, zoom: 1 });
    });

    it("fits a 1100 x 400 picture at (1100 - 2 margins) / 1100, centred", () => {
      const fit = fitOf({ width: 1100, height: 400 }, 1100, 700);
      const zoom = (1100 - 2 * FIT_MARGIN) / 1100;
      expect(fit?.zoom).toBeCloseTo(zoom, 10);
      expect(fit?.x).toBeCloseTo(FIT_MARGIN, 10);
      expect(fit?.y).toBeCloseTo((700 - 400 * zoom) / 2, 10);
    });

    it("lands a 3000 x 400 picture at 0.92, x at the margin, y centred: a long walk from its first column, panned", () => {
      expect(fitOf({ width: 3000, height: 400 }, 1100, 700)).toEqual({ x: FIT_MARGIN, y: (700 - 400 * 0.92) / 2, zoom: 0.92 });
    });

    it("lands an 800 x 2000 picture at 0.92, y at the margin: the height has its say, never below the floor", () => {
      const fit = fitOf({ width: 800, height: 2000 }, 1100, 700);
      expect(fit?.zoom).toBe(0.92);
      expect(fit?.y).toBe(FIT_MARGIN);
      expect(fit?.x).toBe((1100 - 800 * 0.92) / 2);
    });

    it("gives no fit on a canvas with no size", () => {
      expect(fitOf({ width: 900, height: 400 }, 0, 0)).toBeNull();
    });
  });

  it("takes a press that moves 5 px or less for a click (C-1)", () => {
    expect(FLOW_CLICK_DISTANCE).toBe(5);
  });

  it("gives an arrow a 12 px band to click (writer-studio D8)", () => {
    expect(EDGE_HIT).toBe(12);
  });

  it("tells a box in view from one past its edge, at the zoom", () => {
    const view = { x: 0, y: 0, zoom: 0.5, width: 300, height: 300 };
    expect(inView({ x: 100, y: 100, width: 240, height: 72 }, view)).toBe(true);
    expect(inView({ x: 500, y: 100, width: 240, height: 72 }, view)).toBe(false);
  });
});

describe("motion (AC-13)", () => {
  it("takes a viewport move's time from its token, 0 when reduced motion zeroes it or no sheet applies", () => {
    const root = document.documentElement;
    root.style.setProperty("--duration-normal", "200ms");
    expect(durationOf("--duration-normal")).toBe(200);
    root.style.setProperty("--duration-normal", "0.4s");
    expect(durationOf("--duration-normal")).toBe(400);
    root.style.setProperty("--duration-normal", "0ms");
    expect(durationOf("--duration-normal")).toBe(0);
    root.style.removeProperty("--duration-normal");
    expect(durationOf("--duration-normal")).toBe(0);
  });
});
