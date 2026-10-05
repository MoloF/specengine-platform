import { useRef, type MouseEvent, type ReactNode } from "react";
import { apiErrorOf } from "../api/client";
import { useNode } from "../api/queries";
import type { ShownLink, ShownLinks, ShownNode } from "../api/types";
import { sectionHash } from "../app/routes";
import { Badge } from "../ui/Badge";
import { Icon } from "../ui/Icon";
import { ErrorPanel, Skeleton } from "../ui/states";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { useRetryFocus } from "../ui/useRetryFocus";
import { linkStateLook, WEAK_LINK_TYPE } from "./labels";
import { placeKey, rowName } from "./rows";

/** What the live rule left out, zero parts omitted; null when nothing was. */
export function leftOutText(leftOut: { generated: number; tier3: number }): string | null {
  const parts = [
    leftOut.generated > 0 ? `${String(leftOut.generated)} generated` : null,
    leftOut.tier3 > 0 ? `${String(leftOut.tier3)} archived` : null,
  ].filter((part): part is string => part !== null);
  return parts.length === 0 ? null : `Left out: ${parts.join(", ")}`;
}

/**
 * The written form when it is not the name of the node it names, as `spec show --links` prints
 * "as <written>" (`docs/canon/spec-cli-graph.md` "spec show --links"; `links.rs` `named`): an
 * outgoing link's written form names its target, `name`; an incoming one's names the node it lands
 * on here, `here` (`at`, else the holder). `status: superseded-by X` names its source instead, the
 * other end, and the JSON says not which end: a written form that is either end's name is no
 * "as". Null when unresolved: the written form is then the name shown.
 */
export function writtenAs(link: ShownLink, here: string): string | null {
  if (link.name === null || link.written === link.name || link.written === here) {
    return null;
  }
  return link.written;
}

/** A section heading at `level`: a holder's own heading is h2 when several holders share the tab. */
function SectionTitle({ level, className, children }: { level: 2 | 3; className: string; children: ReactNode }) {
  return level === 2 ? <h2 className={className}>{children}</h2> : <h3 className={className}>{children}</h3>;
}

