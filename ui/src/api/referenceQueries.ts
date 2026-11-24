import { useInfiniteQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useClient } from "./provider";
import { cardWritesKey, queryKeys, referencesOf } from "./queries";
import type { PromoteBody, Proposal, Reference, ReferenceAddBody } from "./types";

// The references' reads and writes (docs/features/work-references.md "UI"; ADR-0088), imported by
// src/references/ alone so they load with its chunk, never with the app. The keys are under
// ./queries.ts's `referencesOf`, which the live tail reads again on a `reference.*` event and a gap.
// An attach and a delete go under the card writes' mutation key, so no dialog opens or closes while
// one is out, and settle once the project's references are read again: nothing here is optimistic.
// An upload is no such write: it streams with its own progress and Cancel, so it is not under that key.
// A promotion (docs/canon/work-references-promotion-surfaces.md "UI") stores a proposal and writes nothing else:
// under the same key, it settles once the inbox is read again.

/** One target's references, as the page names it (a card's or a proposal's ID, a node's canonical ID or path). */
export function referencesKey(project: string, on: string) {
  return [...referencesOf(project), on] as const;
}

/**
 * A target's references, page by page: the first read the newest page (oldest first within it), each
 * "Load older" the page before the last one's `next_before`. Judged: a refusal rejects in the
 * daemon's words; the query's AbortSignal taken.
 */
export function useReferences(project: string, on: string) {
  const client = useClient();
  return useInfiniteQuery({
    queryKey: referencesKey(project, on),
    queryFn: ({ pageParam, signal }) => client.getReferences(project, pageParam === null ? { on } : { on, before: pageParam }, signal),
    initialPageParam: null as string | null,
    getNextPageParam: (page) => page.next_before,
  });
}

export type ReferencesQuery = ReturnType<typeof useReferences>;

/** One reference write as the part sends it. */
export type ReferenceWrite = { write: "add"; body: ReferenceAddBody } | { write: "delete"; id: string };

/** A reference attached or deleted: one call, then the project's references read again, answered or refused, and the call settles once they have. */
export function useReferenceWrite(project: string) {
  const client = useClient();
  const queryClient = useQueryClient();
  return useMutation({
    mutationKey: cardWritesKey(project),
    mutationFn: (request: ReferenceWrite): Promise<Reference> => (request.write === "add" ? client.addReference(project, request.body) : client.deleteReference(project, request.id)),
    onSettled: () => queryClient.invalidateQueries({ queryKey: referencesOf(project) }),
  });
}

/** What a promotion sends: the reference and the route's body. */
export interface PromoteRequest {
  id: string;
  body: PromoteBody;
}

/**
 * "Promote to canon": one POST storing a `reference` proposal a person confirms in the Inbox, nothing
 * written under the root. Settled, stored or refused, the inbox is read again, and the call settles
 * once it has; the references and their "In the spec" change only when the proposal is applied.
 */
export function usePromote(project: string) {
  const client = useClient();
  const queryClient = useQueryClient();
  return useMutation({
    mutationKey: cardWritesKey(project),
    mutationFn: ({ id, body }: PromoteRequest): Promise<Proposal> => client.promoteReference(project, id, body),
    onSettled: () => queryClient.invalidateQueries({ queryKey: queryKeys.inbox(project) }),
  });
}
