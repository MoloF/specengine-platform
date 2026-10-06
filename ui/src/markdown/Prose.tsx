import { lazy, Suspense } from "react";
import type { MarkdownProps } from "./Markdown";

// What the views import (docs/features/ui-markdown.md "Data"): the renderer arrives in a chunk of
// its own, like the graph canvas. Until it does, the text shows as it was sent, wrapped.

const Markdown = lazy(() => import("./Markdown").then((module) => ({ default: module.Markdown })));

/** Spec prose rendered as markdown (`./Markdown`), the text itself while the renderer loads. */
export function Prose(props: MarkdownProps) {
  const pending = props.className === undefined ? "markdown markdown-pending" : `markdown markdown-pending ${props.className}`;
  return (
    <Suspense
      fallback={
        <div className={pending} aria-busy="true">
          {props.source}
        </div>
      }
    >
      <Markdown {...props} />
    </Suspense>
  );
}

export type { MarkdownProps };
