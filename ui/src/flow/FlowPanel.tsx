import type { MouseEvent, ReactNode, Ref } from "react";
import { apiErrorOf } from "../api/client";
import type { NodeQuery } from "../api/queries";
import { useNode } from "../api/queries";
import type { FlowEdge, FlowNode, TraceCounts } from "../api/types";
import { openDocFrom, replaceHash } from "../app/location";
import { flowHash, type FlowPlace } from "../app/routes";
import { nodeName } from "../graph/layout";
import { useTranslate } from "../i18n";
import { Prose } from "../markdown/Prose";
import { splitFrontMatter } from "../markdown/source";
import { TraceBar } from "../trace/TraceBar";
import { Button } from "../ui/Button";
import { Disclosure } from "../ui/Disclosure";
import { Icon } from "../ui/Icon";
import { Lozenge } from "../ui/Lozenge";
import { ErrorPanel, Skeleton } from "../ui/states";
import { isPlainClick } from "./clicks";
import { countsText, gapMark, holdsText, kindWords, rollupLozenge, stageWords, startWord } from "./labels";
import { isGroup } from "./layout";

// The panel (docs/features/writer-studio.md D7; docs/features/doc-modal.md "Acceptance criteria",
// DM-14): always mounted, at its own width, scrolling inside, and minimal. With nothing selected it
// shows the scope; a box its title, stage, work state and marks, its text's first paragraph and Open
// document, the whole of it in the document modal; an arrow its served words. IDs, raw kinds,
// `path:line`, distance and inner counts stand folded under Technical details. The gap rows are the
// bar's. Its work state and marks are the drawing read's: the panel reads no trace. The spec is read
// only here; a card is written, a queue row.

/** A link that selects: one history entry replaced, never pushed (D5). */
function SelectLink({ project, place, className, children }: { project: string; place: FlowPlace; className?: string; children: ReactNode }) {
  return (
    <a
      className={className}
      href={flowHash(project, place)}
      onClick={(event: MouseEvent<HTMLAnchorElement>) => {
        if (isPlainClick(event)) {
          event.preventDefault();
          replaceHash(event.currentTarget.getAttribute("href") ?? "");
        }
      }}
    >
      {children}
    </a>
  );
}

/** Back to the scope: the selection cleared, as Esc clears it. */
function BackTo({ name, onBack }: { name: string; onBack: () => void }) {
  const t = useTranslate();
  return (
    <p className="flow-panel-back">
      <button type="button" className="link-button" onClick={onBack}>
        <Icon name="back" />
        <span>{t("flow.backTo", { name })}</span>
      </button>
    </p>
  );
}

/** A document's lead: its text's first paragraph from its `with=links` read, anchors as document links; none, nothing. */
function Lead({ project, name, read }: { project: string; name: string; read: NodeQuery }) {
  const t = useTranslate();
  const holder = read.data?.nodes[0];
  const text = holder === undefined ? null : splitFrontMatter(holder.text);
  return (
    <>
      {read.data === undefined && read.error === null && <Skeleton label={t("flow.nodeReading", { name })} lines={2} />}
      {read.data === undefined && read.error !== null && (
        <ErrorPanel
          title={t("flow.nodeFailed", { name })}
          message={apiErrorOf(read.error).message}
          retrying={read.isFetching}
          attempt={read.errorUpdateCount}
          onRetry={() => {
            void read.refetch({ cancelRefetch: false });
          }}
        />
      )}
      {read.data !== undefined && read.data.reason !== null && (
        <p className="verbatim" lang="en">
          {read.data.reason}
        </p>
      )}
      {holder !== undefined && text !== null && (
        <Prose
          className="flow-panel-lead"
          source={text.body}
          links={holder.links?.outgoing ?? null}
          project={project}
          baseLevel={3}
          frontMatter={false}
          firstLine={holder.line + text.lines}
          path={holder.path}
          firstParagraph
        />
      )}
    </>
  );
}

/** A served count line and its bar: no read of them, no line. */
function Counts({ counts }: { counts: TraceCounts | null }) {
  const t = useTranslate();
  if (counts === null) {
    return null;
  }
  return (
    <div className="flow-panel-counts">
      <p>{countsText(counts, t)}</p>
      <TraceBar counts={counts} />
    </div>
  );
}

/** Open document, the primary, and New card: a document's ways on from the panel. */
function Acts({ name, onNewCard, children }: { name: string; onNewCard: (name: string, trigger: HTMLElement) => void; children?: ReactNode }) {
  const t = useTranslate();
  return (
    <p className="flow-panel-actions">
      <Button
        appearance="primary"
        icon="document"
        aria-haspopup="dialog"
        onClick={(event) => {
          openDocFrom(name, event.currentTarget);
        }}
      >
        {t("flow.openDoc")}
      </Button>
      <Button
        icon="cardNew"
        aria-haspopup="dialog"
        onClick={(event) => {
          onNewCard(name, event.currentTarget);
        }}
      >
        {t("flow.newCard")}
      </Button>
      {children}
    </p>
  );
}

