import type { FlowView, TraceCounts } from "../api/types";
import { NO_STAGE_ASKED } from "./labels";

// The lanes as the places pane lists them (docs/features/writer-studio.md "Description and interactions"), from what
// the daemon served: nothing here counts a rollup or joins a card. Lanes keep the served order; a
// stage is asked as served, and compared to nothing.

/** A lane's key for what has no stage: it has no `stage:`, so no served stage can equal it. */
const NO_STAGE = "none";

/** One served stage's key: prefixed, so no project's word can equal the two above. */
export function stageKey(stage: string | null): string {
  return stage === null ? NO_STAGE : `stage:${stage}`;
}

/** What a lane's read asks: its stage as served, `-` for none. */
export function stageAsked(stage: string | null): string {
  return stage ?? NO_STAGE_ASKED;
}

/** A read whose re-read failed, its boxes saying no state and no gap: what it last said may no longer hold. */
export function withoutStates(view: FlowView): FlowView {
  return { ...view, nodes: view.nodes.map((node) => ({ ...node, rollup: null, counts: null, gaps: 0 })) };
}

/** Every step of a lane: the five served parts. */
export function laneTotal(counts: TraceCounts): number {
  return counts.no_card + counts.in_work + counts.suspect + counts.departed + counts.confirmed;
}
