import type { Proposal, ShownNode } from "../api/types";

/** A whole proposal for tests; omitted keys null or empty as the review JSON gives them. */
export function aProposal(fields: Partial<Proposal> & Pick<Proposal, "id">): Proposal {
  return {
    project: "alpha",
    kind: "update",
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
    created_at: "2026-10-01T10:00:00Z",
    updated_at: "2026-10-01T10:00:00Z",
    notes: [],
    severity: "normal",
    gap_type: null,
    task_id: null,
    target_ids: [],
    evidence: [],
    options: [],
    recommendation: null,
    working_answer: null,
    summary: `Summary of ${fields.id}`,
    ...fields,
  };
}

/** A whole shown node for tests. */
export function aNode(fields: Partial<ShownNode> & Pick<ShownNode, "id">): ShownNode {
  return {
    kind: "widget",
    title: `Title of ${fields.id ?? "the node"}`,
    path: "docs/spec/a.md",
    line: 1,
    end_line: 3,
    status: null,
    rev: 1,
    tokens_est: 10,
    archived: false,
    utf8: true,
    sections: [],
    text: `## ${fields.id ?? "Node"}\n\nCurrent text.`,
    truncated: false,
    omitted: null,
    links: null,
    ...fields,
  };
}
