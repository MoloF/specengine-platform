import { lazy, Suspense, type ReactNode } from "react";
import { useDocNode } from "../api/queries";
import { RegionBoundary } from "../app/RegionBoundary";
import { useTranslate } from "../i18n";
import { IssueView } from "../ui/IssueView";
import { Skeleton } from "../ui/states";
import type { PartLevel } from "./DocLinks";

// The document view's frame (docs/features/doc-modal.md "Acceptance criteria", DM-9, DM-11): the work
// item's layout around the frame's own title, the modal's `h2` or the page's `h1`, and its tools, in
// first load; every part around them comes from one chunk of its own (./DocBody.tsx, DM-20), each
// slot waiting for it alone. The title is never swapped when the chunk lands, so focus on it stays
// where the dialog put it: a mouse open shows no focus ring, a keyboard one does.

/** The parts' chunk, one import for every slot. */
const parts = () => import("./DocBody");
const DocCrumbs = lazy(() => parts().then((module) => ({ default: module.DocCrumbs })));
const DocMeta = lazy(() => parts().then((module) => ({ default: module.DocMeta })));
const DocActs = lazy(() => parts().then((module) => ({ default: module.DocActs })));
const DocMain = lazy(() => parts().then((module) => ({ default: module.DocMain })));
const DocSide = lazy(() => parts().then((module) => ({ default: module.DocSide })));

export interface DocFrameProps {
  project: string;
  docRef: string;
  /** The modal (its parts' headings `h3`) or the page (`h2`). */
  frame: "modal" | "page";
  /** The modal's `h2` or the page's `h1`: the title, the REF until read. */
  heading: ReactNode;
  /** On the breadcrumb line: the modal's Open as a page and Close; the page has none. */
  tools: ReactNode;
  /** Inside expanded: the Shell's choice for the project, kept for the page's life. */
  insideOpen: boolean;
  onInside: (open: boolean) => void;
  /** In the modal, an ancestor crumb's plain click replaces the document; on the page it is followed. */
  onCrumb?: (ref: string) => void;
}

export function DocFrame({ project, docRef, frame, heading, tools, insideOpen, onInside, onCrumb }: DocFrameProps) {
  const t = useTranslate();
  const read = useDocNode(project, docRef);
  const data = read.data;
  const holder = data?.nodes[0];
  const level: PartLevel = frame === "modal" ? 3 : 2;
  const reading = (
    <div aria-busy="true">
      <Skeleton label={t("doc.reading")} lines={6} />
    </div>
  );
  return (
    <IssueView
      breadcrumbs={
        <Suspense fallback={null}>
          <DocCrumbs project={project} docRef={docRef} holder={holder} onCrumb={onCrumb} />
        </Suspense>
      }
      tools={tools}
      title={heading}
      meta={
        holder === undefined ? null : (
          <Suspense fallback={null}>
            <DocMeta project={project} docRef={docRef} holder={holder} />
          </Suspense>
        )
      }
      actions={
        data === undefined ? null : (
          <Suspense fallback={null}>
            <DocActs project={project} docRef={docRef} holder={holder} />
          </Suspense>
        )
      }
      main={
        <RegionBoundary name={t("doc.boundary")}>
          <Suspense fallback={reading}>
            <DocMain project={project} docRef={docRef} read={read} level={level} insideOpen={insideOpen} onInside={onInside} />
          </Suspense>
        </RegionBoundary>
      }
      side={
        holder === undefined || data === undefined ? null : (
          <RegionBoundary name={t("doc.boundary")}>
            <Suspense fallback={null}>
              <DocSide project={project} docRef={docRef} holder={holder} read={data} level={level} />
            </Suspense>
          </RegionBoundary>
        )
      }
    />
  );
}
