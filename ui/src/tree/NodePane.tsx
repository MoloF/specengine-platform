import { useEffect, useId, useRef, useState, type MouseEvent } from "react";
import { apiErrorOf } from "../api/client";
import { useNode, type InboxQuery } from "../api/queries";
import type { ShownNode } from "../api/types";
import { RegionBoundary } from "../app/RegionBoundary";
import { sectionHash } from "../app/routes";
import { focusIsLost } from "../ui/focus";
import { Icon } from "../ui/Icon";
import type { TextMode } from "../markdown/TextMode";
import { ErrorPanel, Skeleton } from "../ui/states";
import { TabPanel, Tabs } from "../ui/Tabs";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { useRetryFocus } from "../ui/useRetryFocus";
import { BundlePanel } from "./BundlePanel";
import { LinksPanel } from "./LinksPanel";
import { proposalsFor } from "./matching";
import { ProposalsPanel } from "./ProposalsPanel";
import { rowName, type TreeRow } from "./rows";
import { TextPanel, type TextTarget } from "./TextPanel";

type TabId = "text" | "links" | "bundle" | "proposals";

/** How the pane came to show its node, for where focus goes (docs/features/ui-tree-node.md "Keys"). */
export type Arrival = "start" | "follow" | "change";

/** One holder's header facts: kind, place, status, rev, size, span hash, flags. */
function Facts({ holder }: { holder: ShownNode }) {
  return (
    <dl className="facts node-facts">
      <div className="fact">
        <dt>Kind</dt>
        <dd className="mono">{holder.kind ?? "-"}</dd>
      </div>
      <div className="fact">
        <dt>Where</dt>
        <dd className="mono">
          {holder.path}:{holder.line}-{holder.end_line}
        </dd>
      </div>
      {holder.status !== null && (
        <div className="fact">
          <dt>Status</dt>
          <dd>
            <span className="status-tag">{holder.status}</span>
          </dd>
        </div>
      )}
      {holder.rev !== null && (
        <div className="fact">
          <dt>Rev</dt>
          <dd className="mono">{holder.rev}</dd>
        </div>
      )}
      <div className="fact">
        <dt>Size</dt>
        <dd>{holder.tokens_est} tokens</dd>
      </div>
      <div className="fact fact-wide">
        <dt>Span hash</dt>
        <dd className="mono">{holder.span_hash}</dd>
      </div>
      {(holder.archived || !holder.utf8) && (
        <div className="fact">
          <dt>Flags</dt>
          <dd className="flags">
            {holder.archived && (
              <span className="plain-tag">
                <Icon name="archived" />
                archived
              </span>
            )}
            {!holder.utf8 && <span className="plain-tag">not UTF-8</span>}
          </dd>
        </div>
      )}
    </dl>
  );
}

/** The node pane with no node chosen: the view's heading and where to start. */
export function NoNodePane() {
  return (
    <article className="node-pane" aria-labelledby="tree-title">
      <header className="node-head">
        <h1 id="tree-title" tabIndex={-1}>
          Spec tree
        </h1>
        <p className="view-about">The business-logic tree and each node&rsquo;s text, links, bundle and proposals.</p>
      </header>
      <div className="empty-state">
        <h2>No node open</h2>
        <p>Pick a node in the tree, or search the spec, to read its text as written, its links, the bundle an agent gets and the proposals that name it.</p>
        <p className="muted">In the tree: the arrow keys move, Enter opens, ? lists every key.</p>
      </div>
    </article>
  );
}

/**
 * One node by REF: its heading (the REF when several holders answer it, each holder then under
 * its own h2), the header facts, the daemon's notes and four tabs. The Text tab shows at once,
 * rendered or as its source; the node's links read starts beside it (the rendered text's anchors,
 * the Links tab), the text never waiting for it. The Bundle tab reads on first open; Proposals
 * reads the Inbox's query.
 */
