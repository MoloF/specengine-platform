import type { Finding, InboxEntry, LinkOrigin, LinkState, Project, Proposal, ProposalStatus, TreeMark } from "../api/types";

/**
 * One node of a mock corpus: a document or a nested ID section of one, with its span in the
 * file (1-based lines, inclusive). The fields `spec show` and `spec tree` print come from here.
 */
export interface MockNode {
  id: string | null;
  kind: string | null;
  title: string | null;
  path: string;
  line: number;
  endLine: number;
  rev: number | null;
  /** A document's front-matter status; null for a section. */
  status: string | null;
  /** A document's front-matter summary (the bundle's `summary` form); null for a section. */
  summary: string | null;
  /** Tier 3: left out of the tree, search and incoming links unless `archive`. */
  archived: boolean;
  /** `class: generated`: never in the tree. */
  generated: boolean;
  utf8: boolean;
  /** A document's tree parent, by name (ID, else path); null for a root. Sections: unused. */
  parent: string | null;
  /** Why a document is a root although it declares a parent. */
  mark: TreeMark | null;
  /** A section's enclosing ID section; null: the document holds it. */
  within: string | null;
  isDocument: boolean;
}

/** A spec file of a mock corpus: its lines (no trailing empty one), its document and sections. */
export interface MockDocument {
  path: string;
  lines: string[];
  node: MockNode;
  /** Nested ID sections in file order. */
  sections: MockNode[];
}

/** A link written in a mock corpus, where it is written and where it lands. */
export interface MockLink {
  type: string;
  origin: LinkOrigin;
  written: string;
  path: string;
  line: number;
  /** The name of the node it lands on when resolved; null otherwise. */
  to: string | null;
  state: LinkState;
  reason: string | null;
}

/** What a mock project holds besides its queue: its spec files, their links, the tree's notes. */
export interface MockCorpus {
  documents: MockDocument[];
  links: MockLink[];
  /** The notes `spec tree` prints for the whole tree. */
  treeNotes: string[];
}

/** One invented project as the mock holds it: its spec and its queue. */
export interface MockProject {
  project: Project;
  /** The project's node kinds (`src/mocks/<slug>/kinds.ts`). */
  nodeKinds: readonly string[];
  /** The project's spec statuses (`src/mocks/<slug>/kinds.ts`). */
  specStatuses: readonly string[];
  corpus: MockCorpus;
  proposals: MockProposal[];
  notes: string[];
}

/** A review document as the mock stores one: never the exit-1 document, so these keys are set. */
export type StoredReview = Proposal & {
  id: string;
  project: string;
  kind: string;
  status: ProposalStatus;
  branch: string;
  created_at: string;
  updated_at: string;
  diagnostics: Finding[];
};

/**
 * A queued proposal as the mock holds it: its review document (`getProposal`) and the task it was
 * raised for, which only the task package reads (`docs/features/task-package.md` "Data").
 */
export interface MockProposal {
  review: StoredReview;
  task_id: string | null;
}

const MINUTE = 60_000;

/** UTC in the stored form, `2026-10-05T21:14:03Z`, `minutesAgo` before `now`. */
export function stamp(now: number, minutesAgo: number): string {
  return new Date(now - minutesAgo * MINUTE).toISOString().replace(/\.\d{3}Z$/, "Z");
}

/** A stable fake hex digest of `length` digits (FNV-1a over the text, repeated): not BLAKE3. */
export function fakeHex(text: string, length: number): string {
  let hash = 0x811c9dc5;
  let out = "";
  for (let round = 0; out.length < length; round += 1) {
    for (const char of `${text}:${String(round)}`) {
      hash ^= char.charCodeAt(0);
      hash = Math.imul(hash, 0x01000193) >>> 0;
    }
    out += hash.toString(16).padStart(8, "0");
  }
  return out.slice(0, length);
}

/** The core's estimate: a token per four characters, rounded up. */
export function tokensEst(text: string): number {
  return Math.ceil(Array.from(text).length / 4);
}

/** A nested section as a fixture declares it; its span is found from its heading. */
export interface SectionFields {
  id: string;
  kind: string | null;
  title: string | null;
  rev?: number | null;
}

/** A spec file as a fixture declares it; omitted keys take a plain live document's values. */
export interface DocumentFields {
  path: string;
  id: string | null;
  kind: string | null;
  title: string | null;
  status?: string | null;
  rev?: number | null;
  summary?: string | null;
  parent?: string | null;
  mark?: TreeMark | null;
  archived?: boolean;
  generated?: boolean;
  lines: string[];
  sections?: SectionFields[];
}

