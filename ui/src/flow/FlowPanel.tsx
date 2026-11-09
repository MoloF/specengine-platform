import { useState, type MouseEvent, type ReactNode, type Ref } from "react";
import { apiErrorOf } from "../api/client";
import type { NodeQuery } from "../api/queries";
import { useNode } from "../api/queries";
import type { FlowEdge, FlowNode, TraceCounts } from "../api/types";
import { replaceHash } from "../app/location";
import { flowHash, sectionHash, type FlowPlace } from "../app/routes";
import { nodeName } from "../graph/layout";
import { useTranslate } from "../i18n";
import { Prose } from "../markdown/Prose";
import { splitFrontMatter } from "../markdown/source";
import { CardsBlock } from "../trace/CardsBlock";
import { TraceBar } from "../trace/TraceBar";
import { LinksPanel } from "../tree/LinksPanel";
import type { LineTarget } from "../tree/TextView";
import { Icon } from "../ui/Icon";
import { ErrorPanel, Skeleton } from "../ui/states";
import { isPlainClick } from "./clicks";
import { boxWords, countsText, holdsText, stageWords, startWord, withGapsText } from "./labels";
import { isGroup } from "./layout";

// The panel (docs/features/writer-studio.md D7, "Panel"): always mounted, at its own width, scrolling
// inside. With nothing selected it shows the scope, its actions and its gap rows; a box shows its
// words, place, cards, text and links; an arrow its served words. The spec is read only here and
// changes through the AI, as proposals; a card is written, a queue row.

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

/** A node's text and links, from its `with=links` read. */
function NodeText({ project, name, read }: { project: string; name: string; read: NodeQuery }) {
  const t = useTranslate();
  const [target, setTarget] = useState<LineTarget | null>(null);
  const holder = read.data?.nodes[0];
  const text = holder === undefined ? null : splitFrontMatter(holder.text);
  return (
    <>
      {read.data === undefined && read.error === null && <Skeleton label={t("flow.nodeReading", { name })} lines={3} />}
      {read.error !== null && (
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
        <section className="flow-panel-section" aria-label={t("flow.textHeading")}>
          <h3 className="card-section-title">{t("flow.textHeading")}</h3>
          <Prose
            className="flow-panel-text"
            source={text.body}
            links={holder.links?.outgoing ?? null}
            project={project}
            baseLevel={3}
            frontMatter={false}
            firstLine={holder.line + text.lines}
            path={holder.path}
            target={target}
          />
        </section>
      )}
      <section className="flow-panel-section" aria-label={t("flow.linksHeading")}>
        <LinksPanel
          project={project}
          nodeRef={name}
          archive={false}
          query={read}
          onShowInText={(_place, line) => {
            setTarget((last) => ({ line, nonce: (last?.nonce ?? 0) + 1 }));
          }}
          onFollow={() => undefined}
        />
      </section>
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

/** What the scope says with nothing selected. */
export interface ScopeShown {
  /** Null: the unrooted top. */
  name: string | null;
  title: string;
  kind: string | null;
  counts: TraceCounts | null;
  /** Its `with=links` read; null at the top. */
  read: NodeQuery | null;
}

/** The scope: its words, counts, text and links, its ways out, then what the caller puts under (the gap rows). */
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
  /** Opens New card with the scope as its target; null at the top. */
  onNewCard: (() => void) | null;
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
      {scope.name !== null && (
        <dl className="facts">
          <div className="fact fact-wide">
            <dt>{t("flow.fact.name")}</dt>
            <dd className="mono">{scope.name}</dd>
          </div>
          <div className="fact">
            <dt>{t("flow.fact.kind")}</dt>
            <dd className="mono">{scope.kind ?? "-"}</dd>
          </div>
        </dl>
      )}
      <Counts counts={scope.counts} />
      {scope.name !== null && (
        <p className="flow-panel-actions">
          <a className="button" href={sectionHash(project, "tree", scope.name)}>
            {t("flow.openInTree")}
          </a>
          {onNewCard !== null && (
            <button type="button" className="button button-quiet" aria-haspopup="dialog" onClick={onNewCard}>
              <Icon name="cardNew" />
              <span>{t("flow.newCard")}</span>
            </button>
          )}
        </p>
      )}
      {children}
      {scope.name !== null && scope.read !== null && <NodeText project={project} name={scope.name} read={scope.read} />}
      <p className="muted">{t("flow.throughAi")}</p>
    </>
  );
}

