import { useId, useRef, useState, type MouseEvent } from "react";
import { apiErrorOf } from "../api/client";
import { useLockDiff } from "../api/queries";
import type { DriftRow, LockDiffCodeSide, LockDiffNode, LockDiffRow, LockDiffSpecSide } from "../api/types";
import { CodeRowList, oneLine } from "../health/codeRows";
import { useTranslate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import { DriftLine } from "../health/DriftPart";
import { Notes } from "../overview/parts";
import { useAnnounce } from "../ui/announcer";
import { DiffView } from "../ui/DiffView";
import { Icon } from "../ui/Icon";
import { ErrorPanel, Skeleton } from "../ui/states";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { useRetryFocus } from "../ui/useRetryFocus";
import { CommandList } from "./CommandList";
import { nodeCommands } from "./commands";
import { sideNoteOf } from "./lockDiff";
import { placeKey, rowName } from "./rows";

// "Changes since the lock" in the Bindings tab
// (docs/features/lock-diff-ui.md "Description and interactions", "Data"), its read
// `docs/canon/spec-lock-diff.md` "The read": for each binding the lock holds for the REF's holders
// whose state raises drift, verify's row, then what moved on each side since it was locked, as
// git's hunks against the working tree, then the row's commands. Read on the first expanding,
// never when the tab opens. Visibility only: no row, side or note holds any work (ADR-0012,
// ADR-0053); hunks reach DiffView as sent, never re-made, sorted, trimmed or cut
// (docs/canon/architecture.md#ui); a side's presence is `null` or not, and no row word is compared
// here (the label maps tell them apart).

/** A present side's body: its hunks in a figure named by its title; `""` equal; `null` not shown, why from its note or the notes above, never "No change". */
function SideBody({ title, diff, why }: { title: string; diff: string | null; why: string | null }) {
  const t = useTranslate();
  if (diff === null) {
    return (
      <p className="bindings-state" data-diff-state="not-shown">
        {why === null ? t("tree.lockDiff.noDiffNotes") : fill(t("tree.lockDiff.noDiff", { why: slot("why") }), { why: <span lang="en">{oneLine(why)}</span> })}
      </p>
    );
  }
  if (diff === "") {
    return (
      <p className="bindings-state" data-diff-state="equal">
        {t("tree.lockDiff.noChange")}
      </p>
    );
  }
  return <DiffView diff={diff} label={title} />;
}

/** One present side: its title line, then its body. */
function Side({ which, title, diff, why }: { which: "spec" | "code"; title: string; diff: string | null; why: string | null }) {
  return (
    <div className="lock-diff-side" data-side={which}>
      <p className="lock-diff-side-title">{title}</p>
      <SideBody title={title} diff={diff} why={why} />
    </div>
  );
}

/** The node's side: its text at the entry's commit against the working tree's. */
function SpecSide({ side, why }: { side: LockDiffSpecSide; why: string | null }) {
  const t = useTranslate();
  return <Side which="spec" title={t("tree.lockDiff.specSide", { path: oneLine(side.path), commit: oneLine(side.commit) })} diff={side.diff} why={why} />;
}

/** The code's side: the entry's blob against the working tree's. */
function CodeSide({ side, why }: { side: LockDiffCodeSide; why: string | null }) {
  const t = useTranslate();
  return <Side which="code" title={t("tree.lockDiff.codeSide", { file: oneLine(side.file), blob: oneLine(side.blob) })} diff={side.diff} why={why} />;
}

/**
 * A kept row's sides, spec then code: each one sent is drawn (two side by side, a lone one
 * spanning both columns); a `null` one is not, since it did not move; none sent, nothing.
 */
function Sides({ entry, notes }: { entry: LockDiffRow; notes: readonly string[] }) {
  const { row, spec, code } = entry;
  if (spec === null && code === null) {
    return null;
  }
  return (
    <div className="lock-diff-sides">
      {spec !== null && <SpecSide side={spec} why={spec.diff === null ? sideNoteOf(notes, row, "spec") : null} />}
      {code !== null && <CodeSide side={code} why={code.diff === null ? sideNoteOf(notes, row, "code") : null} />}
    </div>
  );
}

/** A kept row's commands for its node, the ID checked as the tab's REF is (`nodeCommands`); an ID failing the check gets none, said so. */
function RowCommands({ row }: { row: DriftRow }) {
  const t = useTranslate();
  const commands = nodeCommands(row.node, t);
  if (commands === null) {
    return <p className="muted lock-diff-commands">{fill(t("tree.commands.none", { ref: slot("ref") }), { ref: <bdi className="mono">{oneLine(row.node)}</bdi> })}</p>;
  }
  return (
    <div className="lock-diff-commands">
      <CommandList commands={commands} label={t("tree.lockDiff.commandsLabel", { id: row.node })} />
    </div>
  );
}

/**
 * One holder's kept rows: its place when several hold the REF; not judged (null: a run cause, its
 * one note above) said so, no count; none ([]) said so; else how many, then the rows in the
 * daemon's order (verify's), never sorted or filtered: verify's row line, its sides, its commands.
 */
function HolderChanges({
  project,
  holder,
  notes,
  many,
  onFollow,
}: {
  project: string;
  holder: LockDiffNode;
  notes: readonly string[];
  many: boolean;
  onFollow?: (event: MouseEvent<HTMLAnchorElement>) => void;
}) {
  const t = useTranslate();
  const place = placeKey(holder);
  let body;
  if (holder.rows === null) {
    body = (
      <p className="bindings-state bindings-not-read" data-diff-state="not-judged">
        <Icon name="info" />
        <span>{t("tree.lockDiff.notJudged")}</span>
      </p>
    );
  } else if (holder.rows.length === 0) {
    body = (
      <p className="bindings-state" data-diff-state="none">
        {t("tree.lockDiff.none")}
      </p>
    );
  } else {
    body = (
      <>
        <p className="bindings-count">{t("tree.lockDiff.rows", { count: holder.rows.length })}</p>
        <CodeRowList
          rows={holder.rows}
          label={t("tree.lockDiff.listLabel", { name: rowName(holder) })}
          order={t("tree.bindings.order")}
          render={(entry) => (
            <DriftLine project={project} row={entry.row} onFollow={onFollow}>
              <Sides entry={entry} notes={notes} />
              <RowCommands row={entry.row} />
            </DriftLine>
          )}
        />
      </>
    );
  }
  return (
    <section className="lock-diff-holder" aria-label={many ? t("tree.lockDiff.at", { place }) : undefined} data-holder={place}>
      {many && <h3 className="holder-text-title mono">{place}</h3>}
      {body}
    </section>
  );
}

/**
 * "Changes since the lock": collapsed at mount, nothing read. The first expanding enables the
 * read (`lock-diff?ref=<REF>`), which stays enabled while the pane shows the node: collapsing and
 * expanding again read nothing; an apply and a gap read it again, expanded or not
 * (`READS_OF_THE_SPEC`); its own Read again sends one GET per click, the lock diff only (the tab's
 * reads the bindings only). Loading: a skeleton; failed: the daemon's words and Retry; a later
 * failure a note under the last answer. `onFollow`: the tree view's, for a row's node link, as the
 * tab's test evidence takes it.
 */
export function LockDiffPanel({ project, nodeRef, onFollow }: { project: string; nodeRef: string; onFollow?: (event: MouseEvent<HTMLAnchorElement>) => void }) {
  const t = useTranslate();
  const base = useId();
  const titleId = `${base}-title`;
  const bodyId = `${base}-body`;
  const [open, setOpen] = useState(false);
  const [asked, setAsked] = useState(false);
  const query = useLockDiff(project, nodeRef, asked);
  const failure = useRetainedFailure(query.error, query.isFetching);
  const region = useRef<HTMLDivElement>(null);
  const retried = useRetryFocus(query.data, () => region.current);
  const announce = useAnnounce();

  function toggle() {
    setOpen((current) => !current);
    setAsked(true);
  }

  function readAgain() {
    if (query.isFetching) {
      return;
    }
    void query.refetch({ cancelRefetch: false }).then((result) => {
      if (result.data !== undefined && result.error === null) {
        announce(t("tree.lockDiff.readAgainSaid", { ref: nodeRef }));
      }
    });
  }

  let body;
  const view = query.data;
  if (view === undefined) {
    body =
      failure === null ? (
        <div aria-busy="true">
          <Skeleton label={t("tree.lockDiff.loading", { ref: nodeRef })} lines={3} />
        </div>
      ) : (
        <ErrorPanel
          title={t("tree.lockDiff.failed", { ref: nodeRef })}
          message={apiErrorOf(failure).message}
          retrying={query.isFetching}
          attempt={query.errorUpdateCount}
          onRetry={() => {
            retried();
            void query.refetch({ cancelRefetch: false });
          }}
        />
      );
  } else {
    const many = view.nodes.length > 1;
    body = (
      <div ref={region} tabIndex={-1} className="lock-diff-body" aria-busy={query.isFetching}>
        <p>
          {/* Described by the panel's heading: a screen reader tells it from the tab's Read again. */}
          <button type="button" className="button" aria-disabled={query.isFetching} aria-describedby={titleId} onClick={readAgain}>
            <Icon name="retry" />
            <span>{query.isFetching ? t("tree.bindings.reading") : t("tree.bindings.readAgain")}</span>
          </button>
        </p>
        {view.reason !== null ? (
          <p className="verbatim" lang="en">
            {view.reason}
          </p>
        ) : (
          <>
            <Notes notes={view.notes.map(oneLine)} label={t("tree.lockDiff.notes")} />
            {view.nodes.map((holder) => (
              <HolderChanges key={placeKey(holder)} project={project} holder={holder} notes={view.notes} many={many} onFollow={onFollow} />
            ))}
          </>
        )}
        {query.error !== null && !query.isFetching && (
          <p className="note" role="alert">
            {fill(t("tree.lockDiff.failedAgain", { message: slot("message") }), { message: <span lang="en">{apiErrorOf(query.error).message}</span> })}
          </p>
        )}
      </div>
    );
  }

  return (
    <section className="lock-diff" aria-labelledby={titleId}>
      <h2 className="lock-diff-title">
        <button id={titleId} type="button" className="button lock-diff-toggle" aria-expanded={open} aria-controls={open ? bodyId : undefined} onClick={toggle}>
          <Icon name={open ? "chevronDown" : "chevronRight"} />
          <span>{t("tree.lockDiff.title")}</span>
        </button>
      </h2>
      <p className="muted lock-diff-about">{t("tree.lockDiff.about")}</p>
      {open && <div id={bodyId}>{body}</div>}
    </section>
  );
}
