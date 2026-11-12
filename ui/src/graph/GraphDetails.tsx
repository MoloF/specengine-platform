import { useId, type KeyboardEvent, type Ref } from "react";
import type { GraphEdge, GraphNode, GraphView } from "../api/types";
import { sectionHash } from "../app/routes";
import { Badge } from "../ui/Badge";
import { Button, buttonClasses } from "../ui/Button";
import { Icon } from "../ui/Icon";
import { hasModifier } from "../ui/keys";
import { PropertyList, type Property } from "../ui/PropertyList";
import { linkStateLook } from "../tree/labels";
import type { BoxContent, LayoutBox } from "./layout";
import { edgeLine, moreLabel } from "./lines";
import { useTranslate, type Translate } from "../i18n";
import { fill, slot } from "../i18n/fill";

/**
 * One holder's facts in the details (docs/features/design-sweep-b.md "Acceptance criteria", DB2):
 * kind, title, place, flags, in that order; null where the holder has none to show.
 */
export function graphFactsOf(holder: GraphNode, t: Translate): Property[] {
  return [
    { key: "kind", label: t("graph.details.kind"), value: <span className="mono">{holder.kind ?? "-"}</span> },
    { key: "title", label: t("graph.details.title"), value: holder.title ?? "-" },
    {
      key: "where",
      label: t("graph.details.where"),
      value: (
        <span className="mono">
          {holder.path}:{holder.line}
        </span>
      ),
    },
    {
      key: "flags",
      label: t("graph.details.flags"),
      value: holder.archived ? (
        <span className="plain-tag">
          <Icon name="archived" />
          {t("graph.archivedWord")}
        </span>
      ) : null,
    },
  ];
}

function EdgeLines({ edges }: { edges: readonly GraphEdge[] }) {
  const t = useTranslate();
  if (edges.length === 0) {
    return <p className="muted">{t("graph.details.noEdge")}</p>;
  }
  return (
    <ul className="graph-edge-lines">
      {edges.map((edge, index) => (
        <li key={`${String(index)}-${edge.path}:${String(edge.line)}`} className="verbatim">
          {edgeLine(edge)}
        </li>
      ))}
    </ul>
  );
}

/**
 * The selected box's details (docs/features/ui-graph.md "Implementation", Details): a walked
 * node's holders, its distance and edges, and where to go next; a stub's state and edge, never a
 * link; a more-box's count and the way to the List. Esc closes it and puts focus back on the box.
 */
export function GraphDetails({
  project,
  answer,
  box,
  headingRef,
  onClose,
  onShowInList,
}: {
  project: string;
  answer: GraphView;
  box: LayoutBox;
  headingRef: Ref<HTMLHeadingElement>;
  onClose: () => void;
  onShowInList: (more: Extract<BoxContent, { type: "more" }>) => void;
}) {
  const t = useTranslate();
  const titleId = useId();
  const content = box.content;

  function onKeyDown(event: KeyboardEvent<HTMLElement>) {
    if (event.key === "Escape" && !hasModifier(event)) {
      event.preventDefault();
      event.stopPropagation();
      onClose();
    }
  }

  let title: string;
  let body;
  if (content.type === "walked") {
    const walked = content.walked;
    title = walked.name;
    const edges = answer.edges.filter((edge) => edge.src === walked.name || edge.dst === walked.name);
    body = (
      <>
        <ul className="graph-holders">
          {walked.holders.map((holder) => (
            <li key={`${holder.path}:${String(holder.line)}`} className="graph-holder">
              <PropertyList label={t("graph.details.holder")} items={graphFactsOf(holder, t)} foldEmpty />
            </li>
          ))}
        </ul>
        <p className="graph-details-distance">
          {t("graph.details.distance", { distance: walked.distance })}
          {walked.distance === 0 && (
            <span className="graph-mark graph-mark-focus">
              <Icon name="focus" />
              {t("graph.focus")}
            </span>
          )}
        </p>
        <section className="graph-details-section">
          <h3 className="card-section-title">{t("graph.list.edges", { count: edges.length })}</h3>
          <EdgeLines edges={edges} />
        </section>
        <p className="graph-details-actions">
          <a className={buttonClasses("subtle", "compact")} href={sectionHash(project, "tree", walked.name)}>
            {t("graph.details.openInTree")}
          </a>
          <a className={buttonClasses("subtle", "compact")} href={sectionHash(project, "graph", walked.name)}>
            <Icon name="focus" />
            {t("graph.details.centre")}
          </a>
        </p>
      </>
    );
  } else if (content.type === "stub") {
    const stub = content.stub.stub;
    if (stub.type === "section") {
      title = stub.name;
      body = (
        <>
          <p className="graph-mark">
            <Icon name="stub" />
            {t("graph.notWalkedSection")}
          </p>
          <p className="muted">{t("graph.details.sectionStub")}</p>
          <EdgeLines edges={content.stub.edges} />
        </>
      );
    } else {
      title = stub.edge.written;
      body = (
        <>
          <p className="graph-details-state">
            <span className="graph-mark">
              <Icon name="stub" />
              {t("graph.notWalked")}
            </span>
            <Badge name={t("graph.list.state")} look={linkStateLook(stub.edge.state, t)} />
          </p>
          <EdgeLines edges={[stub.edge]} />
          {stub.edge.reason !== null && (
            <p className="verbatim" lang="en">
              {stub.edge.reason}
            </p>
          )}
        </>
      );
    }
  } else {
    title = moreLabel(content.hidden, content.column, t);
    body = (
      <>
        <p>{t("graph.details.moreText", { column: content.column, count: content.hidden })}</p>
        <p>
          <Button
            onClick={() => {
              onShowInList(content);
            }}
          >
            {t("graph.details.showInList")}
          </Button>
        </p>
      </>
    );
  }

  return (
    <aside className="graph-details" aria-labelledby={titleId} onKeyDown={onKeyDown}>
      <header className="graph-details-head">
        <h2 id={titleId} ref={headingRef} tabIndex={-1} className="graph-details-title mono">
          {title}
        </h2>
        <span className="graph-mark graph-mark-selected">
          <Icon name="selected" />
          {t("graph.selected")}
        </span>
        <Button appearance="subtle" className="graph-details-close" onClick={onClose}>
          {fill(t("graph.details.close", { what: slot("what") }), { what: <span className="sr-only">{t("graph.details.closeWhat")}</span> })}
        </Button>
      </header>
      {body}
    </aside>
  );
}