/** A box: its words and place, its holder with Open, a group's inner links and Open, its cards, its text and links. */
export function BoxView({
  project,
  node,
  inLane,
  scopeName,
  scopeTitle,
  headingRef,
  enterPlace,
  onBack,
}: {
  project: string;
  node: FlowNode;
  inLane: boolean;
  /** The scope drawn: its own name is no holder to open. */
  scopeName: string | null;
  scopeTitle: string;
  headingRef: Ref<HTMLHeadingElement> | undefined;
  /** The place entering a group. */
  enterPlace: (name: string) => FlowPlace;
  onBack: () => void;
}) {
  const t = useTranslate();
  const name = nodeName(node);
  const read = useNode(project, name, { with: ["links"] });
  return (
    <>
      <BackTo name={scopeTitle} onBack={onBack} />
      <header className="flow-panel-head">
        <h2 ref={headingRef} tabIndex={-1} className="flow-panel-title mono">
          {name}
        </h2>
      </header>
      <dl className="facts">
        <div className="fact fact-wide">
          <dt>{t("flow.fact.title")}</dt>
          <dd>
            <bdi>{boxWords(node)}</bdi>
          </dd>
        </div>
        <div className="fact">
          <dt>{t("flow.fact.kind")}</dt>
          <dd className="mono">{node.kind ?? "-"}</dd>
        </div>
        <div className="fact">
          <dt>{t("flow.fact.distance")}</dt>
          <dd>{node.distance ?? t("flow.offWalk")}</dd>
        </div>
        <div className="fact">
          <dt>{t("flow.fact.stage")}</dt>
          <dd>
            <bdi>{stageWords(node.stage, t)}</bdi>
          </dd>
        </div>
        <div className="fact fact-wide">
          <dt>{t("flow.fact.where")}</dt>
          <dd className="mono">
            {node.path}:{node.line}
          </dd>
        </div>
        {node.group !== null && node.group !== scopeName && (
          <div className="fact fact-wide">
            <dt>{t("flow.fact.group")}</dt>
            <dd>
              <span className="mono">{node.group}</span> <a href={flowHash(project, enterPlace(node.group))}>{t("flow.openGroup", { name: node.group })}</a>
            </dd>
          </div>
        )}
      </dl>
      {(node.start || node.end) && (
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
      )}
      {isGroup(node) && (
        <section className="flow-panel-section" aria-label={t("flow.groupHeading")}>
          <h3 className="card-section-title">{t("flow.groupHeading")}</h3>
          <p>
            {holdsText(node.holds, t)}
            {node.gaps > 0 && `, ${withGapsText(node.gaps, t)}`}
          </p>
          {node.inner.length > 0 && (
            <ul className="flow-panel-inner">
              {node.inner.map((inner) => (
                <li key={inner.type}>
                  <span className="mono">{inner.type}</span> {t("flow.within", { count: inner.count })}
                </li>
              ))}
            </ul>
          )}
          <p>
            <a className="button" href={flowHash(project, enterPlace(name))}>
              <Icon name="chevronRight" />
              <span>{t("flow.openGroup", { name })}</span>
            </a>
          </p>
        </section>
      )}
      <CardsBlock project={project} nodeRef={name} section="flow" heading="h3" />
      <NodeText project={project} name={name} read={read} />
      <p className="flow-panel-actions">
        <a className="button" href={sectionHash(project, "tree", name)}>
          {t("flow.openInTree")}
        </a>
        <a className="button" href={sectionHash(project, "graph", name)}>
          <Icon name="graph" />
          <span>{t("flow.openInGraph")}</span>
        </a>
      </p>
      <p className="muted">{t("flow.throughAi")}</p>
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
          <dd className="mono">{edge.kind ?? t("flow.edge.none")}</dd>
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
      <p className="muted">{t("flow.throughAi")}</p>
    </>
  );
}