/** What the scope says with nothing selected. */
export interface ScopeShown {
  /** Null: the unrooted top. */
  name: string | null;
  title: string;
  kind: string | null;
  /** Its `path:line`, from the read that holds it; null while none does. */
  where: string | null;
  counts: TraceCounts | null;
  /** Its `with=links` read; null at the top. */
  read: NodeQuery | null;
}

/** The facts a person rarely needs, folded: a disclosure, collapsed (flow-scenario D12). */
function TechnicalDetails({ children, more }: { children: ReactNode; more?: ReactNode }) {
  const t = useTranslate();
  return (
    <Disclosure summary={t("ui.technicalDetails")} className="flow-panel-details">
      <dl className="facts">{children}</dl>
      {more}
    </Disclosure>
  );
}

/** The scope: its title, counts and lead, Open document and New card, then what the caller puts under; its name, kind and place folded. An unrooted top has none of these. */
export function ScopeView({
  project,
  scope,
  headingRef,
  onNewCard,
  children,
}: {
  project: string;
  scope: ScopeShown;
  headingRef: Ref<HTMLHeadingElement>;
  onNewCard: (name: string, trigger: HTMLElement) => void;
  children: ReactNode;
}) {
  const t = useTranslate();
  return (
    <>
      <header className="flow-panel-head">
        <h2 ref={headingRef} tabIndex={-1} className="flow-panel-title">
          <bdi>{scope.title}</bdi>
        </h2>
      </header>
      <Counts counts={scope.counts} />
      {scope.name !== null && scope.read !== null && <Lead project={project} name={scope.name} read={scope.read} />}
      {scope.name !== null && <Acts name={scope.name} onNewCard={onNewCard} />}
      {children}
      {scope.name !== null && (
        <TechnicalDetails>
          <div className="fact fact-wide">
            <dt>{t("flow.fact.name")}</dt>
            <dd className="mono">{scope.name}</dd>
          </div>
          <div className="fact">
            <dt>{t("flow.fact.kind")}</dt>
            <dd className="mono">{scope.kind ?? "-"}</dd>
          </div>
          <div className="fact fact-wide">
            <dt>{t("flow.fact.where")}</dt>
            <dd className="mono">{scope.where ?? "-"}</dd>
          </div>
        </TechnicalDetails>
      )}
    </>
  );
}

/** A box: its title; a line of its stage, its work state and the marks the canvas gives it; its lead; Open document, New card, a group's Open; its group by title; its name, kind, place, distance, start, end and inner links folded. */
export function BoxView({
  project,
  node,
  inLane,
  scopeName,
  scopeTitle,
  titleOf,
  named,
  headingRef,
  enterPlace,
  onBack,
  onNewCard,
  children,
}: {
  project: string;
  node: FlowNode;
  inLane: boolean;
  /** The scope drawn: its own name is no group to show. */
  scopeName: string | null;
  scopeTitle: string;
  /** A name's title where a read gives one, else the name. */
  titleOf: (name: string) => string;
  /** The names the read's gap rows name: the box's gap mark, as on the canvas. */
  named: ReadonlySet<string>;
  headingRef: Ref<HTMLHeadingElement> | undefined;
  /** The place entering a group. */
  enterPlace: (name: string) => FlowPlace;
  onBack: () => void;
  onNewCard: (name: string, trigger: HTMLElement) => void;
  /** Under the acts: the note of a card made from this box. */
  children?: ReactNode;
}) {
  const t = useTranslate();
  const name = nodeName(node);
  const title = node.title ?? name;
  // The document's read, the modal's key: Open document draws from it at once.
  const read = useNode(project, name, { with: ["links"] });
  const gap = gapMark(node, named, t);
  const state = node.rollup === null ? null : rollupLozenge(node.rollup, t);
  return (
    <>
      <BackTo name={scopeTitle} onBack={onBack} />
      <header className="flow-panel-head">
        <h2 ref={headingRef} tabIndex={-1} className="flow-panel-title">
          <bdi>{title}</bdi>
        </h2>
      </header>
      <p className="flow-panel-line">
        <span className="flow-panel-stage">
          <span className="sr-only">{t("flow.factLabel", { fact: t("flow.fact.stage") })}</span>
          <bdi>{stageWords(node.stage, t)}</bdi>
        </span>
        {state !== null && <Lozenge appearance={state.appearance} icon={state.icon} label={state.label} lang={state.served ? "en" : undefined} />}
        {gap !== null && (
          <span className="flow-mark flow-mark-gap">
            <Icon name="flowGap" />
            {gap.shown}
          </span>
        )}
        {isGroup(node) && <span className="flow-mark">{holdsText(node.holds, t)}</span>}
      </p>
      {isGroup(node) && <Counts counts={node.counts} />}
      <Lead project={project} name={name} read={read} />
      <Acts name={name} onNewCard={onNewCard}>
        {isGroup(node) && (
          <a className="button" href={flowHash(project, enterPlace(name))}>
            <Icon name="chevronRight" />
            <span>{t("flow.openGroup", { name: title })}</span>
          </a>
        )}
      </Acts>
      {children}
      {node.group !== null && node.group !== scopeName && (
        <p className="flow-panel-group">
          <span>{t("flow.factLabel", { fact: t("flow.fact.group") })}</span>
          <a href={flowHash(project, enterPlace(node.group))}>
            <bdi>{titleOf(node.group)}</bdi>
          </a>
        </p>
      )}
      <TechnicalDetails
        more={
          (node.start || node.end) && (
            <p className="flow-panel-marks">
              {node.start && (
                <span className="flow-mark flow-mark-start">
                  <Icon name="flowStart" />
                  {startWord(inLane, t)}
                </span>
              )}
              {node.end && (
                <span className="flow-mark flow-mark-end">
                  <Icon name="flowEnd" />
                  {t("flow.end")}
                </span>
              )}
            </p>
          )
        }
      >
        <div className="fact fact-wide">
          <dt>{t("flow.fact.name")}</dt>
          <dd className="mono">{node.id ?? "-"}</dd>
        </div>
        <div className="fact">
          <dt>{t("flow.fact.kind")}</dt>
          <dd className="mono">{node.kind ?? "-"}</dd>
        </div>
        <div className="fact">
          <dt>{t("flow.fact.distance")}</dt>
          <dd>{node.distance ?? t("flow.offWalk")}</dd>
        </div>
        <div className="fact fact-wide">
          <dt>{t("flow.fact.where")}</dt>
          <dd className="mono">
            {node.path}:{node.line}
          </dd>
        </div>
        {node.inner.length > 0 && (
          <div className="fact fact-wide">
            <dt>{t("flow.groupHeading")}</dt>
            <dd>
              <ul className="flow-panel-inner">
                {node.inner.map((inner) => (
                  <li key={inner.type}>
                    <span className="mono">{inner.type}</span> {t("flow.within", { count: inner.count })}
                  </li>
                ))}
              </ul>
            </dd>
          </div>
        )}
      </TechnicalDetails>
    </>
  );
}

