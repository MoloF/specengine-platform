import { ClientError, type CommentsOptions, type MentionsOptions } from "../api/client";
import type { CommentAddBody, CommentDocument, CommentEditBody, CommentList, CommentRaiseBody, IntakeDocument, MentionList } from "../api/types";
import { proposal, stamp, type MockProposal } from "./build";

// The mock's comments (docs/features/comments.md "Data", "HTTP"; ADR-0087): harbor-sim's talk under
// a task, a document and a proposal, held in memory until a reload, answered as `spec serve` would:
// a list oldest first in pages of 100 by ID, the newest page first; mentions newest first; a write
// refused in the CLI's words (400 a body, 403 another's on a hub, 404 none, 409 a stale version or a
// reply rule). A body's text is invented data; non-Latin text is escaped here (ADR-0072).

/** The most comments a page lists (`COMMENT_PAGE_MAX`). */
const PAGE = 100;

/** The most bytes a body holds (`COMMENT_BODY_MAX`). */
const BODY_MAX = 8192;

/** The word a write with no name records. */
const LOCAL = "local";

/** A mention: `@` at the start or after no login character, then the longest login (docs/features/comments.md "Mentions grammar"); code is not left out here. */
const MENTION = /(?<![A-Za-z0-9._-])@([a-z](?:[a-z0-9._-]{0,62}[a-z0-9])?)/g;

function mentionsOf(body: string): string[] {
  return [...new Set([...body.matchAll(MENTION)].map((match) => match[1] ?? ""))].filter((login) => login !== "").sort();
}

function numberOf(id: string): number {
  return Number(id.replace(/^\D+-/, ""));
}

/** One project's comments, by ID number. */
export interface MockComments {
  rows: CommentDocument[];
}

const PERSON = (login: string | null) => (login === null ? { type: "human", role: null, model: null, run: null } : { type: "human", role: null, model: null, run: null, login });

/** harbor-sim's talk: under C-0003 a question with an agent's reply and a deleted remark, under MEC-MOORING a remark in Cyrillic naming mira, and one under PR-0049. */
export function mockComments(slug: string, now: number): MockComments {
  if (slug !== "harbor-sim") {
    return { rows: [] };
  }
  const at = (minutesAgo: number) => stamp(now, minutesAgo);
  const row = (fields: Partial<CommentDocument> & Pick<CommentDocument, "id" | "on" | "actor" | "created_at">): CommentDocument => ({
    on_rev: null,
    reply_to: null,
    author: PERSON(fields.actor),
    version: 1,
    body: null,
    mentions: [],
    updated_at: fields.created_at,
    deleted: null,
    items: [],
    ...fields,
  });
  const card = { kind: "card", id: "C-0003" };
  const node = { kind: "node", id: "MEC-MOORING" };
  return {
    rows: [
      row({
        id: "CM-0001",
        on: card,
        actor: "ivo",
        created_at: at(180),
        updated_at: at(170),
        version: 2,
        body: "Keep the **night** speed at 6 knots, as `RULE-FAIRWAY-SPEED` writes it? @tomas knows the log, @mira decides.",
        mentions: ["mira", "tomas"],
      }),
      row({
        id: "CM-0002",
        on: card,
        reply_to: "CM-0001",
        actor: "tomas",
        author: { type: "agent", role: "spec-writer", model: "claude-opus-5-5", run: "run-77", login: "tomas" },
        created_at: at(150),
        body: "The log says 5 knots after dark:\n\n- RULE-FAIRWAY-SPEED, line 12\n- the pilots' office notes",
      }),
      row({ id: "CM-0003", on: card, actor: "mira", created_at: at(120), deleted: { by: "mira", at: at(110) } }),
      row({
        id: "CM-0004",
        on: node,
        on_rev: 2,
        actor: "lev",
        created_at: at(90),
        // "Mooring" in Russian, escaped: shown as written, any script (ADR-0072).
        body: "\u0428\u0432\u0430\u0440\u0442\u043e\u0432\u043a\u0430: the second line holds at spring tide. @mira",
        mentions: ["mira"],
      }),
      row({ id: "CM-0005", on: { kind: "proposal", id: "PR-0049" }, actor: "ivo", created_at: at(60), body: "Read with DEC-0012 before you stage it." }),
    ],
  };
}

/** A target's page: the newest `limit` older than `before`, oldest first. */
export function listIn(store: MockComments, options: CommentsOptions, kind: string): CommentList {
  const all = store.rows.filter((comment) => comment.on.id === options.on).sort((a, b) => numberOf(a.id) - numberOf(b.id));
  const older = options.before === undefined ? all : all.filter((comment) => numberOf(comment.id) < numberOf(options.before ?? ""));
  const page = older.slice(-(options.limit ?? PAGE));
  const first = page[0];
  return {
    on: all[0]?.on ?? { kind, id: options.on },
    total: all.length,
    comments: structuredClone(page),
    next_before: first !== undefined && older.length > page.length ? first.id : null,
    notes: [],
  };
}

/** A login's mentions, every one's without a login, newest first. */
export function mentionsIn(store: MockComments, options: MentionsOptions): MentionList {
  const login = options.login ?? null;
  const all = store.rows.filter((comment) => (login === null ? comment.mentions.length > 0 : comment.mentions.includes(login))).sort((a, b) => numberOf(b.id) - numberOf(a.id));
  const older = options.before === undefined ? all : all.filter((comment) => numberOf(comment.id) < numberOf(options.before ?? ""));
  const page = older.slice(0, options.limit ?? PAGE);
  const last = page.at(-1);
  return { login, comments: structuredClone(page), next_before: last !== undefined && older.length > page.length ? last.id : null, notes: [] };
}

