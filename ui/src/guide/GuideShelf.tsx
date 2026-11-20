import { useRef } from "react";
import { apiErrorOf } from "../api/client";
import { useGuide } from "../api/queries";
import { guideHash } from "../app/routes";
import { useTranslate } from "../i18n";
import { EmptyState } from "../ui/EmptyState";
import { ErrorPanel, Skeleton } from "../ui/states";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { useRetryFocus } from "../ui/useRetryFocus";
import { GuideWords } from "./text";

// Documents' second shelf, SpecEngine guide (docs/features/specengine-guide.md "The UI"; ADR-0085):
// the topics of SpecEngine's own guide as the daemon's build embedded it, in the guide's order, each its
// title, its one-line summary and its rule count, the title opening the topic's page. Read only: the
// guide is the build's, never the project's, so nothing here proposes, edits or files a task, and its
// read takes no project and no turn. Its text is English, shown as written; only the chrome is
// translated. A failure, an older daemon's 404 too, is the daemon's words with Retry, as the library's.

export function GuideShelf({ project }: { project: string }) {
  const t = useTranslate();
  const region = useRef<HTMLDivElement>(null);
  const read = useGuide();
  const failure = useRetainedFailure(read.error, read.isFetching);
  const retried = useRetryFocus(read.data, () => region.current?.querySelector<HTMLElement>(".guide-topic-link"));
  const topics = read.data?.topics;

  let body;
  if (topics === undefined) {
    body = failure === null ? <Skeleton label={t("guide.loading")} lines={6} /> : null;
  } else if (topics.length === 0) {
    body = <EmptyState level={2} title={t("guide.empty")} description={<p>{t("guide.emptyText")}</p>} />;
  } else {
    body = (
      <ol className="guide-topics" aria-label={t("guide.topics", { count: topics.length })}>
        {topics.map((entry) => (
          <li key={entry.topic} className="guide-topic">
            <a className="guide-topic-link" href={guideHash(project, entry.topic)} lang="en">
              <GuideWords text={entry.title} />
            </a>
            <p className="guide-topic-summary" lang="en">
              <GuideWords text={entry.summary} />
            </p>
            <p className="guide-topic-facts">
              <span className="mono">{entry.topic}</span>
              <span>{t("guide.rules", { count: entry.rules })}</span>
            </p>
          </li>
        ))}
      </ol>
    );
  }

  return (
    <div ref={region} className="guide-shelf" aria-busy={read.isFetching}>
      <p className="guide-about">{t("guide.about")}</p>
      {failure !== null && (
        <ErrorPanel
          title={t("guide.failed")}
          message={apiErrorOf(failure).message}
          retrying={read.isFetching}
          attempt={read.errorUpdateCount}
          onRetry={() => {
            retried();
            void read.refetch({ cancelRefetch: false });
          }}
        />
      )}
      {body}
    </div>
  );
}
