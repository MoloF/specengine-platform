import type { Project, Proposal, ShownNode } from "../api/types";

/** One invented project as the mock holds it: its nodes and its queue. */
export interface MockProject {
  project: Project;
  /** The project's node kinds (`src/mocks/<slug>/kinds.ts`). */
  nodeKinds: readonly string[];
  nodes: ShownNode[];
  proposals: Proposal[];
  notes: string[];
}

const MINUTE = 60_000;

/** UTC in the stored form, `2026-10-05T21:14:03Z`, `minutesAgo` before `now`. */
export function stamp(now: number, minutesAgo: number): string {
  return new Date(now - minutesAgo * MINUTE).toISOString().replace(/\.\d{3}Z$/, "Z");
}

/** A whole node as `spec show --json` prints it; omitted keys take show's defaults. */
export function node(fields: Pick<ShownNode, "id" | "kind" | "title" | "path" | "line" | "text"> & Partial<ShownNode>): ShownNode {
  const lines = fields.text.split("\n").length;
  return {
    status: null,
    rev: 1,
    end_line: fields.line + lines - 1,
    tokens_est: Math.ceil(fields.text.length / 4),
    archived: false,
    utf8: true,
    sections: [],
    truncated: false,
    omitted: null,
    links: null,
    ...fields,
  };
}

/** A whole queue card; omitted keys are null or empty, as the review JSON gives them. */
export function proposal(
  fields: Pick<Proposal, "id" | "project" | "kind" | "created_at"> & Partial<Proposal>,
): Proposal {
  return {
    status: "open",
    target_id: null,
    target_path: null,
    worktree: null,
    branch: null,
    base_commit: null,
    base_hash: null,
    base_text: null,
    new_text: null,
    patch_hash: null,
    rationale: null,
    author: null,
    diagnostics: [],
    diff: null,
    preview: null,
    conflict: null,
    decided_by: null,
    decided_at: null,
    decision_note: null,
    applied_commit: null,
    updated_at: fields.created_at,
    notes: [],
    severity: null,
    gap_type: null,
    task_id: null,
    target_ids: [],
    evidence: [],
    options: [],
    recommendation: null,
    working_answer: null,
    summary: null,
    ...fields,
  };
}
