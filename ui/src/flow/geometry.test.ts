import { describe, expect, it } from "vitest";
import { curveMiddle, curvePath, curvePoints, durationOf, fitOf, fittedCanvasHeight, inView, loopPath, LOOP_DEPTH, pointInside, roundedPath, stretchCrosses } from "./geometry";

// The Flow's drawing geometry (docs/features/flow-canvas.md "Layout"): pure numbers.

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

  it("starts a picture too wide to read whole at its top left, at the smallest readable zoom", () => {
    const fit = fitOf({ width: 5000, height: 3000 }, 1000, 800);
    expect(fit?.zoom).toBe(0.6);
    expect([fit?.x, fit?.y]).toEqual([32, 32]);
  });

  it("gives no fit on a canvas with no size", () => {
    expect(fitOf({ width: 800, height: 400 }, 0, 0)).toBeNull();
  });

  it("lets the width decide: a flow fills it whatever the canvas's height, and a tall one starts at its top, readable", () => {
    const wide = fitOf({ width: 1000, height: 300 }, 832, 800);
    expect(wide?.zoom).toBe(0.768);
    expect(wide?.x).toBe(32);
    expect(fitOf({ width: 1000, height: 300 }, 832, 300)?.zoom).toBe(0.768);
    const tall = fitOf({ width: 800, height: 3000 }, 1000, 500);
    expect(tall?.zoom).toBe(1);
    expect([tall?.x, tall?.y]).toEqual([100, 32]);
  });

  it("gives a short flow a short canvas: the fitted picture, its margins and the controls under it", () => {
    expect(fittedCanvasHeight({ width: 1000, height: 300 }, 832)).toBe(Math.ceil(300 * 0.768 + 64 + 44));
    expect(fittedCanvasHeight({ width: 400, height: 92 }, 832)).toBe(92 + 64 + 44);
    expect(fittedCanvasHeight({ width: 5000, height: 300 }, 832)).toBe(180 + 64 + 44);
    expect(fittedCanvasHeight({ width: 1000, height: 300 }, 0)).toBeNull();
  });

  it("holds the height it gives: the picture fits and stays clear of the controls", () => {
    const picture = { width: 1000, height: 300 };
    const height = fittedCanvasHeight(picture, 832) ?? 0;
    const fit = fitOf(picture, 832, height);
    expect(fit?.y).toBeGreaterThanOrEqual(32);
    expect(height - (fit?.y ?? 0) - picture.height * (fit?.zoom ?? 0)).toBeGreaterThanOrEqual(44);
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
