import { forceCenter, forceLink, forceManyBody, forceSimulation, forceX, forceY, type Simulation, type SimulationLinkDatum, type SimulationNodeDatum } from "d3-force";

// The map's layout (docs/features/corpus-map.md "Acceptance criteria", CM6; ADR-0080): d3-force,
// pure, no drawing. Nodes go in in the scene's order, so one answer gives one picture: d3 places
// them on its phyllotaxis spiral and draws from its fixed-seed random source, never the platform's.
// Exactly MAP_TICKS ticks run, alpha reaching `alphaMin` at the last; the screen runs them in slices
// of at most MAP_SLICE_MS between frames, and any slicing ends in the positions of one
// uninterrupted run. The timer d3 starts is stopped at once: nothing here runs by itself.

/** The ticks of every layout, whatever the size or the time they take. */
export const MAP_TICKS = 200;

/** Where alpha ends, d3's default, reached at tick MAP_TICKS. */
export const MAP_ALPHA_MIN = 0.001;

/** The decay that brings alpha from 1 to MAP_ALPHA_MIN in MAP_TICKS ticks. */
export const MAP_ALPHA_DECAY = 1 - MAP_ALPHA_MIN ** (1 / MAP_TICKS);

/** A slice of ticks runs at most this long before the frame it leaves to paint. */
export const MAP_SLICE_MS = 16;

/** A link's length at rest, and how far a node's charge reaches. */
export const LINK_DISTANCE = 40;
export const CHARGE_REACH = 400;

/** The weak pull to the middle that keeps an orphan near the edge rather than drifting off. */
export const CENTRE_PULL = 0.05;

type LayoutNode = SimulationNodeDatum;
type LayoutLink = SimulationLinkDatum<LayoutNode>;

/** A layout under way: its ticks so far, and the positions they reached. */
export interface MapLayout {
  readonly count: number;
  /** The ticks run so far, at most MAP_TICKS. */
  readonly ticks: number;
  readonly done: boolean;
  /** Runs up to `ticks` more, never past MAP_TICKS. */
  step: (ticks: number) => void;
  /** Each point's x then y, in the scene's order. */
  positions: () => Float64Array;
  /** The engine, for a test to spy on. */
  readonly simulation: Simulation<LayoutNode, LayoutLink>;
}

/** Starts a layout of `count` points joined by `links` (point indexes); no tick has run. */
export function startLayout(count: number, links: readonly { source: number; target: number }[]): MapLayout {
  const nodes: LayoutNode[] = Array.from({ length: count }, () => ({}));
  const simulation = forceSimulation<LayoutNode, LayoutLink>(nodes)
    .stop()
    .alphaMin(MAP_ALPHA_MIN)
    .alphaDecay(MAP_ALPHA_DECAY)
    .force(
      "link",
      forceLink<LayoutNode, LayoutLink>(links.map((link) => ({ source: link.source, target: link.target }))).distance(LINK_DISTANCE),
    )
    .force("charge", forceManyBody<LayoutNode>().distanceMax(CHARGE_REACH))
    .force("centre", forceCenter<LayoutNode>(0, 0))
    .force("x", forceX<LayoutNode>(0).strength(CENTRE_PULL))
    .force("y", forceY<LayoutNode>(0).strength(CENTRE_PULL));
  let ticks = 0;
  return {
    count,
    get ticks() {
      return ticks;
    },
    get done() {
      return ticks >= MAP_TICKS;
    },
    step(more: number) {
      const run = Math.max(0, Math.min(more, MAP_TICKS - ticks));
      for (let at = 0; at < run; at += 1) {
        simulation.tick();
        ticks += 1;
      }
    },
    positions() {
      const out = new Float64Array(count * 2);
      nodes.forEach((node, at) => {
        out[at * 2] = node.x ?? 0;
        out[at * 2 + 1] = node.y ?? 0;
      });
      return out;
    },
    simulation,
  };
}

/**
 * One slice: ticks one at a time until MAP_SLICE_MS has passed by `now` or the layout is done; at
 * least one. The time only cuts a slice; the layout ends at MAP_TICKS ticks, never at a time.
 */
export function runSlice(layout: MapLayout, now: () => number, budget: number = MAP_SLICE_MS): number {
  const start = now();
  const before = layout.ticks;
  do {
    layout.step(1);
  } while (!layout.done && now() - start < budget);
  return layout.ticks - before;
}

/** A whole layout in one run: what any slicing ends in. */
export function layoutAll(count: number, links: readonly { source: number; target: number }[]): Float64Array {
  const layout = startLayout(count, links);
  layout.step(MAP_TICKS);
  return layout.positions();
}
