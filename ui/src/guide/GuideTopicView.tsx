import { useCallback, useId, useMemo, useRef, type MouseEvent } from "react";
import { apiErrorOf } from "../api/client";
import { useGuideTopic } from "../api/queries";
import { GUIDE_PREFIX, guideHash, guideRefOf, parseHash, sectionHash } from "../app/routes";
import type { DocumentsMemory } from "../documents/DocumentsView";
import { useLanguage, useTranslate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import type { HeadingAnchors } from "../markdown/Markdown";
import { Prose } from "../markdown/Prose";
import { CopyButton } from "../ui/CopyButton";
import { Icon } from "../ui/Icon";
import { ErrorPanel, Skeleton } from "../ui/states";
import { useFocusLater } from "../ui/useFocusLater";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { useRetryFocus } from "../ui/useRetryFocus";
import { GuideWords, plainWords } from "./text";

// A topic of SpecEngine's guide, `#/<p>/documents/guide:<topic>[%23<ID>]`, a chunk of its own
// (docs/features/specengine-guide.md "The UI"; ADR-0085): the page as the daemon's build embedded it,
// read only. Its text is English, rendered through src/markdown/ under `lang="en"`; its rules are
// anchors, each heading the rule's ID as its `id`, with Copy citation beside it (chrome, in the
// viewer's language) copying `guide:<topic>#<ID>`, the form a project cites it by. The route naming a
// rule brings that rule into view, clear of the sticky header, and focuses it once drawn; a link to
// the rule already named (the Rules list, a citation in the text) does so again on a plain click, where
// no hash changes. A rule the page lacks is said so above the page. There is no edit, proposal, task,
// split or Flow act here: the guide is the build's, never a project's. Reached from a card's citation
// the page leaves Documents' shelf as it was; its back link, SpecEngine guide, opens Documents on the
// guide's shelf (reached from the shelf, it already shows it).

/** A click with the primary button and no modifier: one that follows a link in place. */
function isPlainClick(event: MouseEvent<HTMLElement>): boolean {
  return event.button === 0 && !event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey && !event.defaultPrevented;
}

/** The citation a project writes for a rule of a topic. */
function citationOf(topic: string, rule: string): string {
  return `${GUIDE_PREFIX}${topic}#${rule}`;
}

export function GuideTopicView({ project, topic, rule, memory }: { project: string; topic: string; rule: string | null; memory: DocumentsMemory }) {
  const t = useTranslate();
  const { language } = useLanguage();
  const titleId = useId();
  const region = useRef<HTMLElement>(null);
  const focusLater = useFocusLater();
  const read = useGuideTopic(topic);
  const failure = useRetainedFailure(read.error, read.isFetching);
  const retried = useRetryFocus(read.data, () => region.current?.querySelector<HTMLElement>("h1"));
  const page = read.data;

  // The wanted rule's heading, once drawn: brought to the top and focused after the commit, so the
  // Shell's focus on a new view's heading comes first and the rule last.
  const onReach = useCallback(
    (heading: HTMLElement) => {
      focusLater(() => {
        heading.scrollIntoView({ block: "start" });
        return heading;
      });
    },
    [focusLater],
  );
  // Chrome inside the English text: in the viewer's language, said so.
  const tool = useCallback(
    (id: string) => (
      <span className="guide-tool" lang={language}>
        <CopyButton text={citationOf(topic, id)} label={t("guide.copyCitation")} context={citationOf(topic, id)} failed={t("guide.copyFailed")} />
      </span>
    ),
    [topic, t, language],
  );
  const ids = useMemo(() => page?.rules.map((entry) => entry.id) ?? [], [page]);
  const anchors = useMemo<HeadingAnchors>(() => ({ ids, tool, wanted: rule, onReach }), [ids, tool, rule, onReach]);
  const missing = page !== undefined && rule !== null && !ids.includes(rule);

  /** A plain click on a link to the rule the route already names: no hash changes, so the rule is reached here again. */
  function reachAgain(event: MouseEvent<HTMLElement>) {
    const link = event.target instanceof Element ? event.target.closest("a") : null;
    const href = link?.getAttribute("href") ?? null;
    if (rule === null || href === null || !isPlainClick(event)) {
      return;
    }
    const route = parseHash(href);
    const asked = route.type === "section" && route.section === "documents" && route.project === project ? guideRefOf(route.id) : null;
    const heading = asked?.topic === topic && asked.rule === rule ? document.getElementById(rule) : null;
    if (heading !== null && region.current?.contains(heading) === true) {
      onReach(heading);
    }
  }

  /** Back to the shelf: Documents opens on the guide's, as the link says, when it is followed in place. */
  function backToShelf(event: MouseEvent<HTMLElement>) {
    if (!isPlainClick(event)) {
      return;
    }
    const kept = memory.recall(project);
    memory.remember(project, { filters: kept?.filters ?? null, archive: kept?.archive ?? false, sort: kept?.sort ?? null, shelf: "guide" });
  }

  let body;
  if (page === undefined) {
    body = failure === null ? <Skeleton label={t("guide.page.loading", { topic })} lines={8} /> : null;
  } else {
    body = (
      <div className="guide-page" onClick={reachAgain}>
        {page.rules.length > 0 && (
          <nav className="guide-rules" aria-label={t("guide.page.rules")}>
            <h2 className="guide-rules-title">{t("guide.page.rules")}</h2>
            <ol className="guide-rules-list">
              {page.rules.map((entry) => (
                <li key={entry.id}>
                  <a href={guideHash(project, topic, entry.id)} aria-current={entry.id === rule ? "location" : undefined} lang="en">
                    <GuideWords text={entry.title} />
                  </a>
                  <span className="guide-rule-id mono">{entry.id}</span>
                </li>
              ))}
            </ol>
          </nav>
        )}
        <article className="guide-article" lang="en" aria-labelledby={titleId}>
          <Prose source={page.text} links={null} project={project} baseLevel={1} frontMatter={false} dropTitle={plainWords(page.title)} anchors={anchors} className="guide-text" />
        </article>
      </div>
    );
  }

  return (
    <section ref={region} className="view guide-view" aria-labelledby={titleId} aria-busy={read.isFetching}>
      <header className="view-head">
        <a className="guide-back" href={sectionHash(project, "documents")} onClick={backToShelf}>
          <Icon name="back" />
          <span>{t("guide.back")}</span>
        </a>
        <h1 id={titleId} tabIndex={-1} lang="en">
          {page === undefined ? <span className="mono">{topic}</span> : <GuideWords text={page.title} />}
        </h1>
        {page !== undefined && (
          <>
            <p className="view-about" lang="en">
              <GuideWords text={page.summary} />
            </p>
            <p className="guide-facts">
              <span>{t("guide.readOnly")}</span>
              <span>{fill(t("guide.page.reviewed", { date: slot("date") }), { date: <span className="mono">{page.reviewed}</span> })}</span>
              <span>{t("guide.rules", { count: page.rules.length })}</span>
            </p>
          </>
        )}
      </header>
      {failure !== null && (
        <ErrorPanel
          title={t("guide.page.failed", { topic })}
          message={apiErrorOf(failure).message}
          retrying={read.isFetching}
          attempt={read.errorUpdateCount}
          onRetry={() => {
            retried();
            void read.refetch({ cancelRefetch: false });
          }}
        />
      )}
      {missing && <p className="note guide-missing">{fill(t("guide.page.noRule", { id: slot("id") }), { id: <span className="mono">{rule}</span> })}</p>}
      {body}
    </section>
  );
}
