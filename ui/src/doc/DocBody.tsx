import { lazy, Suspense, useId, useRef, useState, type ReactNode } from "react";
import { apiErrorOf } from "../api/client";
import { useProjects, useTrace, type NodeQuery } from "../api/queries";
import type { CardSplit, CardWrite, NodeView, ShownNode, TraceDocument } from "../api/types";
import { docPageHash, flowHash, homeHash, sectionHash } from "../app/routes";
import { NewCardDialog } from "../board/CardDialogs";
import { rollupLozenge } from "../flow/labels";
import { useTranslate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import { Prose } from "../markdown/Prose";
import { splitFrontMatter } from "../markdown/source";
import { CardsBlock } from "../trace/CardsBlock";
import { rowName } from "../tree/rows";
import { useAnnounce } from "../ui/announcer";
import { Breadcrumbs, type Crumb } from "../ui/Breadcrumbs";
import { Button, buttonClasses } from "../ui/Button";
import { Icon } from "../ui/Icon";
import { Lozenge } from "../ui/Lozenge";
import { ErrorPanel, Skeleton } from "../ui/states";
import { useFocusLater } from "../ui/useFocusLater";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { DocHistory } from "./DocHistory";
import { DocInside } from "./DocInside";
import { DocLinks, PartHeading, type PartLevel } from "./DocLinks";
import { DocTechnical } from "./DocTechnical";

/** Split to departments, a chunk of its own, loaded once pressed (docs/features/department-split.md "Acceptance criteria", U1). */
const SplitDialog = lazy(() => import("../board/SplitDialog").then((module) => ({ default: module.SplitDialog })));

// One document as a work item (docs/features/doc-modal.md "Acceptance criteria", DM-12): the parts the
// frame (./DocFrame.tsx) lays out around its own title, loaded on demand as one chunk: the path from
// the top as the breadcrumb, the work state and the raw status, the acts; the main column (the text
// at reading width, the links by title, Inside, History); the side (Cards, Technical details folded). The
// frame's title never waits for this chunk, so focus on it never moves. Read only: changes go through
// the AI, as proposals (ADR-0004); a card is the one write here.

/** The trace read's row of this document, never another's: a read of a group holds what it holds besides. */
function rowOf(trace: TraceDocument, ref: string) {
  return trace.nodes.find((node) => node.id === ref);
}

/** Under the title: the work state, from the node's trace row, none while it is read; then the raw status, none for a section. */
export function DocMeta({ project, docRef, holder }: { project: string; docRef: string; holder: ShownNode }) {
  const t = useTranslate();
  const trace = useTrace(project, { ref: docRef });
  const data = trace.data;
  let state: ReactNode = null;
  if (data !== undefined && data.reason === null && trace.error === null) {
    const row = rowOf(data, docRef);
    const look = row === undefined ? { appearance: "neutral" as const, icon: "unknown" as const, label: t("doc.notKnown"), served: false } : rollupLozenge(row.rollup, t);
    state = (
      <span className="doc-meta-item">
        <span className="sr-only">{t("doc.workState")}</span>
        <Lozenge appearance={look.appearance} icon={look.icon} label={look.label} lang={look.served ? "en" : undefined} />
      </span>
    );
  }
  return (
    <>
      {state}
      {holder.status !== null && (
        <span className="doc-meta-item">
          <span className="sr-only">{t("doc.status")}</span>
          <Lozenge appearance="neutral" icon="document" label={holder.status} lang="en" />
        </span>
      )}
    </>
  );
}

/** The document's text, less its front-matter and a first heading saying its title, at reading width; its own headings under Text's. */
function Text({ project, holder, level }: { project: string; holder: ShownNode; level: PartLevel }) {
  const t = useTranslate();
  const id = useId();
  const split = splitFrontMatter(holder.text);
  return (
    <section className="doc-part" aria-labelledby={id}>
      <PartHeading level={level} id={id}>
        {t("doc.text")}
      </PartHeading>
      <Prose
        className="doc-text"
        source={split.body}
        links={holder.links?.outgoing ?? null}
        project={project}
        baseLevel={level}
        frontMatter={false}
        firstLine={holder.line + split.lines}
        path={holder.path}
        dropTitle={holder.title}
      />
      <p className="muted">{t("doc.throughAi")}</p>
    </section>
  );
}

/** The path from the top: Project, each ancestor by title (else ID, else path) a page link, the document current. */
export function DocCrumbs({ project, docRef, holder, onCrumb }: { project: string; docRef: string; holder: ShownNode | undefined; onCrumb?: (ref: string) => void }) {
  const projects = useProjects();
  const listed = projects.data?.find((candidate) => candidate.slug === project);
  const crumbs: Crumb[] = [
    { key: "project", label: listed?.name ?? project, href: homeHash(project) },
    ...(holder?.ancestors ?? []).map((ancestor, index): Crumb => {
      const ref = ancestor.id ?? ancestor.path;
      return {
        key: `${String(index)}:${ref}`,
        label: ancestor.title ?? ancestor.id ?? ancestor.path,
        href: docPageHash(project, ref),
        onFollow:
          onCrumb === undefined
            ? undefined
            : () => {
                onCrumb(ref);
              },
      };
    }),
    { key: "doc", label: holder?.title ?? docRef },
  ];
  return <Breadcrumbs items={crumbs} />;
}

/** Open in Spec tree: the technical view, the one act left when no document answers the REF. */
function TreeAct({ project, name }: { project: string; name: string }) {
  const t = useTranslate();
  return (
    <a className={buttonClasses()} href={sectionHash(project, "tree", name)}>
      {t("doc.openTree")}
    </a>
  );
}

/**
 * The acts: New card (the one primary, its dialog here), Split to departments… (its dialog a chunk of
 * its own, the document the parent's target: docs/features/department-split.md "Acceptance criteria",
 * U1), Open in Flow under the last ancestor, Open in Spec tree; a REF naming nothing, the last alone.
 * A split made is said in the live region and noted under the acts, the parent a link to its work item.
 */
export function DocActs({ project, docRef, holder }: { project: string; docRef: string; holder: ShownNode | undefined }) {
  const t = useTranslate();
  const announce = useAnnounce();
  const focusLater = useFocusLater();
  const newButton = useRef<HTMLButtonElement>(null);
  const splitButton = useRef<HTMLButtonElement>(null);
  const [creating, setCreating] = useState(false);
  const [splitting, setSplitting] = useState(false);
  const [split, setSplit] = useState<{ id: string; count: number } | null>(null);
  if (holder === undefined) {
    return (
      <div className="doc-acts">
        <TreeAct project={project} name={docRef} />
      </div>
    );
  }
  const name = rowName(holder);
  const scope = holder.ancestors.at(-1);
  const flowPlace = { module: null, scope: scope === undefined ? null : (scope.id ?? scope.path), stage: null, select: { node: name } };

  function made(answer: CardWrite | null) {
    setCreating(false);
    focusLater(() => newButton.current);
    if (answer !== null && answer.id !== null) {
      announce(t("trace.created", { id: answer.id }));
    }
  }

  function splitDone(answer: CardSplit | null) {
    setSplitting(false);
    focusLater(() => splitButton.current);
    const id = answer?.parent?.id ?? null;
    if (answer !== null && id !== null) {
      setSplit({ id, count: answer.children.length });
      announce(t("board.split.done", { id, count: answer.children.length }));
    }
  }

  return (
    <div className="doc-acts">
      <Button
        ref={newButton}
        appearance="primary"
        icon="cardNew"
        aria-haspopup="dialog"
        onClick={() => {
          setCreating(true);
        }}
      >
        {t("doc.newCard")}
      </Button>
      <Button
        ref={splitButton}
        icon="workspace"
        aria-haspopup="dialog"
        onClick={() => {
          setSplit(null);
          setSplitting(true);
        }}
      >
        {t("board.split.act")}
      </Button>
      <a className={buttonClasses()} href={flowHash(project, flowPlace)}>
        <Icon name="flowStart" />
        <span>{t("doc.openFlow")}</span>
      </a>
      <TreeAct project={project} name={name} />
      {creating && (
        <div
          onKeyDown={(event) => {
            event.stopPropagation();
          }}
        >
          <NewCardDialog
            project={project}
            target={name}
            onClose={() => {
              made(null);
            }}
            onDone={made}
          />
        </div>
      )}
      {splitting && (
        <div
          onKeyDown={(event) => {
            event.stopPropagation();
          }}
        >
          <Suspense fallback={null}>
            <SplitDialog
              project={project}
              source={{ target: name, title: holder.title ?? docRef }}
              heldTraceRef={docRef}
              onClose={() => {
                splitDone(null);
              }}
              onDone={splitDone}
            />
          </Suspense>
        </div>
      )}
      {split !== null && (
        <p className="note doc-acts-note">
          <Icon name="info" />
          <span>{fill(t("board.split.done", { id: slot("id"), count: split.count }), { id: <a href={sectionHash(project, "board", split.id)}>{split.id}</a> })}</span>
        </p>
      )}
    </div>
  );
}

/**
 * The main column: a skeleton while read, the daemon's words and Retry, its reason for a REF naming
 * nothing, else the holders' line, Text, Links, Inside and History (docs/features/node-history.md
 * "Acceptance criteria", NH7), each folded part held open or not by the Shell.
 */
export function DocMain({
  project,
  docRef,
  read,
  level,
  insideOpen,
  onInside,
  historyOpen,
  onHistory,
}: {
  project: string;
  docRef: string;
  read: NodeQuery;
  level: PartLevel;
  insideOpen: boolean;
  onInside: (open: boolean) => void;
  historyOpen: boolean;
  onHistory: (open: boolean) => void;
}) {
  const t = useTranslate();
  const failure = useRetainedFailure(read.error, read.isFetching);
  const data = read.data;
  const holder = data?.nodes[0];
  if (data === undefined) {
    return failure === null ? (
      <div aria-busy="true">
        <Skeleton label={t("doc.reading")} lines={6} />
      </div>
    ) : (
      <ErrorPanel
        title={t("doc.failed")}
        message={apiErrorOf(failure).message}
        retrying={read.isFetching}
        attempt={read.errorUpdateCount}
        onRetry={() => {
          void read.refetch({ cancelRefetch: false });
        }}
      />
    );
  }
  if (holder === undefined) {
    return (
      <p className="verbatim" lang="en">
        {data.reason}
      </p>
    );
  }
  return (
    <>
      {data.nodes.length > 1 && <p className="note">{t("doc.holders", { ref: docRef, count: data.nodes.length })}</p>}
      <Text project={project} holder={holder} level={level} />
      <DocLinks project={project} links={holder.links} level={level} />
      <DocInside project={project} docRef={docRef} open={insideOpen} onToggle={onInside} level={level} />
      <DocHistory project={project} docRef={docRef} open={historyOpen} onToggle={onHistory} level={level} />
    </>
  );
}

/** The side column: Cards (no New card of its own: the head holds it), then Technical details. */
export function DocSide({ project, docRef, holder, read, level }: { project: string; docRef: string; holder: ShownNode; read: NodeView; level: PartLevel }) {
  return (
    <>
      <CardsBlock project={project} nodeRef={docRef} section="doc" heading={level === 2 ? "h2" : "h3"} newCard={false} />
      <DocTechnical project={project} holder={holder} read={read} />
    </>
  );
}
