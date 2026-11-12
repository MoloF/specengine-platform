import { useRef, type MouseEvent, type ReactNode } from "react";
import { apiErrorOf } from "../api/client";
import type { NodeQuery } from "../api/queries";
import type { ShownLink, ShownLinks, ShownNode } from "../api/types";
import { sectionHash } from "../app/routes";
import { Badge } from "../ui/Badge";
import { EmptyState } from "../ui/EmptyState";
import { Icon } from "../ui/Icon";
import { ErrorPanel, Skeleton } from "../ui/states";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { useRetryFocus } from "../ui/useRetryFocus";
import { linkStateLook, WEAK_LINK_TYPE } from "./labels";
import { placeKey, rowName } from "./rows";
import { useTranslate, type Translate } from "../i18n";
import { fill, slot } from "../i18n/fill";

/** The parts the live rule left out, zero parts omitted, joined; null when nothing was. */
function leftOutParts(leftOut: { generated: number; tier3: number }, t: Translate): string | null {
  const parts: string[] = [];
  if (leftOut.generated > 0) {
    parts.push(t("tree.leftOutGenerated", { count: leftOut.generated }));
  }
  if (leftOut.tier3 > 0) {
    parts.push(t("tree.leftOutArchived", { count: leftOut.tier3 }));
  }
  return parts.length === 0 ? null : parts.join(", ");
}

/** What the live rule left out, zero parts omitted; null when nothing was. */
export function leftOutText(leftOut: { generated: number; tier3: number }, t: Translate): string | null {
  const parts = leftOutParts(leftOut, t);
  return parts === null ? null : t("tree.leftOut", { parts });
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
  const t = useTranslate();
  const shown = link.name ?? link.written;
  const asWritten = writtenAs(link, link.at ?? rowName(holder));
  const inText = outgoing && link.path === holder.path && link.line >= holder.line && link.line <= holder.end_line;
  return (
    <li className="link-row">
      <p className="link-row-head">
        <span className="kind-tag">{link.type}</span>
        {link.type === WEAK_LINK_TYPE && <span className="weak-tag">{t("tree.links.weak")}</span>}
        {link.state === "resolved" && link.name !== null ? (
          <a className="mono link-name" href={sectionHash(project, "tree", link.name)} onClick={onFollow}>
            {link.name}
          </a>
        ) : (
          <span className="mono link-name">{shown}</span>
        )}
        <Badge name={t("tree.links.state")} look={linkStateLook(link.state, t)} />
      </p>
      <p className="link-row-meta">
        <span className="mono">
          {link.path}:{link.line}
        </span>
        {link.at !== null && <span>{fill(t("tree.links.at", { at: slot("at") }), { at: <span className="mono">{link.at}</span> })}</span>}
        {asWritten !== null && <span>{fill(t("tree.links.as", { written: slot("written") }), { written: <span className="mono">{asWritten}</span> })}</span>}
        {inText && (
          <button
            type="button"
            className="link-button"
            onClick={() => {
              onShowInText(placeKey(holder), link.line);
            }}
          >
            {fill(t("tree.links.showInText", { line: slot("line") }), { line: <span className="sr-only">{t("tree.links.showInTextLine", { line: link.line })}</span> })}
          </button>
        )}
      </p>
      {link.reason !== null && (
        <p className="link-reason verbatim" lang="en">
          {link.reason}
        </p>
      )}
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
  const t = useTranslate();
  const parts = leftOutParts(links.left_out, t);
  // One holder: its lists sit under the node's h1; several: under each holder's h2.
  const common = { project, holder, onShowInText, onFollow, level: many ? 3 : 2 } as const;
  const none = links.outgoing.length === 0 && links.incoming.length === 0;
  return (
    <section className="holder-links" aria-label={many ? t("tree.links.holder", { place: `${holder.path}:${String(holder.line)}` }) : undefined}>
      {many && (
        <h2 className="holder-text-title mono">
          {holder.path}:{holder.line}-{holder.end_line}
        </h2>
      )}
      {none ? (
        // One level below its part's heading: the holder's h2 when several answer, else the node's h1.
        <EmptyState level={many ? 3 : 2} title={t("tree.links.none")} description={<p>{archive ? t("tree.links.next") : t("tree.links.nextArchive")}</p>} />
      ) : (
        <>
          <LinkList title={t("tree.links.outgoing")} links={links.outgoing} empty={t("tree.links.noneOut")} outgoing {...common} />
          <LinkList title={t("tree.links.incoming")} links={links.incoming} empty={t("tree.links.noneIn")} outgoing={false} {...common} />
        </>
      )}
      {parts !== null && (
        <p className="note">
          <Icon name="info" />
          <span>{links.left_out.tier3 > 0 && !archive ? t("tree.links.leftOutArchive", { parts }) : t("tree.leftOut", { parts })}</span>
        </p>
      )}
      {links.omitted > 0 && <p className="note">{t("tree.links.omitted", { count: links.omitted })}</p>}
    </section>
  );
}

/**
 * The Links tab: outgoing then incoming per holder, in the daemon's order. The read is the node
 * pane's (`nodes?with=links`, started with the node), shared with the rendered text's anchors.
 */
export function LinksPanel({
  project,
  nodeRef,
  archive,
  query,
  onShowInText,
  onFollow,
}: {
  project: string;
  nodeRef: string;
  archive: boolean;
  /** The node's links read. */
  query: NodeQuery;
  onShowInText: (place: string, line: number) => void;
  onFollow: (event: MouseEvent<HTMLAnchorElement>) => void;
}) {
  const t = useTranslate();
  const failure = useRetainedFailure(query.error, query.isFetching);
  const region = useRef<HTMLDivElement>(null);
  const retried = useRetryFocus(query.data, () => region.current);

  if (query.data === undefined) {
    return failure === null ? (
      <div aria-busy="true">
        <Skeleton label={t("tree.links.loading", { ref: nodeRef })} lines={4} />
      </div>
    ) : (
      <ErrorPanel
        title={t("tree.links.failed", { ref: nodeRef })}
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
      {view.reason !== null && (
        <p className="verbatim" lang="en">
          {view.reason}
        </p>
      )}
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
          {fill(t("tree.links.failedAgain", { message: slot("message") }), { message: <span lang="en">{apiErrorOf(query.error).message}</span> })}
        </p>
      )}
    </div>
  );
}
