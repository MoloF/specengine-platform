import { useEffect, useId, useRef, useState, type MouseEvent } from "react";
import { apiErrorOf } from "../api/client";
import { useNode, useProjectName, type InboxQuery } from "../api/queries";
import { openDocFrom } from "../app/location";
import { RegionBoundary } from "../app/RegionBoundary";
import { homeHash, sectionHash } from "../app/routes";
import { useTranslate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import { CardsBlock } from "../trace/CardsBlock";
import { Breadcrumbs, type Crumb } from "../ui/Breadcrumbs";
import { Button, buttonClasses } from "../ui/Button";
import { EmptyState } from "../ui/EmptyState";
import { focusIsLost } from "../ui/focus";
import { Icon } from "../ui/Icon";
import { IssueView } from "../ui/IssueView";
import { Lozenge } from "../ui/Lozenge";
import { PropertyList } from "../ui/PropertyList";
import type { TextMode } from "../markdown/TextMode";
import { ErrorPanel, Skeleton } from "../ui/states";
import { TabPanel, Tabs } from "../ui/Tabs";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { useRetryFocus } from "../ui/useRetryFocus";
import { BindingsPanel } from "./BindingsPanel";
import { BundlePanel } from "./BundlePanel";
import { factsOf } from "./Facts";
import { LinksPanel } from "./LinksPanel";
import { proposalsFor } from "./matching";
import { ProposalsPanel } from "./ProposalsPanel";
import { rowName, type TreeRow } from "./rows";
import { TextPanel, type TextTarget } from "./TextPanel";

type TabId = "text" | "links" | "bindings" | "bundle" | "proposals";

/** The forms a REF takes, written as typed: code in every language. */
const REF_FORMS = { slug: "slug/ID", section: "ID#SECTION", md: ".md" } as const;

/** How the pane came to show its node, for where focus goes (docs/features/ui-tree-node.md "Implementation", Keys). */
export type Arrival = "start" | "follow" | "change";

/** The node pane with no node chosen: the view's heading and where to start. */
export function NoNodePane() {
  const t = useTranslate();
  return (
    <article className="node-pane" aria-labelledby="tree-title">
      <header className="node-head">
        <h1 id="tree-title" tabIndex={-1}>
          {t("tree.label")}
        </h1>
        <p className="view-about">{t("tree.node.about")}</p>
      </header>
      <EmptyState
        level={2}
        title={t("tree.node.noneOpen")}
        description={
          <>
            <p>{t("tree.node.pick")}</p>
            <p className="muted">{t("tree.node.keysHint")}</p>
          </>
        }
      />
    </article>
  );
}

/**
 * One node by REF, a work item beside the tree (docs/features/design-sweep.md "Acceptance criteria",
 * DS7): the trail, its heading (the REF when several holders answer it), one holder's status, the
 * acts; the facts beside it (each holder's under its own h2 when several answer); then the daemon's
 * notes, its cards and five tabs. The Text tab shows at once,
 * rendered or as its source; the node's links read starts beside it (the rendered text's anchors,
 * the Links tab), the text never waiting for it. The Bindings and Bundle tabs each read on first
 * open (docs/features/ui-node-bindings.md); Proposals reads the Inbox's query.
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
  const t = useTranslate();
  const projectName = useProjectName(project);
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
          <Skeleton label={t("tree.node.loading", { ref: nodeRef })} lines={6} />
        </div>
      ) : (
        <ErrorPanel
          title={t("tree.node.failed", { ref: nodeRef })}
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
      <EmptyState
        level={2}
        title={t("tree.node.nothing")}
        description={
          <>
            {data.reason !== null && (
              <p className="verbatim" lang="en">
                {data.reason}
              </p>
            )}
            <p>
              {fill(t("tree.node.nothingNext", { slug: slot("slug"), section: slot("section"), md: slot("md") }), {
                slug: <span className="mono">{REF_FORMS.slug}</span>,
                section: <span className="mono">{REF_FORMS.section}</span>,
                md: <span className="mono">{REF_FORMS.md}</span>,
              })}
            </p>
          </>
        }
      />
    );
  } else {
    const count = inbox.data === undefined ? null : proposalsFor(inbox.data.proposals, holders).length;
    body = (
      <>
        <Tabs
          label={t("tree.node.views", { ref: nodeRef })}
          base={base}
          selected={tab}
          onSelect={select}
          tabs={[
            { id: "text", label: t("tree.node.tabText") },
            { id: "links", label: t("tree.node.tabLinks") },
            { id: "bindings", label: t("tree.node.tabBindings") },
            { id: "bundle", label: t("tree.node.tabBundle") },
            {
              id: "proposals",
              label: (
                <>
                  {t("tree.node.tabProposals")}
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
          <RegionBoundary name={t("tree.node.boundary.text")}>
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
            <RegionBoundary name={t("tree.node.boundary.links")}>
              <LinksPanel project={project} nodeRef={nodeRef} archive={archive} query={links} onShowInText={jump} onFollow={onFollow} />
            </RegionBoundary>
          )}
        </TabPanel>
        <TabPanel base={base} id="bindings" selected={tab === "bindings"}>
          {opened.has("bindings") && (
            <RegionBoundary name={t("tree.node.boundary.bindings")}>
              <BindingsPanel project={project} nodeRef={nodeRef} onFollow={onFollow} />
            </RegionBoundary>
          )}
        </TabPanel>
        <TabPanel base={base} id="bundle" selected={tab === "bundle"}>
          {opened.has("bundle") && (
            <RegionBoundary name={t("tree.node.boundary.bundle")}>
              <BundlePanel project={project} nodeRef={nodeRef} onFollow={onFollow} />
            </RegionBoundary>
          )}
        </TabPanel>
        <TabPanel base={base} id="proposals" selected={tab === "proposals"}>
          {opened.has("proposals") && (
            <RegionBoundary name={t("tree.node.boundary.proposals")}>
              <ProposalsPanel project={project} holders={holders} inbox={inbox} />
            </RegionBoundary>
          )}
        </TabPanel>
      </>
    );
  }

  const crumbs: Crumb[] = [
    { key: "project", label: projectName, href: homeHash(project) },
    { key: "tree", label: t("app.sections.tree.label"), href: sectionHash(project, "tree") },
    { key: "ref", label: nodeRef, mono: true },
  ];

  return (
    <article className="node-pane" aria-labelledby={titleId} aria-busy={data === undefined && failure === null}>
      <IssueView
        layout="pane"
        breadcrumbs={<Breadcrumbs items={crumbs} />}
        title={
          <h1 id={titleId} ref={heading} tabIndex={-1} className="node-heading">
            {title}
          </h1>
        }
        meta={
          single === undefined || single.status === null ? null : (
            <span className="node-meta-item">
              <span className="sr-only">{t("tree.node.status")}</span>
              <Lozenge appearance="neutral" icon="document" label={single.status} lang="en" />
            </span>
          )
        }
        actions={
          holders.length === 0 ? null : (
            <div className="node-actions">
              {/* The document view over the tree, its own links there taking the modal (docs/features/doc-modal.md "Acceptance criteria", DM-16). */}
              <Button
                appearance="subtle"
                spacing="compact"
                icon="document"
                aria-haspopup="dialog"
                onClick={(event) => {
                  openDocFrom(nodeRef, event.currentTarget);
                }}
              >
                {t("tree.node.openDoc")}
              </Button>
              <a className={buttonClasses("subtle", "compact")} href={sectionHash(project, "graph", nodeRef)}>
                <Icon name="graph" />
                {t("tree.node.showInGraph")}
              </a>
            </div>
          )
        }
        side={
          single !== undefined ? (
            <PropertyList label={t("tree.node.details")} items={factsOf(single, t)} foldEmpty />
          ) : holders.length > 1 ? (
            <>
              <p className="muted">{t("tree.node.holders", { count: holders.length, ref: nodeRef })}</p>
              <ul className="holder-list">
                {holders.map((holder) => (
                  <li key={`${holder.path}:${String(holder.line)}`} className="holder">
                    <h2 className="holder-heading">
                      <span className="mono">{rowName(holder)}</span>
                      {holder.title !== null && <span className="node-title-text">{holder.title}</span>}
                    </h2>
                    <PropertyList label={t("tree.node.detailsAt", { place: `${holder.path}:${String(holder.line)}` })} items={factsOf(holder, t)} foldEmpty />
                  </li>
                ))}
              </ul>
            </>
          ) : null
        }
        main={
          <>
            {data !== undefined && data.notes.length > 0 && (
              <ul className="note-list" aria-label={t("tree.node.notes")}>
                {data.notes.map((note) => (
                  <li key={note} lang="en">
                    {note}
                  </li>
                ))}
              </ul>
            )}
            {data !== undefined && view.error !== null && (
              <p className="note" role="alert">
                {fill(t("tree.node.failedAgain", { ref: nodeRef, message: slot("message") }), { message: <span lang="en">{apiErrorOf(view.error).message}</span> })}
              </p>
            )}
            {holders.length > 0 && (
              <RegionBoundary name={t("tree.node.boundary.cards")}>
                <CardsBlock key={nodeRef} project={project} nodeRef={nodeRef} section="tree" heading="h2" />
              </RegionBoundary>
            )}
            {body}
            {data !== undefined && holders.length > 0 && (
              <p className="node-foot muted">{fill(t("tree.node.readOnly", { link: slot("link") }), { link: <a href={sectionHash(project, "inbox")}>{t("tree.node.openInbox")}</a> })}</p>
            )}
          </>
        }
      />
    </article>
  );
}
