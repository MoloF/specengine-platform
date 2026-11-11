import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { apiErrorOf, CARD_REFUSED } from "./client";
import { useClient } from "./provider";
import { cardListsOf, cardWritesKey, flowsOf, queryKeys, tracesOf } from "./queries";
import type { CardSplit, CardSplitBody } from "./types";

// The Split dialog's reads and its write (docs/features/department-split.md "Acceptance criteria",
// U1, U2): imported by src/board/SplitDialog.tsx alone, so they load with its chunk, never with the
// app. The keys are ./queries.ts's: the write shares the card writes' mutation key, which
// `useCardWriting` reads, and re-reads the same lists, traces, flows and card a card write does; the
// held trace is the very entry the document view reads. The plan's key is this module's alone: no
// other read shares it and no event reads it again.

/**
 * A split's plan as the cache keys it (docs/features/department-split.md "Acceptance criteria", U2):
 * its source, a document's REF and title or a stored task, and the discipline the read sent, every
 * one present, an absent one as `null`, as ./queries.ts's `queryKeys` are.
 */
function splitPlanKey(project: string, source: Pick<Partial<CardSplitBody>, "target" | "title" | "parent" | "discipline">) {
  return ["split-plan", project, { target: source.target ?? null, title: source.title ?? null, parent: source.parent ?? null, discipline: source.discipline ?? null }] as const;
}

/**
 * A split's plan (docs/canon/card-split.md "The rule"; docs/features/department-split.md
 * "Acceptance criteria", U2): one dry run as the Split dialog opens, `child` null so every
 * discipline the flows list is planned with its suggestion and targets. Held for the dialog's life:
 * no event, focus, reconnect or change to the form reads it again, and it leaves the cache with the
 * dialog, so the next opening reads anew. A read, not a write: outside the card writes' key, it
 * keeps no other dialog shut. `body.dry_run` is true.
 */
export function useSplitPlan(project: string, body: CardSplitBody) {
  const client = useClient();
  return useQuery({
    queryKey: splitPlanKey(project, body),
    queryFn: (): Promise<CardSplit> => client.splitCard(project, body),
    staleTime: Infinity,
    gcTime: 0,
    refetchOnWindowFocus: false,
    refetchOnReconnect: false,
  });
}

/**
 * A split written (docs/features/department-split.md "Acceptance criteria", U2): one POST, all
 * cards or none, under the card writes' key, so nothing opens or closes while it is out
 * (`useCardWriting`). After an answer, a refusal or no answer at all, the project's card lists (the
 * children reads among them), its traces and flows, and a stored parent's read are read again, as
 * after a card write; the call settles once they have.
 */
export function useCardSplit(project: string) {
  const client = useClient();
  const queryClient = useQueryClient();
  async function readAgain(parent: string | null) {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: cardListsOf(project) }),
      queryClient.invalidateQueries({ queryKey: tracesOf(project) }),
      queryClient.invalidateQueries({ queryKey: flowsOf(project) }),
      parent === null ? Promise.resolve() : queryClient.invalidateQueries({ queryKey: queryKeys.card(project, parent), exact: true }),
    ]);
  }
  return useMutation({
    mutationKey: cardWritesKey(project),
    mutationFn: (body: CardSplitBody): Promise<CardSplit> => client.splitCard(project, body),
    onSuccess: (_answer, body) => readAgain(body.parent),
    onError: async (error, body) => {
      const { status } = apiErrorOf(error);
      if (status === CARD_REFUSED || status === 0) {
        await readAgain(body.parent);
      }
    },
  });
}

/**
 * A document's trace read as the view around the dialog holds it, under the REF that view read it
 * by (the route's, an alias or a path among them), never asked from here: the Split dialog lists the
 * document's tasks from it when held, and nothing when it is not (docs/features/department-split.md
 * "Acceptance criteria", U2).
 */
export function useHeldTrace(project: string, ref: string) {
  const client = useClient();
  return useQuery({
    queryKey: queryKeys.trace(project, { ref }),
    queryFn: ({ signal }) => client.getTrace(project, { ref }, signal),
    enabled: false,
  });
}
