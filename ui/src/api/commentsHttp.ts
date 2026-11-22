import type { CommentsOptions, MentionsOptions } from "./client";
import { projectPath, queryOf, request, segment } from "./http";
import type { CommentAddBody, CommentDocument, CommentEditBody, CommentList, CommentRaiseBody, IntakeDocument, MentionList } from "./types";

// The comments' requests (`docs/features/comments.md` "HTTP"), `HttpClient`'s in a chunk of their
// own, reached by its dynamic import alone. The two reads judged: a `{id, reason}` 404 or 409 rejects
// with its reason, an older daemon's unknown route in its words; the writes a JSON body (the delete
// none) and no AbortSignal: 200 the comment document (a raise, the intake document), a refusal (an
// error body 400 or 403, `{id, reason}` 404 or 409, an intake document's last note) rejecting verbatim.

/** A project's comments; one comment's edit, items and delete are under it. */
function commentsPath(project: string): string {
  return `${projectPath(project)}/comments`;
}

/** `spec comment list --json`: `on`, then `before` and `limit` when given. */
export async function getComments(project: string, options: CommentsOptions, signal?: AbortSignal): Promise<CommentList> {
  const query = queryOf([
    ["on", options.on],
    ["before", options.before],
    ["limit", options.limit],
  ]);
  return (await request("GET", `${commentsPath(project)}${query}`, { signal, judged: true })) as CommentList;
}

/** `spec comment mentions --json`: `login`, `before` and `limit`, each when given. */
export async function getMentions(project: string, options: MentionsOptions, signal?: AbortSignal): Promise<MentionList> {
  const query = queryOf([
    ["login", options.login],
    ["before", options.before],
    ["limit", options.limit],
  ]);
  return (await request("GET", `${projectPath(project)}/mentions${query}`, { signal, judged: true })) as MentionList;
}

export async function addComment(project: string, body: CommentAddBody): Promise<CommentDocument> {
  return (await request("POST", commentsPath(project), { body: JSON.stringify(body) })) as CommentDocument;
}

export async function editComment(project: string, id: string, body: CommentEditBody): Promise<CommentDocument> {
  return (await request("POST", `${commentsPath(project)}/${segment(id)}/edit`, { body: JSON.stringify(body) })) as CommentDocument;
}

export async function deleteComment(project: string, id: string): Promise<CommentDocument> {
  return (await request("DELETE", `${commentsPath(project)}/${segment(id)}`)) as CommentDocument;
}

export async function raiseFromComment(project: string, id: string, body: CommentRaiseBody): Promise<IntakeDocument> {
  return (await request("POST", `${commentsPath(project)}/${segment(id)}/items`, { body: JSON.stringify(body) })) as IntakeDocument;
}
