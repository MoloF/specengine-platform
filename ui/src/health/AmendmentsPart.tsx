import { useRef, useState } from "react";
import { apiErrorOf } from "../api/client";
import type { HealthQuery } from "../api/queries";
import type { HealthReport, ShownLink, UnappliedAmendment } from "../api/types";
import { RegionBoundary } from "../app/RegionBoundary";
import { DocLink } from "../doc/DocLink";
import { Notes } from "../ui/served";
import { linkStateLook } from "../tree/labels";
import { writtenAs } from "../tree/LinksPanel";
import { Badge } from "../ui/Badge";
import { Button } from "../ui/Button";
import { ReadFailure, Skeleton } from "../ui/states";
import { useFocusLater } from "../ui/useFocusLater";
import { useRetryFocus } from "../ui/useRetryFocus";
import { SHOWN_OPEN } from "./findings";
import { useTranslate } from "../i18n";
import { around, fill, slot } from "../i18n/fill";

/** How many unapplied amendments are rendered until "Show all": as many as a Findings group shows open. */
export const AMENDMENTS_SHOWN = SHOWN_OPEN;

/**
 * One amends link as `spec show --links` lists it: the node it names linked in the tree when it
 * resolved to one (`name`), else what is written, as text; its state's label and icon, where it is
 * written, and the reason verbatim. Nothing here reads the link's type or state but to show them.
 */
function AmendedLink({ project, link, here }: { project: string; link: ShownLink; here: string }) {
  const t = useTranslate();
  const asWritten = writtenAs(link, link.at ?? here);
  return (
    <li className="health-amendment-link">
      <p className="link-row-head">
        <span className="kind-tag">{link.type}</span>
        {link.name === null ? (
          <span className="mono link-name">{link.written}</span>
        ) : (
          <DocLink className="mono link-name" project={project} docRef={link.name}>
            {link.name}
          </DocLink>
        )}
        <Badge name={t("health.amendments.state")} look={linkStateLook(link.state, t)} />
      </p>
      <p className="link-row-meta">
        <span className="mono">
          {link.path}:{link.line}
        </span>
        {link.at !== null && <span>{fill(t("tree.links.at", { at: slot("at") }), { at: <span className="mono">{link.at}</span> })}</span>}
        {asWritten !== null && <span>{fill(t("tree.links.as", { written: slot("written") }), { written: <span className="mono">{asWritten}</span> })}</span>}
      </p>
      {link.reason !== null && (
        <p className="link-reason verbatim" lang="en">
          {link.reason}
        </p>
      )}
    </li>
  );
}

/** One live document that amends a node: its path linked to it in the tree, its title, ID, class and status raw, then its links. */
function AmendmentRow({ project, amendment }: { project: string; amendment: UnappliedAmendment }) {
  const t = useTranslate();
  const facts = amendment.id !== null || amendment.class !== null || amendment.status !== null;
  return (
    <li className="health-left-row health-amendment" data-amendment={amendment.path}>
      <p className="health-amendment-head">
        <DocLink className="health-amendment-path" project={project} docRef={amendment.path}>
          <span className="mono">{amendment.path}</span>
        </DocLink>
        {amendment.title !== null && <span className="health-amendment-title">{amendment.title}</span>}
      </p>
      {facts && (
        <dl className="facts health-amendment-facts">
          {amendment.id !== null && (
            <div className="fact">
              <dt>{t("health.amendments.id")}</dt>
              <dd className="mono">{amendment.id}</dd>
            </div>
          )}
          {amendment.class !== null && (
            <div className="fact">
              <dt>{t("health.amendments.class")}</dt>
              <dd className="mono">{amendment.class}</dd>
            </div>
          )}
          {amendment.status !== null && (
            <div className="fact">
              <dt>{t("health.amendments.status")}</dt>
              <dd>
                <span className="status-tag">{amendment.status}</span>
              </dd>
            </div>
          )}
        </dl>
      )}
      <ul className="health-amendment-links" aria-label={t("health.amendments.linksFrom", { path: amendment.path })}>
        {amendment.links.map((link, index) => (
          <AmendedLink key={`${String(index)}-${link.path}:${String(link.line)}`} project={project} link={link} here={amendment.id ?? amendment.path} />
        ))}
      </ul>
    </li>
  );
}