export function NodePane({
  project,
  nodeRef,
  archive,
  rows,
  inbox,
  textMode,
  onTextMode,
  arrival,
  onFollow,
}: {
  project: string;
  nodeRef: string;
  archive: boolean;
  rows: readonly TreeRow[];
  inbox: InboxQuery;
  /** Rendered or Source: the view keeps it from node to node. */
  textMode: TextMode;
  onTextMode: (mode: TextMode) => void;
  /** Called once on mount: how this node was reached. */
  arrival: () => Arrival;
  /** An anchor to another node was clicked: the view decides whether it moves this page. */
  onFollow: (event: MouseEvent<HTMLAnchorElement>) => void;
}) {
  const view = useNode(project, nodeRef);
  // One observer of the links read: the Links tab shows this same read, and adds none.
  const links = useNode(project, nodeRef, { with: ["links"], archive });
  const failure = useRetainedFailure(view.error, view.isFetching);
  const heading = useRef<HTMLHeadingElement>(null);
  const arrived = useRef<Arrival | null>(null);
  const retried = useRetryFocus(view.data, () => heading.current);
  const base = useId();
  const titleId = `${base}-title`;
  const [tab, setTab] = useState<TabId>("text");
  const [opened, setOpened] = useState<ReadonlySet<TabId>>(() => new Set<TabId>(["text"]));
  const [target, setTarget] = useState<TextTarget | null>(null);

  // A followed link lands on this heading; so does Back or Forward when focus went with the old node.
  useEffect(() => {
    arrived.current ??= arrival();
    if (arrived.current === "follow" || (arrived.current === "change" && focusIsLost())) {
      heading.current?.focus();
    }
  }, [arrival]);

  function select(next: TabId) {
    setTab(next);
    setOpened((current) => (current.has(next) ? current : new Set([...current, next])));
  }

  // A new mode mounts the other view: the jump already made is not made again there, so focus stays
  // on the switch that was pressed (WCAG 3.2.2).
  function changeMode(mode: TextMode) {
    setTarget(null);
    onTextMode(mode);
  }

  function jump(place: string, line: number) {
    select("text");
    setTarget((current) => ({ place, line, nonce: (current?.nonce ?? 0) + 1 }));
  }

  const data = view.data;
  const holders = data?.nodes ?? [];
  const single = holders.length === 1 ? holders[0] : undefined;

  let title;
  if (single !== undefined) {
    title = (
      <>
        <span className="node-name mono">{rowName(single)}</span>
        {single.title !== null && <span className="node-title-text">{single.title}</span>}
      </>
    );
  } else {
    title = <span className="node-name mono">{nodeRef}</span>;
  }

  let body;
  if (data === undefined) {
    body =
      failure === null ? (
        <div aria-busy="true">
          <Skeleton label={`Loading ${nodeRef}`} lines={6} />
        </div>
      ) : (
        <ErrorPanel
          title={`${nodeRef} could not be read`}
          message={apiErrorOf(failure).message}
          retrying={view.isFetching}
          attempt={view.errorUpdateCount}
          onRetry={() => {
            retried();
            void view.refetch({ cancelRefetch: false });
          }}
        />
      );
  } else if (holders.length === 0) {
    body = (
      <div className="empty-state">
        <h2>Nothing answers this REF</h2>
        {data.reason !== null && <p className="verbatim">{data.reason}</p>}
        <p>
          Next: search the spec for it, or pick a node in the tree. A REF is an ID, <span className="mono">slug/ID</span>,{" "}
          <span className="mono">ID#SECTION</span> or a root-relative <span className="mono">.md</span> path.
        </p>
      </div>
    );
  } else {
    const count = inbox.data === undefined ? null : proposalsFor(inbox.data.proposals, holders).length;
    body = (
      <>
        <Tabs
          label={`Views of ${nodeRef}`}
          base={base}
          selected={tab}
          onSelect={select}
          tabs={[
            { id: "text", label: "Text" },
            { id: "links", label: "Links" },
            { id: "bundle", label: "Bundle" },
            {
              id: "proposals",
              label: (
                <>
                  Proposals
                  {count !== null && (
                    <>
                      {" "}
                      <span className="tab-count">{count}</span>
                    </>
                  )}
                </>
              ),
            },
          ]}
        />
        <TabPanel base={base} id="text" selected={tab === "text"}>
          <RegionBoundary name="text">
            <TextPanel
              project={project}
              holders={holders}
              rows={rows}
              mode={textMode}
              links={links.data}
              target={target}
              onMode={changeMode}
              onJump={jump}
              onFollow={onFollow}
            />
          </RegionBoundary>
        </TabPanel>
        <TabPanel base={base} id="links" selected={tab === "links"}>
          {opened.has("links") && (
            <RegionBoundary name="links">
              <LinksPanel project={project} nodeRef={nodeRef} archive={archive} query={links} onShowInText={jump} onFollow={onFollow} />
            </RegionBoundary>
          )}
        </TabPanel>
        <TabPanel base={base} id="bundle" selected={tab === "bundle"}>
          {opened.has("bundle") && (
            <RegionBoundary name="bundle">
              <BundlePanel project={project} nodeRef={nodeRef} onFollow={onFollow} />
            </RegionBoundary>
          )}
        </TabPanel>
        <TabPanel base={base} id="proposals" selected={tab === "proposals"}>
          {opened.has("proposals") && (
            <RegionBoundary name="proposals">
              <ProposalsPanel project={project} holders={holders} inbox={inbox} />
            </RegionBoundary>
          )}
        </TabPanel>
      </>
    );
  }

  return (
    <article className="node-pane" aria-labelledby={titleId} aria-busy={data === undefined && failure === null}>
      <header className="node-head">
        <h1 id={titleId} ref={heading} tabIndex={-1} className="node-heading">
          {title}
        </h1>
        {holders.length > 0 && (
          <p className="node-actions">
            <a className="node-action" href={sectionHash(project, "graph", nodeRef)}>
              <Icon name="graph" />
              Show in graph
            </a>
          </p>
        )}
        {single !== undefined && <Facts holder={single} />}
        {holders.length > 1 && (
          <>
            <p className="muted">
              {holders.length} holders: {nodeRef} is declared in more than one place. Each is shown below.
            </p>
            <ul className="holder-list">
              {holders.map((holder) => (
                <li key={`${holder.path}:${String(holder.line)}`} className="holder">
                  <h2 className="holder-heading">
                    <span className="mono">{rowName(holder)}</span>
                    {holder.title !== null && <span className="node-title-text">{holder.title}</span>}
                  </h2>
                  <Facts holder={holder} />
                </li>
              ))}
            </ul>
          </>
        )}
        {data !== undefined && data.notes.length > 0 && (
          <ul className="note-list" aria-label="Notes from the daemon">
            {data.notes.map((note) => (
              <li key={note}>{note}</li>
            ))}
          </ul>
        )}
        {data !== undefined && view.error !== null && (
          <p className="note" role="alert">
            {nodeRef} could not be read again: {apiErrorOf(view.error).message}
          </p>
        )}
      </header>
      {body}
      {data !== undefined && holders.length > 0 && (
        <p className="node-foot muted">
          Read-only: text changes reach the spec only through a proposal you accept. <a href={sectionHash(project, "inbox")}>Open the Inbox</a>
        </p>
      )}
    </article>
  );
}
