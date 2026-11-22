import type { CommentDocument, CommentOn } from "../api/types";
import { sectionHash } from "../app/routes";
import { byAgent } from "../inbox/labels";

// The daemon's words a comment carries, told apart here alone (docs/features/comments.md "UI",
// Strings; policy.test.ts): what it is under, and who may act on it as the page shows it. The daemon
// judges every write either way, a refusal in its own words; nothing here judges a person.

/** What a comment is under: the comment document's `on.kind` words (docs/features/comments.md "The comment document"). */
const PLACES = { card: true, proposal: true, node: true } as const;

/** A place this build knows; another word is kept and shown raw. */
export type CommentPlace = keyof typeof PLACES;

/** What a comment is under, a word this build knows, else null: the place's ID is then shown raw. */
export function placeOf(on: CommentOn): CommentPlace | null {
  return Object.hasOwn(PLACES, on.kind) ? (on.kind as CommentPlace) : null;
}

/** Where a link to what a comment is under goes: a route's hash (a task's card, a proposal in the Inbox), a document by its REF, or nowhere for a place this build does not know. */
export type PlaceLink = { href: string } | { doc: string } | null;

/** The link to what a comment is under, looked up by its place's word, never compared elsewhere. */
export function placeLink(project: string, on: CommentOn): PlaceLink {
  switch (placeOf(on)) {
    case "card":
      return { href: sectionHash(project, "board", on.id) };
    case "proposal":
      return { href: sectionHash(project, "inbox", on.id) };
    case "node":
      return { doc: on.id };
    case null:
      return null;
  }
}

/** The three items a comment turns into, the daemon's words (`CommentRaiseBody`'s `kind`, docs/features/comments.md "HTTP"). */
export const TURNS = { question: "question", decision: "decision", discrepancy: "discrepancy" } as const;

/** What a comment turns into. */
export type Turn = keyof typeof TURNS;

/** The login grammar (`docs/canon/hub-deploy.md#accounts`): what a mention names, and what the Mentions view asks locally. */
const LOGIN = /^[a-z]([a-z0-9._-]{0,62}[a-z0-9])?$/;

/** Whether a name is a login by its grammar: locally the Mentions view lists that name's, else every mention. */
export function fitsLogin(name: string): boolean {
  return LOGIN.test(name);
}

/** Who the page acts as for a comment: a hub's login, else the acting name as typed (`local` when none, the daemon's word for it). */
export interface Acting {
  login: string | null;
  name: string;
}

/** The word a write with no name records (`docs/canon/card-flows.md` "Hand-off"): the actor of a comment made so. */
const NO_NAME = "local";

/** The actor a write from this page records: the login, else the name trimmed, else `local`. */
export function actorOf({ login, name }: Acting): string {
  return login ?? (name.trim() === "" ? NO_NAME : name.trim());
}

/** Whether the comment is this page's person's: the login against `author.login` on a hub, the acting name against `actor` locally. */
export function isOwn(comment: CommentDocument, acting: Acting): boolean {
  return acting.login !== null ? comment.author.login === acting.login : comment.actor === actorOf(acting);
}

/** Edit stands on one's own live comment a person wrote: an agent's is never edited, only deleted (docs/canon/comments.md "Authors and edits"). */
export function canEdit(comment: CommentDocument, acting: Acting): boolean {
  return comment.deleted === null && !byAgent(comment.author) && isOwn(comment, acting);
}

/**
 * Delete on a live comment: on (`true`) where it is one's own, where the hub's `allowed` lists
 * `delete_comment` (`allows`, unknown reading on), and anywhere locally; else off, `ActOff` beside it.
 */
export function deleteStands(comment: CommentDocument, acting: Acting, allows: boolean): boolean {
  return acting.login === null || isOwn(comment, acting) || allows;
}

/** A reply stands on a live top-level comment: one level of replies (docs/canon/comments.md "Threads"). */
export function takesReply(comment: CommentDocument): boolean {
  return comment.deleted === null && comment.reply_to === null;
}

/** The most bytes a body prefills an item's text with (docs/features/comments.md "UI", Turn into…). */
export const PREFILL_BYTES = 1024;

/** The text's first `max` bytes of UTF-8, cut at a character boundary, never inside a code point. */
export function firstBytes(text: string, max: number): string {
  const encoder = new TextEncoder();
  let bytes = 0;
  let end = 0;
  for (const character of text) {
    const size = encoder.encode(character).length;
    if (bytes + size > max) {
      break;
    }
    bytes += size;
    end += character.length;
  }
  return text.slice(0, end);
}

/** A row of the Mentions view as the browser keeps it seen: its ID and version, so an edit counts as unseen again. */
export function seenWord(comment: Pick<CommentDocument, "id" | "version">): string {
  return `${comment.id}@${String(comment.version)}`;
}