function LinkRow({
  project,
  link,
  holder,
  outgoing,
  onShowInText,
  onFollow,
}: {
  project: string;
  link: ShownLink;
  holder: ShownNode;
  outgoing: boolean;
  onShowInText: (place: string, line: number) => void;
  onFollow: (event: MouseEvent<HTMLAnchorElement>) => void;
}) {
  const shown = link.name ?? link.written;
  const asWritten = writtenAs(link, link.at ?? rowName(holder));
  const inText = outgoing && link.path === holder.path && link.line >= holder.line && link.line <= holder.end_line;
  return (
    <li className="link-row">
      <p className="link-row-head">
        <span className="kind-tag">{link.type}</span>
        {link.type === WEAK_LINK_TYPE && <span className="weak-tag">weak</span>}
        {link.state === "resolved" && link.name !== null ? (
          <a className="mono link-name" href={sectionHash(project, "tree", link.name)} onClick={onFollow}>
            {link.name}
          </a>
        ) : (
          <span className="mono link-name">{shown}</span>
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
        {inText && (
          <button
            type="button"
            className="link-button"
            onClick={() => {
              onShowInText(placeKey(holder), link.line);
            }}
          >
            Show in text<span className="sr-only">, line {link.line}</span>
          </button>
        )}
      </p>
      {link.reason !== null && <p className="link-reason verbatim">{link.reason}</p>}
    </li>
  );
}

function LinkList({
  title,
  level,
  links,
  empty,
  ...row
}: {
  title: string;
  level: 2 | 3;
  links: readonly ShownLink[];
  empty: string;
  project: string;
  holder: ShownNode;
  outgoing: boolean;
  onShowInText: (place: string, line: number) => void;
  onFollow: (event: MouseEvent<HTMLAnchorElement>) => void;
}) {
  return (
    <section className="link-group">
      <SectionTitle level={level} className="card-section-title">
        {title} <span className="count-tag">{links.length}</span>
      </SectionTitle>
      {links.length === 0 ? (
        <p className="muted">{empty}</p>
      ) : (
        <ul className="link-list">
          {links.map((link, index) => (
            <LinkRow key={`${String(index)}-${link.path}:${String(link.line)}`} link={link} {...row} />
          ))}
        </ul>
      )}
    </section>
  );
}

function HolderLinks({
  project,
  holder,
  links,
  archive,
  many,
  onShowInText,
  onFollow,
}: {
  project: string;
  holder: ShownNode;
  links: ShownLinks;
  archive: boolean;
  many: boolean;
  onShowInText: (place: string, line: number) => void;
  onFollow: (event: MouseEvent<HTMLAnchorElement>) => void;
}) {
  const leftOut = leftOutText(links.left_out);
  // One holder: its lists sit under the node's h1; several: under each holder's h2.
  const common = { project, holder, onShowInText, onFollow, level: many ? 3 : 2 } as const;
  const none = links.outgoing.length === 0 && links.incoming.length === 0;
  return (
    <section className="holder-links" aria-label={many ? `Links at ${holder.path}:${String(holder.line)}` : undefined}>
      {many && (
        <h2 className="holder-text-title mono">
          {holder.path}:{holder.line}-{holder.end_line}
        </h2>
      )}
      {none ? (
        <div className="empty-state">
          <p>No link is written in this node, and none lands on it.</p>
          <p className="muted">
            Next: links come from front-matter fields and references in the text.
            {!archive && " Include archive adds the links written in archived documents."}
          </p>
        </div>
      ) : (
        <>
          <LinkList title="Outgoing" links={links.outgoing} empty="None written in this node." outgoing {...common} />
          <LinkList title="Incoming" links={links.incoming} empty="None lands on this node." outgoing={false} {...common} />
        </>
      )}
      {leftOut !== null && (
        <p className="note">
          <Icon name="info" />
          <span>
            {leftOut}
            {links.left_out.tier3 > 0 && !archive && ". Include archive to list the archived ones."}
          </span>
        </p>
      )}
      {links.omitted > 0 && <p className="note">{links.omitted} links not shown</p>}
    </section>
  );
}

/** The Links tab: outgoing then incoming per holder, in the daemon's order; read on first open. */
export function LinksPanel({
  project,
  nodeRef,
  archive,
  onShowInText,
  onFollow,
}: {
  project: string;
  nodeRef: string;
  archive: boolean;
  onShowInText: (place: string, line: number) => void;
  onFollow: (event: MouseEvent<HTMLAnchorElement>) => void;
}) {
  const query = useNode(project, nodeRef, { with: ["links"], archive });
  const failure = useRetainedFailure(query.error, query.isFetching);
  const region = useRef<HTMLDivElement>(null);
  const retried = useRetryFocus(query.data, () => region.current);

  if (query.data === undefined) {
    return failure === null ? (
      <div aria-busy="true">
        <Skeleton label={`Loading the links of ${nodeRef}`} lines={4} />
      </div>
    ) : (
      <ErrorPanel
        title={`The links of ${nodeRef} could not be read`}
        message={apiErrorOf(failure).message}
        retrying={query.isFetching}
        attempt={query.errorUpdateCount}
        onRetry={() => {
          retried();
          void query.refetch({ cancelRefetch: false });
        }}
      />
    );
  }
  const view = query.data;
  return (
    <div ref={region} tabIndex={-1} className="links-panel" aria-busy={query.isFetching}>
      {view.reason !== null && <p className="verbatim">{view.reason}</p>}
      {view.nodes.map((holder) =>
        holder.links === null ? null : (
          <HolderLinks
            key={placeKey(holder)}
            project={project}
            holder={holder}
            links={holder.links}
            archive={archive}
            many={view.nodes.length > 1}
            onShowInText={onShowInText}
            onFollow={onFollow}
          />
        ),
      )}
      {query.error !== null && (
        <p className="note" role="alert">
          The links could not be read again: {apiErrorOf(query.error).message}
        </p>
      )}
    </div>
  );
}