function bodyProblem(body: string): string | null {
  const bytes = new TextEncoder().encode(body).length;
  if (body.trim() === "") {
    return "body: blank: give a value";
  }
  if (bytes > BODY_MAX) {
    return `body: ${String(bytes)} bytes; at most ${String(BODY_MAX)}`;
  }
  if (body.includes("\u0000")) {
    return "body: holds U+0000";
  }
  const mentions = mentionsOf(body);
  return mentions.length > 16 ? `body: ${String(mentions.length)} mentions; at most 16` : null;
}

function stored(store: MockComments, id: string): CommentDocument {
  const found = store.rows.find((comment) => comment.id === id);
  if (found === undefined) {
    throw new ClientError({ status: 404, message: `no comment ${id}` });
  }
  return found;
}

/** A comment added under `on`, by `actor` (a hub's login, else `as` or `local`); `kind` what `on` names. */
export function addIn(store: MockComments, body: CommentAddBody, kind: string, actor: string, login: string | null, at: string): CommentDocument {
  const problem = bodyProblem(body.body);
  if (problem !== null) {
    throw new ClientError({ status: 400, message: problem });
  }
  if (body.reply_to !== null) {
    const to = stored(store, body.reply_to);
    if (to.reply_to !== null) {
      throw new ClientError({ status: 409, message: `reply_to: ${to.id} is a reply: a reply names a top-level comment` });
    }
    if (to.on.id !== body.on) {
      throw new ClientError({ status: 409, message: `reply_to: ${to.id} is on ${to.on.id}, not ${body.on}` });
    }
  }
  const highest = store.rows.reduce((most, comment) => Math.max(most, numberOf(comment.id)), 0);
  const made: CommentDocument = {
    id: `CM-${String(highest + 1).padStart(4, "0")}`,
    on: { kind, id: body.on },
    on_rev: null,
    reply_to: body.reply_to,
    actor,
    author: PERSON(login),
    version: 1,
    body: body.body,
    mentions: mentionsOf(body.body),
    created_at: at,
    updated_at: at,
    deleted: null,
    items: [],
  };
  store.rows.push(made);
  return structuredClone(made);
}

/** An edit by its author alone, at the version stored. */
export function editIn(store: MockComments, id: string, body: CommentEditBody, actor: string, at: string): CommentDocument {
  const comment = stored(store, id);
  if (comment.deleted !== null) {
    throw new ClientError({ status: 409, message: `${id} is deleted: nothing changed` });
  }
  if (comment.author.type === "agent") {
    throw new ClientError({ status: 403, message: `refused: ${id} is an agent's: it is never edited, only deleted` });
  }
  if (comment.actor !== actor) {
    throw new ClientError({ status: 403, message: `refused: ${id} is ${comment.actor}'s: only its author edits it` });
  }
  if (comment.version !== body.version) {
    throw new ClientError({ status: 409, message: `${id} is at version ${String(comment.version)}, not ${String(body.version)}: nothing changed` });
  }
  const problem = bodyProblem(body.body);
  if (problem !== null) {
    throw new ClientError({ status: 400, message: problem });
  }
  Object.assign(comment, { body: body.body, mentions: mentionsOf(body.body), version: comment.version + 1, updated_at: at });
  return structuredClone(comment);
}

/** A delete: its tombstone, every version's text gone. `allowed` false refuses another's, as a hub's `[team]` would. */
export function deleteIn(store: MockComments, id: string, actor: string, allowed: boolean, at: string): CommentDocument {
  const comment = stored(store, id);
  if (comment.deleted !== null) {
    throw new ClientError({ status: 409, message: `${id} is deleted: nothing changed` });
  }
  if (!allowed && comment.actor !== actor) {
    throw new ClientError({ status: 403, message: `refused: ${id} is ${comment.actor}'s: only its author or an \`owner\` deletes it` });
  }
  Object.assign(comment, { body: null, mentions: [], deleted: { by: actor, at } });
  return structuredClone(comment);
}

/** An item raised from a live comment into the queue: its `comment_id` set, listed among the comment's items. */
export function raiseIn(store: MockComments, queue: MockProposal[], project: string, id: string, body: CommentRaiseBody, at: string): IntakeDocument {
  const comment = stored(store, id);
  if (comment.deleted !== null) {
    throw new ClientError({ status: 409, message: `${id} is deleted: nothing changed` });
  }
  if (body.node_ids.length === 0 || body.text.trim() === "") {
    throw new ClientError({ status: 409, message: body.node_ids.length === 0 ? "node_ids: 0 IDs; 1 to 16" : "text: blank: give a value" });
  }
  const highest = queue.reduce((most, { review }) => Math.max(most, numberOf(review.id)), 0);
  const made = `PR-${String(highest + 1).padStart(4, "0")}`;
  queue.push(
    proposal({
      id: made,
      project,
      kind: body.kind,
      created_at: at,
      target_id: body.node_ids[0] ?? null,
      target_ids: [...body.node_ids],
      summary: body.text,
      working_answer: body.working_answer ?? null,
      price_of_other: body.price_of_other ?? null,
      why: body.why ?? null,
      gap_type: body.gap_type ?? null,
      evidence: body.observed === undefined ? [] : [{ file: id, qpath: null, lines: null, observed: body.observed, documented: body.documented ?? "" }],
      author: PERSON(null),
      card_id: comment.on.kind === "card" ? comment.on.id : null,
      comment_id: id,
    }),
  );
  comment.items.push({ id: made, kind: body.kind, status: "open" });
  return { id: made, created: true, hits: [], related: [], linked: null, diagnostics: [], notes: [] };
}

export { LOCAL };
