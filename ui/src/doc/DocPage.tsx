import { useId } from "react";
import { useDocNode } from "../api/queries";
import { DocFrame } from "./DocFrame";

// A document's page, `#/<p>/tree/<REF>/page` (docs/features/doc-modal.md "Acceptance criteria",
// DM-11): the modal's body alone, its `h1` the title (the REF until read), no tools; the nav marks
// Spec tree, a reload keeps it. An ancestor crumb is followed as a link; a document link opens the
// modal over the page.

export function DocPage({
  project,
  docRef,
  insideOpen,
  onInside,
  historyOpen,
  onHistory,
}: {
  project: string;
  docRef: string;
  insideOpen: boolean;
  onInside: (open: boolean) => void;
  historyOpen: boolean;
  onHistory: (open: boolean) => void;
}) {
  const titleId = useId();
  const read = useDocNode(project, docRef);
  const title = read.data?.nodes[0]?.title ?? null;
  const heading = (
    <h1 id={titleId} tabIndex={-1} className="doc-heading">
      <bdi>{title ?? docRef}</bdi>
    </h1>
  );
  return (
    <section className="view doc-page" aria-labelledby={titleId}>
      <DocFrame project={project} docRef={docRef} frame="page" heading={heading} tools={null} insideOpen={insideOpen} onInside={onInside} historyOpen={historyOpen} onHistory={onHistory} />
    </section>
  );
}
