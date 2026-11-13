import { describe, expect, it, vi } from "vitest";
import type { SpecEngineClient } from "../api/client";
import { sceneOf } from "./filters";
import { layoutAll, MAP_ALPHA_DECAY, MAP_ALPHA_MIN, MAP_TICKS, runSlice, startLayout } from "./layout";

// docs/features/corpus-map.md "Acceptance criteria", CM6: the mock's answer laid out twice gives
// identical positions, no NaN, exactly MAP_TICKS ticks (spied); slices of one tick end where one
// slice does; with Sections off the sections are not laid out. d3's own seeded random source, never
// `Math.random`; a slice is cut by time, the layout never. M: `Math.random` as the random source;
// ticks counted by time.

/** The mock client, loaded as data through Vite: only the bootstrap imports the mocks (policy.test.ts). */
const mock = import.meta.glob<{ MockClient: new (scenario: string, options: { delayMs: number }) => SpecEngineClient }>("/src/mocks/MockClient.ts", { eager: true });
const MockClient = mock["/src/mocks/MockClient.ts"]?.MockClient;
if (MockClient === undefined) {
  throw new Error("no mock client");
}

/** harbor-sim's map as the mock serves it. */
const answer = await new MockClient("normal", { delayMs: 0 }).getMap("harbor-sim", { archive: false });

function linksOf(sections: boolean) {
  const scene = sceneOf(answer, sections);
  return { scene, links: Array.from(scene.lineFrom, (from, at) => ({ source: from, target: scene.lineTo[at] ?? from })) };
}

describe("the layout (CM6)", () => {
  it("gives the same positions twice, none NaN, alpha at its floor after the last tick", () => {
    const { scene, links } = linksOf(false);
    const first = layoutAll(scene.points.length, links);
    const second = layoutAll(scene.points.length, links);
    expect(Array.from(first)).toEqual(Array.from(second));
    expect(Array.from(first).filter((value) => !Number.isFinite(value))).toEqual([]);
    expect(first).toHaveLength(scene.points.length * 2);
    // Alpha reaches alphaMin at tick MAP_TICKS: (1 - decay)^ticks.
    expect((1 - MAP_ALPHA_DECAY) ** MAP_TICKS).toBeCloseTo(MAP_ALPHA_MIN, 10);
    const layout = startLayout(scene.points.length, links);
    layout.step(MAP_TICKS);
    expect(layout.simulation.alpha()).toBeCloseTo(MAP_ALPHA_MIN, 10);
  });

  it("draws from d3's seeded source, never Math.random (M: Math.random as the random source)", () => {
    const { scene, links } = linksOf(false);
    const random = vi.spyOn(Math, "random");
    const layout = startLayout(scene.points.length, links);
    // d3's fixed-seed LCG: its first draw is (1664525 + 1013904223) / 2^32 on every engine.
    expect(layout.simulation.randomSource()()).toBe(1015568748 / 4294967296);
    expect(layout.simulation.randomSource()).not.toBe(Math.random);
    layout.step(MAP_TICKS);
    expect(random).not.toHaveBeenCalled();
  });

  it("runs exactly MAP_TICKS ticks whatever the clock, each slice cut by time (M: ticks counted by time)", () => {
    const { scene, links } = linksOf(false);
    for (const millisPerRead of [0.5, 7, 40]) {
      const layout = startLayout(scene.points.length, links);
      const tick = vi.spyOn(layout.simulation, "tick");
      let clock = 0;
      const now = () => {
        clock += millisPerRead;
        return clock;
      };
      let slices = 0;
      while (!layout.done) {
        runSlice(layout, now);
        slices += 1;
      }
      expect([millisPerRead, tick.mock.calls.length, layout.ticks]).toEqual([millisPerRead, MAP_TICKS, MAP_TICKS]);
      // A slow clock cuts a slice per tick, a fast one fewer slices: never another tick count.
      expect([millisPerRead, millisPerRead >= 16 ? slices === MAP_TICKS : slices < MAP_TICKS]).toEqual([millisPerRead, true]);
      layout.step(5);
      expect(tick.mock.calls.length).toBe(MAP_TICKS);
    }
  });

  it("ends slices of one tick where one uninterrupted run ends", () => {
    const { scene, links } = linksOf(false);
    const sliced = startLayout(scene.points.length, links);
    while (!sliced.done) {
      runSlice(sliced, () => 0, 0);
    }
    expect(Array.from(sliced.positions())).toEqual(Array.from(layoutAll(scene.points.length, links)));
  });

  it("lays out only the documents with Sections off, the sections too with it on", () => {
    const off = sceneOf(answer, false);
    const on = sceneOf(answer, true);
    const sections = answer.nodes.flatMap((node, index) => (node.of === null ? [] : [index]));
    expect(sections).toHaveLength(4);
    expect(sections.map((index) => off.pointOf[index])).toEqual([-1, -1, -1, -1]);
    expect(sections.every((index) => (on.pointOf[index] ?? -1) >= 0)).toBe(true);
    expect(on.points.length - off.points.length).toBe(4);
  });
});