/** An end of an arrow: a link selecting it; a group adds Open. */
function EdgeEnd({ project, name, group, select, enter }: { project: string; name: string; group: boolean; select: FlowPlace | null; enter: FlowPlace }) {
  const t = useTranslate();
  return (
    <>
      {select === null ? (
        <span className="mono">{name}</span>
      ) : (
        <SelectLink className="mono" project={project} place={select}>
          {name}
        </SelectLink>
      )}
      {group && (
        <>
          {" "}
          <a href={flowHash(project, enter)}>{t("flow.openGroup", { name })}</a>
        </>
      )}
    </>
  );
}

/** An arrow: its ends, type and kind raw, when, how many links it stands for, and where it is first written. */
export function EdgeView({
  project,
  edge,
  scopeTitle,
  headingRef,
  isGroupName,
  selectPlace,
  enterPlace,
  onBack,
}: {
  project: string;
  edge: FlowEdge;
  scopeTitle: string;
  headingRef: Ref<HTMLHeadingElement>;
  isGroupName: (name: string) => boolean;
  /** The place selecting an end; null for one that is no box here (the scope's own node). */
  selectPlace: (name: string) => FlowPlace | null;
  enterPlace: (name: string) => FlowPlace;
  onBack: () => void;
}) {
  const t = useTranslate();
  return (
    <>
      <BackTo name={scopeTitle} onBack={onBack} />
      <header className="flow-panel-head">
        <h2 ref={headingRef} tabIndex={-1} className="flow-panel-title">
          {t("flow.edgeHeading")}
        </h2>
      </header>
      <dl className="facts">
        <div className="fact fact-wide">
          <dt>{t("flow.edge.from")}</dt>
          <dd>
            <EdgeEnd project={project} name={edge.from} group={isGroupName(edge.from)} select={selectPlace(edge.from)} enter={enterPlace(edge.from)} />
          </dd>
        </div>
        <div className="fact fact-wide">
          <dt>{t("flow.edge.to")}</dt>
          <dd>
            <EdgeEnd project={project} name={edge.to} group={isGroupName(edge.to)} select={selectPlace(edge.to)} enter={enterPlace(edge.to)} />
          </dd>
        </div>
        <div className="fact">
          <dt>{t("flow.edge.type")}</dt>
          <dd className="mono">{edge.type}</dd>
        </div>
        <div className="fact">
          <dt>{t("flow.edge.kind")}</dt>
          <dd>{edge.kind === null ? t("flow.edge.none") : kindWords(edge.kind, t)}</dd>
        </div>
        <div className="fact fact-wide">
          <dt>{t("flow.edge.when")}</dt>
          <dd>{edge.when === null ? t("flow.edge.none") : <bdi>{edge.when}</bdi>}</dd>
        </div>
        <div className="fact">
          <dt>{t("flow.edge.links")}</dt>
          <dd>{edge.count}</dd>
        </div>
        <div className="fact fact-wide">
          <dt>{t("flow.fact.where")}</dt>
          <dd className="mono">
            {edge.path}:{edge.line}
          </dd>
        </div>
      </dl>
    </>
  );
}
