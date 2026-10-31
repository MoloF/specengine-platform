import type { FlowNode, FlowView, TraceCounts, TraceNode, TraceStage } from "../api/types";
import { compareText } from "../graph/layout";
import { stageWords } from "./labels";

// The Timeline and the stage filter's arrangement of what the daemon served
// (docs/features/trace-views.md "Timeline", "Stage filter"): nothing here counts a rollup or joins a
// card. Lanes keep the served order; a stage is compared to a served value and to nothing else.

/** The filter showing everything. */
export const ALL_STAGES = "all";

/** The filter's and a lane's key for what has no stage: it has no `stage:`, so no served stage can equal it. */
const NO_STAGE = "none";

/** What `spec trace --stage` takes for the documents with none. */
const NO_STAGE_ASKED = "-";

/** One served stage's key: prefixed, so no project's word can equal the two above. */
export function stageKey(stage: string | null): string {
  return stage === null ? NO_STAGE : `stage:${stage}`;
}

/** What a lane's read asks: its stage as served, `-` for none. */
export function stageAsked(stage: string | null): string {
  return stage ?? NO_STAGE_ASKED;
}

/** One entry of the stage filter. */
export interface StageOption {
  value: string;
  label: string;
}

/** All, then each lane served, in the served order, its "No stage" with it. */
export function stageOptions(stages: readonly TraceStage[]): StageOption[] {
  return [{ value: ALL_STAGES, label: "All stages" }, ...stages.map(({ stage }) => ({ value: stageKey(stage), label: stageWords(stage) }))];
}

/** The filter in force: a stage no lane serves any more shows everything, and so does a summary not read yet. */
export function stageInForce(chosen: string, stages: readonly TraceStage[]): string {
  return stageOptions(stages).some((option) => option.value === chosen) ? chosen : ALL_STAGES;
}

/** Whether a box stands in the filter: it compares nothing but the served value. */
export function inStage(stage: string | null, filter: string): boolean {
  return filter === ALL_STAGES || stageKey(stage) === filter;
}

/** Whether a box is outside the filter, by its own stage. A group with none is not judged: what it holds may stand in any stage. */
export function outsideStage({ stage, holds }: Pick<FlowNode, "stage" | "holds">, filter: string): boolean {
  return !inStage(stage, filter) && (stage !== null || holds === 0);
}

/** Whether the List keeps a box under the filter: a group always, with its Open, the way to what it holds. */
export function keptInList({ stage, holds }: Pick<FlowNode, "stage" | "holds">, filter: string): boolean {
  return holds > 0 || inStage(stage, filter);
}

/** A read whose re-read failed, its boxes saying no state and no gap: what it last said may no longer hold. */
export function withoutStates(view: FlowView): FlowView {
  return { ...view, nodes: view.nodes.map((node) => ({ ...node, rollup: null, counts: null, gaps: 0 })) };
}

/** Every step of a lane: the five served parts. */
export function laneTotal(counts: TraceCounts): number {
  return counts.no_card + counts.in_work + counts.suspect + counts.departed + counts.confirmed;
}

/** Whether no step has a card: every part but `no_card` is empty. */
export function noCardAnywhere(counts: TraceCounts): boolean {
  return counts.in_work + counts.suspect + counts.departed + counts.confirmed === 0;
}

/** Whether no document has a `stage:`: no lane, or only the lane of none. */
export function noStageAnywhere(stages: readonly TraceStage[]): boolean {
  return stages.every((lane) => lane.stage === null);
}

/** A lane's steps by `distance`, those off the walk last, then by name: the same whatever order they came in. */
export function stepsInOrder(nodes: readonly TraceNode[]): TraceNode[] {
  return [...nodes].sort((a, b) => (a.distance ?? Infinity) - (b.distance ?? Infinity) || compareText(a.id, b.id));
}
