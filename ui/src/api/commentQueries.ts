import { useInfiniteQuery, useMutation, useQuery, useQueryClient, type InfiniteData } from "@tanstack/react-query";
import { useClient } from "./provider";
import { cardListsOf, cardWritesKey, commentsOf, flowsOf, mentionsOf, queryKeys, tracesOf } from "./queries";
import type { CommentAddBody, CommentDocument, CommentEditBody, CommentList, CommentRaiseBody, IntakeDocument } from "./types";

// The comments' reads and writes (docs/features/comments.md "UI"; ADR-0087), imported by src/comments/
// alone so they load with its chunk, never with the app, as ./splitQueries.ts does for the Split
// dialog. The keys are under ./queries.ts's `commentsOf` and `mentionsOf`, which the live tail reads
// again on a `comment.*` event and a gap. Every write goes under the card writes' mutation key, so no
// dialog or modal opens or closes while one is out (`useCardWriting`), and settles once what it
// changed is read again: nothing here is optimistic, the thread on screen is the daemon's.

/** One thread: a target's comments, as the page names it (a card's or a proposal's ID, a node's canonical ID or path). */
export function commentThreadKey(project: string, on: string) {
  return [...commentsOf(project), on] as const;
}

/** One mentions read: a login's, or every mention (null). */
export function mentionsKey(project: string, login: string | null) {
  return [...mentionsOf(project), login] as const;
}

/**
 * A target's comments, page by page: the first read the newest page (oldest first within it), each
 * "Load older" the page before the last one's `next_before`. A live event or a write reads every
 * page held again; the query's AbortSignal taken. Judged: a refusal rejects in the daemon's words.
 */
export function useComments(project: string, on: string) {
  const client = useClient();
  return useInfiniteQuery({
    queryKey: commentThreadKey(project, on),
    queryFn: ({ pageParam, signal }) => client.getComments(project, pageParam === null ? { on } : { on, before: pageParam }, signal),
    initialPageParam: null as string | null,
    getNextPageParam: (page) => page.next_before,
  });
}

export type CommentsQuery = ReturnType<typeof useComments>;

/**
 * A comment as the thread under `on` holds it now, read from the cache, never asked: after a write's
 * call settles (its re-read included) the thread says whether the comment still stands and at which
 * version, before the view draws it again.
 */
export function useHeldComment(project: string, on: string): (id: string) => CommentDocument | undefined {
  const queryClient = useQueryClient();
  return (id) =>
    queryClient
      .getQueryData<InfiniteData<CommentList>>(commentThreadKey(project, on))
      ?.pages.flatMap((page) => page.comments)
      .find((comment) => comment.id === id);
}

/**
 * The mentions, newest first, page by page: `login` sent when given, else none (a hub's login is the
 * daemon's; locally, every mention). The badge counts the first page; the view pages back. Unread
 * until `enabled`: whose they are is known.
 */
export function useMentions(project: string, login: string | null, enabled = true) {
  const client = useClient();
  return useInfiniteQuery({
    enabled,
    queryKey: mentionsKey(project, login),
    queryFn: ({ pageParam, signal }) => {
      const before = pageParam === null ? {} : { before: pageParam };
      return client.getMentions(project, login === null ? before : { login, ...before }, signal);
    },
    initialPageParam: null as string | null,
    getNextPageParam: (page) => page.next_before,
  });
}

export type MentionsQuery = ReturnType<typeof useMentions>;

/**
 * The nav's badge's read: the first page of the mentions alone, under a key of its own, so a
 * `comment.*` event reads one page for it, never the pages the Mentions view has paged back
 * (docs/features/comments.md "UI", Mentions). Unread until `enabled`: whose they are is known.
 */
export function useMentionsBadge(project: string, login: string | null, enabled: boolean) {
  const client = useClient();
  return useQuery({
    enabled,
    queryKey: [...mentionsKey(project, login), "first"] as const,
    queryFn: ({ signal }) => client.getMentions(project, login === null ? {} : { login }, signal),
  });
}

/** One comment write as the thread sends it. */
export type CommentWrite = { write: "add"; body: CommentAddBody } | { write: "edit"; id: string; body: CommentEditBody } | { write: "delete"; id: string };

/**
 * A comment added, edited or deleted: one call, then the project's threads and mentions read again,
 * answered or refused (a stale version's 409 among them, which the thread shows reloaded), and the
 * call settles once they have.
 */
export function useCommentWrite(project: string) {
  const client = useClient();
  const queryClient = useQueryClient();
  async function readAgain() {
    await Promise.all([queryClient.invalidateQueries({ queryKey: commentsOf(project) }), queryClient.invalidateQueries({ queryKey: mentionsOf(project) })]);
  }
  return useMutation({
    mutationKey: cardWritesKey(project),
    mutationFn: (request: CommentWrite): Promise<CommentDocument> => {
      switch (request.write) {
        case "add":
          return client.addComment(project, request.body);
        case "edit":
          return client.editComment(project, request.id, request.body);
        case "delete":
          return client.deleteComment(project, request.id);
      }
    },
    onSettled: readAgain,
  });
}

/**
 * A question, a decision or a discrepancy raised from a comment: one POST, under the card writes'
 * key. Settled, stored or not, the project's threads (a comment's items), the inbox, the round sheets
 * and every card read (a card comment's item names its card), its lists, traces and flows are read
 * again, as after a card's raise, and the call settles once they have.
 */
export function useCommentRaise(project: string) {
  const client = useClient();
  const queryClient = useQueryClient();
  async function readAgain() {
    await Promise.all(
      [commentsOf(project), queryKeys.inbox(project), ["round", project], ["card", project], cardListsOf(project), tracesOf(project), flowsOf(project)].map((queryKey) => queryClient.invalidateQueries({ queryKey })),
    );
  }
  return useMutation({
    mutationKey: cardWritesKey(project),
    mutationFn: ({ id, body }: { id: string; body: CommentRaiseBody }): Promise<IntakeDocument> => client.raiseFromComment(project, id, body),
    onSettled: readAgain,
  });
}