/**
 * The measured row: how many over how many documents, then the amendments in the daemon's order
 * (by path; never sorted here), the first AMENDMENTS_SHOWN rendered and the rest not until "Show
 * all <n>", which hands the focus to the first row it adds. None: said as the rule, never as
 * nothing left.
 */
function Amendments({ project, report }: { project: string; report: HealthReport }) {
  const t = useTranslate();
  const [all, setAll] = useState(false);
  const list = useRef<HTMLOListElement>(null);
  const focusLater = useFocusLater();
  const notes = <Notes notes={report.notes} label={t("health.amendments.notes")} />;
  const over = t("health.amendments.over", { count: report.documents });
  const count = report.amendments.length;
  if (count === 0) {
    return (
      <>
        {notes}
        <p className="health-left-total health-amendments-none">
          {/* The link type is a word of the catalog's sentence: app code quotes no link type (AC-14 of ui-tree-node). */}
          {fill(t("health.amendments.none", { none: slot("none") }), { none: <span className="health-value health-measured">{t("health.amendments.noneWord")}</span> })}
        </p>
        <p className="health-amendments-scope">{t("health.amendments.measuredOver", { count: report.documents })}</p>
      </>
    );
  }
  const [before, after] = around(t("health.amendments.total", { count, over }), String(count));
  const shown = all ? report.amendments : report.amendments.slice(0, AMENDMENTS_SHOWN);
  const hidden = count - shown.length;
  return (
    <>
      {notes}
      <p className="health-left-total">
        {before}
        <strong className="health-value health-measured">{count}</strong>
        {after}
      </p>
      <ol ref={list} className="health-left-list health-amendment-list" aria-label={t("health.amendments.listLabel")}>
        {shown.map((amendment) => (
          <AmendmentRow key={amendment.path} project={project} amendment={amendment} />
        ))}
      </ol>
      {hidden > 0 && (
        <div className="health-amendments-more">
          <p className="health-amendments-scope">{t("health.amendments.firstOf", { shown: shown.length, count })}</p>
          <Button
            onClick={() => {
              setAll(true);
              focusLater(() => list.current?.querySelectorAll<HTMLElement>(".health-amendment-path")[AMENDMENTS_SHOWN]);
            }}
          >
            {t("health.amendments.showAll", { count })}
          </Button>
        </div>
      )}
    </>
  );
}

/**
 * Row 3 of What is left (06 §7; docs/features/health-left.md "Description and interactions"): the
 * live documents that amend a node, from its own read, `getHealth`, with its own boundary. Three
 * states kept apart: being measured (a skeleton, no digit), could not be measured (the daemon's
 * words verbatim and a Retry reading `getHealth` alone; an older daemon's 404 too, never none), and
 * measured, zero too, a measured value never drawn as row 6's "Not measured" is. `failure` is the
 * read's retained failure and `busy` whether it is being read, both computed by What is left
 * (LeftRegion), which marks itself busy from the same flag.
 */
export function UnappliedAmendments({ project, health, failure, busy }: { project: string; health: HealthQuery; failure: Error | null; busy: boolean }) {
  const t = useTranslate();
  const heading = useRef<HTMLHeadingElement>(null);
  const retried = useRetryFocus(health.data, () => heading.current);

  let body;
  if (health.data === undefined) {
    body =
      failure === null ? (
        <Skeleton label={t("health.amendments.measuring", { project })} lines={3} />
      ) : (
        <ReadFailure
          title={t("health.amendments.failed")}
          failure={failure}
          retrying={health.isFetching}
          attempt={health.errorUpdateCount}
          onRetry={() => {
            retried();
            void health.refetch({ cancelRefetch: false });
          }}
        />
      );
  } else {
    body = (
      <>
        <Amendments project={project} report={health.data} />
        {health.error !== null && !health.isFetching && (
          <p className="note" role="alert">
            {fill(t("health.amendments.failedAgain", { message: slot("message") }), { message: <span lang="en">{apiErrorOf(health.error).message}</span> })}
          </p>
        )}
      </>
    );
  }

  return (
    <div className="health-part health-amendments" data-row="amendments" aria-busy={busy}>
      <h3 className="health-part-title" ref={heading} tabIndex={-1}>
        {t("health.amendments.title")}
      </h3>
      <RegionBoundary name={t("health.amendments.boundary")}>{body}</RegionBoundary>
    </div>
  );
}
