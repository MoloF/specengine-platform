import { useRef, useState } from "react";
import { apiErrorOf } from "../api/client";
import type { HealthQuery } from "../api/queries";
import type { HealthReport, ShownLink, UnappliedAmendment } from "../api/types";
import { RegionBoundary } from "../app/RegionBoundary";
import { sectionHash } from "../app/routes";
import { Notes } from "../overview/parts";
import { linkStateLook } from "../tree/labels";
import { writtenAs } from "../tree/LinksPanel";
import { Badge } from "../ui/Badge";
import { ReadFailure, Skeleton } from "../ui/states";
import { useFocusLater } from "../ui/useFocusLater";
import { useRetryFocus } from "../ui/useRetryFocus";
import { SHOWN_OPEN } from "./findings";

/** How many unapplied amendments are rendered until "Show all": as many as a Findings group shows open. */
export const AMENDMENTS_SHOWN = SHOWN_OPEN;

/** "<n> <noun>", the noun in the singular for one. */
function counted(count: number, one: string, many: string): string {
  return `${String(count)} ${count === 1 ? one : many}`;
}

/**
 * One amends link as `spec show --links` lists it: the node it names linked in the tree when it
 * resolved to one (`name`), else what is written, as text; its state's label and icon, where it is
 * written, and the reason verbatim. Nothing here reads the link's type or state but to show them.
 */
function AmendedLink({ project, link, here }: { project: string; link: ShownLink; here: string }) {
  const asWritten = writtenAs(link, link.at ?? here);
  return (
    <li className="health-amendment-link">
      <p className="link-row-head">
        <span className="kind-tag">{link.type}</span>
        {link.name === null ? (
          <span className="mono link-name">{link.written}</span>
        ) : (
          <a className="mono link-name" href={sectionHash(project, "tree", link.name)}>
            {link.name}
          </a>
        )}
        <Badge name="State" look={linkStateLook(link.state)} />
      </p>
      <p className="link-row-meta">
        <span className="mono">
          {link.path}:{link.line}
        </span>
        {link.at !== null && (
          <span>
            at <span className="mono">{link.at}</span>
          </span>
        )}
        {asWritten !== null && (
          <span>
            as <span className="mono">{asWritten}</span>
          </span>
        )}
      </p>
      {link.reason !== null && <p className="link-reason verbatim">{link.reason}</p>}
    </li>
  );
}

/** One live document that amends a node: its path linked to it in the tree, its title, ID, class and status raw, then its links. */
function AmendmentRow({ project, amendment }: { project: string; amendment: UnappliedAmendment }) {
  const facts = amendment.id !== null || amendment.class !== null || amendment.status !== null;
  return (
    <li className="health-left-row health-amendment" data-amendment={amendment.path}>
      <p className="health-amendment-head">
        <a className="health-amendment-path" href={sectionHash(project, "tree", amendment.path)}>
          <span className="mono">{amendment.path}</span>
        </a>
        {amendment.title !== null && <span className="health-amendment-title">{amendment.title}</span>}
      </p>
      {facts && (
        <dl className="facts health-amendment-facts">
          {amendment.id !== null && (
            <div className="fact">
              <dt>ID</dt>
              <dd className="mono">{amendment.id}</dd>
            </div>
          )}
          {amendment.class !== null && (
            <div className="fact">
              <dt>Class</dt>
              <dd className="mono">{amendment.class}</dd>
            </div>
          )}
          {amendment.status !== null && (
            <div className="fact">
              <dt>Status</dt>
              <dd>
                <span className="status-tag">{amendment.status}</span>
              </dd>
            </div>
          )}
        </dl>
      )}
      <ul className="health-amendment-links" aria-label={`Links from ${amendment.path}`}>
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
  const [all, setAll] = useState(false);
  const list = useRef<HTMLOListElement>(null);
  const focusLater = useFocusLater();
  const notes = <Notes notes={report.notes} label="Notes from the daemon on the unapplied amendments" />;
  const over = `over ${counted(report.documents, "document", "documents")}`;
  const count = report.amendments.length;
  if (count === 0) {
    return (
      <>
        {notes}
        <p className="health-left-total health-amendments-none">
          <span className="health-value health-measured">None</span>: no live document of this tree declares an <code>amends</code> link.
        </p>
        <p className="health-amendments-scope">Measured {over}.</p>
      </>
    );
  }
  const shown = all ? report.amendments : report.amendments.slice(0, AMENDMENTS_SHOWN);
  const hidden = count - shown.length;
  return (
    <>
      {notes}
      <p className="health-left-total">
        <strong className="health-value health-measured">{count}</strong> unapplied {count === 1 ? "amendment" : "amendments"} {over}
      </p>
      <ol ref={list} className="health-left-list health-amendment-list" aria-label="Unapplied amendments, by path">
        {shown.map((amendment) => (
          <AmendmentRow key={amendment.path} project={project} amendment={amendment} />
        ))}
      </ol>
      {hidden > 0 && (
        <div className="health-amendments-more">
          <p className="health-amendments-scope">
            The first {shown.length} of {count}, by path.
          </p>
          <button
            type="button"
            className="button"
            onClick={() => {
              setAll(true);
              focusLater(() => list.current?.querySelectorAll<HTMLElement>(".health-amendment-path")[AMENDMENTS_SHOWN]);
            }}
          >
            Show all {count}
          </button>
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
  const heading = useRef<HTMLHeadingElement>(null);
  const retried = useRetryFocus(health.data, () => heading.current);

  let body;
  if (health.data === undefined) {
    body =
      failure === null ? (
        <Skeleton label={`Measuring the unapplied amendments of ${project}`} lines={3} />
      ) : (
        <ReadFailure
          title="The unapplied amendments could not be read"
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
            The unapplied amendments could not be read again: {apiErrorOf(health.error).message}
          </p>
        )}
      </>
    );
  }

  return (
    <div className="health-part health-amendments" data-row="amendments" aria-busy={busy}>
      <h3 className="health-part-title" ref={heading} tabIndex={-1}>
        Unapplied amendments
      </h3>
      <RegionBoundary name="unapplied amendments">{body}</RegionBoundary>
    </div>
  );
}
