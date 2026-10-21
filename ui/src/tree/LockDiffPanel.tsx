import { useId, useRef, useState, type MouseEvent } from "react";
import { apiErrorOf } from "../api/client";
import { useLockDiff } from "../api/queries";
import type { DriftRow, LockDiffCodeSide, LockDiffNode, LockDiffRow, LockDiffSpecSide } from "../api/types";
import { CodeRowList, counted, oneLine } from "../health/codeRows";
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
  if (diff === null) {
    return (
      <p className="bindings-state" data-diff-state="not-shown">
        {why === null ? <>No diff: the daemon&rsquo;s notes above say why.</> : `No diff: ${oneLine(why)}`}
      </p>
    );
  }
  if (diff === "") {
    return (
      <p className="bindings-state" data-diff-state="equal">
        No change since the lock
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
  return <Side which="spec" title={`Node text since the lock: ${oneLine(side.path)} at ${oneLine(side.commit)}`} diff={side.diff} why={why} />;
}

/** The code's side: the entry's blob against the working tree's. */
function CodeSide({ side, why }: { side: LockDiffCodeSide; why: string | null }) {
  return <Side which="code" title={`Code since the lock: ${oneLine(side.file)}, blob ${oneLine(side.blob)}`} diff={side.diff} why={why} />;
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
  const commands = nodeCommands(row.node);
  if (commands === null) {
    return (
      <p className="muted lock-diff-commands">
        No command for <bdi className="mono">{oneLine(row.node)}</bdi>: only a REF of Latin letters, digits and . _ / - is put in a command.
      </p>
    );
  }
  return (
    <div className="lock-diff-commands">
      <CommandList commands={commands} label={`Commands for the row of ${row.node}`} />
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
  const place = placeKey(holder);
  let body;
  if (holder.rows === null) {
    body = (
      <p className="bindings-state bindings-not-read" data-diff-state="not-judged">
        <Icon name="info" />
        <span>Not judged: the daemon&rsquo;s note above says why.</span>
      </p>
    );
  } else if (holder.rows.length === 0) {
    body = (
      <p className="bindings-state" data-diff-state="none">
        None: no drifted binding of this node has a lock entry.
      </p>
    );
  } else {
    body = (
      <>
        <p className="bindings-count">{counted(holder.rows.length, "drifted row", "drifted rows")}</p>
        <CodeRowList
          rows={holder.rows}
          label={`Changes since the lock of ${rowName(holder)}, in spec verify's order`}
          order="in spec verify's order"
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
    <section className="lock-diff-holder" aria-label={many ? `Changes at ${place}` : undefined} data-holder={place}>
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
        announce(`The changes since the lock of ${nodeRef} were read again.`);
      }
    });
  }

  let body;
  const view = query.data;
  if (view === undefined) {
    body =
      failure === null ? (
        <div aria-busy="true">
          <Skeleton label={`Loading the changes since the lock of ${nodeRef}`} lines={3} />
        </div>
      ) : (
        <ErrorPanel
          title={`The changes since the lock of ${nodeRef} could not be read`}
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
            <span>{query.isFetching ? "Reading" : "Read again"}</span>
          </button>
        </p>
        {view.reason !== null ? (
          <p className="verbatim">{view.reason}</p>
        ) : (
          <>
            <Notes notes={view.notes.map(oneLine)} label="Notes from the daemon on the changes since the lock" />
            {view.nodes.map((holder) => (
              <HolderChanges key={placeKey(holder)} project={project} holder={holder} notes={view.notes} many={many} onFollow={onFollow} />
            ))}
          </>
        )}
        {query.error !== null && !query.isFetching && (
          <p className="note" role="alert">
            The changes since the lock could not be read again: {apiErrorOf(query.error).message}
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
          <span>Changes since the lock</span>
        </button>
      </h2>
      <p className="muted lock-diff-about">
        For each drifted binding with a lock entry, what moved in the node&rsquo;s text and in its code since the lock, as git&rsquo;s hunks against the
        working tree. Read when first opened; a code edit shows after Read again.
      </p>
      {open && <div id={bodyId}>{body}</div>}
    </section>
  );
}
