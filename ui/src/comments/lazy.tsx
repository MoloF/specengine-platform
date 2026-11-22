import { lazy, Suspense } from "react";
import type { Mandate } from "../api/types";
import { useTranslate } from "../i18n";
import { Skeleton } from "../ui/states";

// Where the Comments part stands (docs/features/comments.md "UI"): the card's work item, the document
// view and the proposal pane draw this, and the part arrives in a chunk of its own, never with the
// app's first load; its region, named as the part is, and its heading stand meanwhile, so nothing jumps.

const CommentsPart = lazy(() => import("./CommentsPart").then((module) => ({ default: module.CommentsPart })));

/** What a Comments part is told. */
export interface CommentsProps {
  project: string;
  /** What the comments are under, as the daemon takes `on`: a card's or a proposal's ID, a node's canonical ID, an ID-less document's path. */
  on: string;
  /** What an item turned from a comment is about, prefilled: the node; the proposal's targets; the card's targets. None: no Turn into…. */
  targets: readonly string[];
  /** The part heading's level among its siblings'. */
  level: 2 | 3;
  /** A task's mandate, under a card alone: a decision turned from a comment is framed by it, as the card's own. */
  mandate?: Mandate | null;
}

export function Comments(props: CommentsProps) {
  const t = useTranslate();
  const Heading = props.level === 2 ? "h2" : "h3";
  return (
    <Suspense
      fallback={
        <section className="comments" aria-label={t("app.comments")} aria-busy="true">
          <Heading className="issue-section-title">{t("app.comments")}</Heading>
          <Skeleton label={t("app.lazy.comments")} lines={2} />
        </section>
      }
    >
      <CommentsPart {...props} />
    </Suspense>
  );
}