function headingLevel(line: string): number {
  const hashes = /^(#{1,6}) /.exec(line)?.[1];
  return hashes === undefined ? 0 : hashes.length;
}

/** A heading line's trailing attribute block naming `id` (`## Title {#ID}`, other attributes allowed). */
function namesInAttributes(line: string, id: string): boolean {
  const block = /\{([^{}]*)\}\s*$/.exec(line)?.[1];
  return block !== undefined && block.trim().split(/\s+/).includes(`#${id}`);
}

/**
 * A mock spec file. Each declared section starts at the heading line naming its ID (`## ID: Title`
 * or `## Title {#ID}`) and ends before the next heading of its level or above; it lies within the
 * nearest declared section around it.
 */
export function specFile(fields: DocumentFields): MockDocument {
  const { lines } = fields;
  const placed = (fields.sections ?? []).map((section) => {
    const at = lines.findIndex((line) => {
      const level = headingLevel(line);
      return (
        level > 0 &&
        ((line.slice(level + 1).startsWith(section.id) && /^[:\s]?$/.test(line.charAt(level + 1 + section.id.length))) ||
          namesInAttributes(line, section.id))
      );
    });
    if (at < 0) {
      throw new Error(`${fields.path}: no heading names ${section.id}`);
    }
    const level = headingLevel(lines[at] ?? "");
    let end = lines.length;
    for (let next = at + 1; next < lines.length; next += 1) {
      const nextLevel = headingLevel(lines[next] ?? "");
      if (nextLevel > 0 && nextLevel <= level) {
        end = next;
        break;
      }
    }
    return { section, level, line: at + 1, endLine: end };
  });
  const sections = placed.map(({ section, level, line, endLine }): MockNode => {
    const around = placed
      .filter((other) => other.level < level && other.line < line && other.endLine >= endLine)
      .sort((a, b) => b.line - a.line)[0];
    return {
      id: section.id,
      kind: section.kind,
      title: section.title,
      path: fields.path,
      line,
      endLine,
      rev: section.rev ?? fields.rev ?? 1,
      status: null,
      summary: null,
      archived: fields.archived ?? false,
      generated: fields.generated ?? false,
      utf8: true,
      parent: null,
      mark: null,
      within: around?.section.id ?? null,
      isDocument: false,
    };
  });
  return {
    path: fields.path,
    lines,
    node: {
      id: fields.id,
      kind: fields.kind,
      title: fields.title,
      path: fields.path,
      line: 1,
      endLine: lines.length,
      rev: fields.rev ?? 1,
      status: fields.status ?? null,
      summary: fields.summary ?? null,
      archived: fields.archived ?? false,
      generated: fields.generated ?? false,
      utf8: true,
      parent: fields.parent ?? null,
      mark: fields.mark ?? null,
      within: null,
      isDocument: true,
    },
    sections: sections.sort((a, b) => a.line - b.line),
  };
}

/** Front matter lines as a spec file opens with them; null values are left out. */
export function frontMatter(fields: Record<string, string | number | null>): string[] {
  const lines = Object.entries(fields)
    .filter((entry): entry is [string, string | number] => entry[1] !== null)
    .map(([key, value]) => `${key}: ${String(value)}`);
  return ["---", ...lines, "---"];
}

/** A link of a mock corpus; resolved ones land on `to`, the rest on nothing. */
export function link(fields: Omit<MockLink, "reason" | "to" | "state"> & Partial<Pick<MockLink, "reason" | "to" | "state">>): MockLink {
  const to = fields.to ?? null;
  return {
    reason: null,
    state: to === null ? "dangling" : "resolved",
    ...fields,
    to,
  };
}

/**
 * A queued proposal: its whole review document, omitted keys null or empty as the review JSON
 * gives them (an update's `target_ids` its `[target_id]`), and the task it was raised for.
 */
export function proposal(
  fields: Pick<StoredReview, "id" | "project" | "kind" | "created_at"> & Partial<StoredReview> & { task_id?: string | null },
): MockProposal {
  const { task_id = null, ...given } = fields;
  return {
    task_id,
    review: {
      status: "open",
      target_id: null,
      target_path: null,
      worktree: null,
      branch: "main",
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
      target_ids: fields.target_id === undefined || fields.target_id === null ? [] : [fields.target_id],
      severity: null,
      gap_type: null,
      summary: null,
      working_answer: null,
      price_of_other: null,
      evidence: [],
      options: [],
      recommendation: null,
      distinct_from: [],
      linked: null,
      record_id: null,
      record_path: null,
      record_title: null,
      record_text: null,
      choice: null,
      notes: [],
      ...given,
    },
  };
}

/** The most characters an inbox line keeps of a rationale or summary (the CLI's `INBOX_RATIONALE_CHARS`). */
const INBOX_LINE_CHARS = 80;

/** A text's first line as `spec inbox` prints it: a final CR dropped, over 80 characters 79 and an ellipsis (U+2026). */
function inboxLine(text: string): string {
  const first = text.split("\n")[0] ?? "";
  const line = first.endsWith("\r") ? first.slice(0, -1) : first;
  const chars = Array.from(line);
  return chars.length <= INBOX_LINE_CHARS ? line : `${chars.slice(0, INBOX_LINE_CHARS - 1).join("")}\u2026`;
}

/** A stored proposal's line of `spec inbox --json` (`docs/features/daemon-read.md` "Data"). */
export function inboxEntryOf(review: StoredReview): InboxEntry {
  const target = review.target_id ?? review.target_ids[0] ?? "";
  return {
    id: review.id,
    kind: review.kind,
    status: review.status,
    target_id: target,
    target_ids: review.target_ids.length > 0 ? [...review.target_ids] : [target],
    branch: review.branch,
    created_at: review.created_at,
    rationale: review.rationale === null ? null : inboxLine(review.rationale),
    severity: review.severity,
    summary: review.summary === null ? null : inboxLine(review.summary),
    record_id: review.record_id,
  };
}

/** `spec review PR --json` for a PR the queue does not hold (exit 1): every scalar null, the reason the last note. */
export function noReview(id: string): Proposal {
  return {
    id: null,
    project: null,
    kind: null,
    status: null,
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
    diagnostics: null,
    diff: null,
    preview: null,
    conflict: null,
    decided_by: null,
    decided_at: null,
    decision_note: null,
    applied_commit: null,
    created_at: null,
    updated_at: null,
    target_ids: [],
    severity: null,
    gap_type: null,
    summary: null,
    working_answer: null,
    price_of_other: null,
    evidence: [],
    options: [],
    recommendation: null,
    distinct_from: [],
    linked: null,
    record_id: null,
    record_path: null,
    record_title: null,
    record_text: null,
    choice: null,
    notes: [`no proposal \`${id}\` in this project's queue`],
  };
}
