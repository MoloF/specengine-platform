import { lazy, Suspense } from "react";
import type { Evidence, Pair } from "../api/types";
import { openDocFrom } from "../app/location";
import { useTranslate } from "../i18n";
import { Skeleton } from "../ui/states";

/**
 * A collision pair's view arrives in a chunk of its own, loaded only for an item that holds a `pair`
 * (docs/features/collision-pairs.md "Acceptance criteria", PP9, PP10): the Inbox's card, a Questions
 * item and a task's Answer… each stand it here, right after the item's summary, in place of the flat
 * evidence list; until it loads, a skeleton named as its region.
 */
const PairView = lazy(() => import("./PairView"));

/** The two documents of a two-sided collision, as served, with their evidence. */
export function PairSlot({ pair, evidence }: { pair: Pair; evidence: readonly Evidence[] }) {
  const t = useTranslate();
  return (
    <Suspense fallback={<Skeleton label={t("inbox.pair.label")} lines={4} />}>
      <PairView pair={pair} evidence={evidence} onOpen={openDocFrom} />
    </Suspense>
  );
}
