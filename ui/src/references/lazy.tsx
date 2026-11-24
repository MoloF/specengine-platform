import { lazy, Suspense } from "react";
import { useTranslate } from "../i18n";
import { Skeleton } from "../ui/states";

// Where the References part stands (docs/features/work-references.md "UI"): above the Comments part in
// the card's work item (modal and page), the document view and the proposal pane, open or closed. The
// part arrives in a chunk of its own, never with the app's first load; its region, named as the part
// is, and its heading stand meanwhile, so nothing jumps.

const ReferencesPart = lazy(() => import("./ReferencesPart").then((module) => ({ default: module.ReferencesPart })));

/** What a References part is told. */
export interface ReferencesProps {
  project: string;
  /** What the references are under, as the daemon takes `on`: a card's or a proposal's ID, a node's canonical ID, an ID-less document's path. */
  on: string;
  /** The documents "Promote to canon…" offers, the first chosen at first (docs/canon/work-references-promotion-surfaces.md "UI"): the node; a card's targets; a proposal's targets. */
  targets: readonly string[];
  /** The part heading's level among its siblings'. */
  level: 2 | 3;
}

export function References(props: ReferencesProps) {
  const t = useTranslate();
  const Heading = props.level === 2 ? "h2" : "h3";
  return (
    <Suspense
      fallback={
        <section className="references" aria-label={t("app.references")} aria-busy="true">
          <Heading className="issue-section-title">{t("app.references")}</Heading>
          <Skeleton label={t("app.lazy.references")} lines={2} />
        </section>
      }
    >
      <ReferencesPart {...props} />
    </Suspense>
  );
}
