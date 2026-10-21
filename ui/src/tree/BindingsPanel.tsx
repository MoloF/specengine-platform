import { useId, useRef, type MouseEvent } from "react";
import { apiErrorOf } from "../api/client";
import { useNodeBindings } from "../api/queries";
import type { ShownBindings, ShownNode, TestedRow } from "../api/types";
import { CodeRowList, counted, oneLine } from "../health/codeRows";
import { TestedLine } from "../health/DriftPart";
import { Notes } from "../overview/parts";
import { useAnnounce } from "../ui/announcer";
import { Icon } from "../ui/Icon";
import { ErrorPanel, Skeleton } from "../ui/states";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { useRetryFocus } from "../ui/useRetryFocus";
import { BindingLine } from "./BindingLine";
import { CommandList } from "./CommandList";
import { nodeCommands } from "./commands";
import { LockDiffPanel } from "./LockDiffPanel";
import { placeKey, rowName } from "./rows";

// The Bindings tab (docs/features/ui-node-bindings.md), its data
// `docs/canon/code-reads.md` "spec show --bindings": per holder, its code bindings and those of
// the sections within it as rows in the daemon's order, each with the state `spec verify --node`
// gives it now, and its `[[tested]]` entries; then "Changes since the lock" (./LockDiffPanel.tsx,
// docs/features/lock-diff-ui.md), then the node's commands to copy. Visibility only: no row, state
// or entry holds any work (ADR-0012, ADR-0046); every word goes through the tree's labels, never
// compared here.

/** How a holder's lists are headed: under the node's h1 with one holder, under each holder's h2 with several. */
function Heading({ many, children }: { many: boolean; children: string }) {
  return many ? <h3 className="card-section-title">{children}</h3> : <h2 className="card-section-title">{children}</h2>;
}

/**
 * A holder's rows: none said as such, never a bare 0; else how many, then the first rows and "Show
 * all", in the daemon's order (verify's key order), never sorted here.
 */
function Rows({ name, bindings, many }: { name: string; bindings: ShownBindings; many: boolean }) {
  const { rows, omitted } = bindings;
  let body;
  if (rows.length === 0 && omitted === 0) {
    body = (
      <p className="bindings-state">
        <span className="health-value health-measured">None</span>: no marker or lock entry binds code to this node or a section within it.
      </p>
    );
  } else {
    body = (
      <>
        <p className="bindings-count">
          {counted(rows.length, "row", "rows")}
          {omitted > 0 && `, ${String(omitted)} more not shown`}
        </p>
        <CodeRowList
          rows={rows}
          label={`Code bindings of ${name}, in spec verify's order`}
          order="in spec verify's order"
          render={(row) => <BindingLine row={row} noId="No ID" focusable />}
        />
      </>
    );
  }
  return (
    <section className="bindings-part">
      <Heading many={many}>Code bindings</Heading>
      {body}
    </section>
  );
}

/** A holder's `[[tested]]` entries: not judged (null) and none ([]) each said as such; else each entry, by node. */
function Tested({
  project,
  name,
  tested,
  many,
  onFollow,
}: {
  project: string;
  name: string;
  tested: TestedRow[] | null;
  many: boolean;
  onFollow: (event: MouseEvent<HTMLAnchorElement>) => void;
}) {
  let body;
  if (tested === null) {
    body = (
      <p className="bindings-state" data-evidence-state="not-judged">
        <span className="health-value">Not judged</span>: the <code>[[tested]]</code> entries were not judged on this read; the daemon&rsquo;s note
        above says why.
      </p>
    );
  } else if (tested.length === 0) {
    body = (
      <p className="bindings-state" data-evidence-state="none">
        <span className="health-value health-measured">None</span>: no <code>[[tested]]</code> block covers this node or a section within it.
      </p>
    );
  } else {
    body = (
      <CodeRowList
        rows={tested}
        label={`Test evidence of ${name}, by node`}
        order="by node"
        render={(entry) => <TestedLine project={project} entry={entry} onFollow={onFollow} />}
      />
    );
  }
  return (
    <section className="bindings-part">
      <Heading many={many}>Test evidence</Heading>
      {body}
    </section>
  );
}

/**
 * One holder's part: its place when several hold the REF; then, bindings not read (null: no `[code]`
 * or a cause listing no row), "Not read" and no number, the daemon's note above saying why; else its
 * rows and its test evidence.
 */
function HolderBindings({
  project,
  holder,
  many,
  onFollow,
}: {
  project: string;
  holder: ShownNode;
  many: boolean;
  onFollow: (event: MouseEvent<HTMLAnchorElement>) => void;
}) {
  const name = rowName(holder);
  const place = `${holder.path}:${String(holder.line)}`;
  return (
    <section className="holder-bindings" aria-label={many ? `Bindings at ${place}` : undefined} data-holder={place}>
      {many && (
        <h2 className="holder-text-title mono">
          {holder.path}:{holder.line}-{holder.end_line}
        </h2>
      )}
      {holder.bindings === null ? (
        <p className="bindings-state bindings-not-read">
          <Icon name="info" />
          <span>
            <span className="health-value">Not read</span>: no code binding of this node came with the read; the daemon&rsquo;s note above says
            why.
          </span>
        </p>
      ) : (
        <>
          <Rows name={name} bindings={holder.bindings} many={many} />
          <Tested project={project} name={name} tested={holder.bindings.tested} many={many} onFollow={onFollow} />
        </>
      )}
    </section>
  );
}

/**
 * The commands for a terminal, once per tab, each with Copy: built from the REF the tab was opened
 * with (the route's, which the daemon resolved; `slug/ID` stays whole), never from a holder's bare
 * `id`. A REF naming several holders, or one document with no ID, gets verify alone, since `propose
 * drift` and `lock accept` each need one node with an ID; a REF the check refuses gets none.
 */
function NodeCommands({ nodeRef, holders }: { nodeRef: string; holders: readonly ShownNode[] }) {
  const headingId = useId();
  const several = holders.length > 1;
  const idless = holders.length === 1 && holders[0]?.id === null;
  const commands = nodeCommands(nodeRef, several || idless);
  return (
    <section className="owner-commands" aria-labelledby={headingId}>
      <h2 id={headingId} className="owner-commands-title">
        <Icon name="terminal" />
        <span>Commands for a terminal</span>
      </h2>
      <p className="owner-commands-note">
        Copy a command and run it in the project&rsquo;s worktree; only <code>spec lock accept</code> asks [y/N], before it writes spec.lock, and{" "}
        <code>spec propose drift</code> queues an item for your decision. Nothing runs from here, and nothing waits on these rows.
      </p>
      {commands === null ? (
        <p className="muted">
          No command for <bdi className="mono">{oneLine(nodeRef)}</bdi>: only a REF of Latin letters, digits and . _ / - is put in a command.
        </p>
      ) : (
        <>
          {several && (
            <p className="owner-commands-note">
              Only verify is offered: <bdi className="mono">{oneLine(nodeRef)}</bdi> names {holders.length} nodes, and <code>spec propose drift</code> and{" "}
              <code>spec lock accept</code> each name one.
            </p>
          )}
          {idless && (
            <p className="owner-commands-note">
              <bdi className="mono">{oneLine(nodeRef)}</bdi> names a document with no ID; <code>spec propose drift</code> and <code>spec lock accept</code>{" "}
              need one.
            </p>
          )}
          <CommandList commands={commands} label={`Commands for ${nodeRef}`} />
        </>
      )}
    </section>
  );
}

/**
 * The Bindings tab: one read, `nodes/<REF>?with=bindings`, made when the tab first opens and kept
 * while the node is shown; read again on Read again, an apply or a gap (the node's query key), never
 * by a code edit, which fires no event. Loading: a skeleton; failed: the daemon's words and Retry;
 * a later failure a note under the last answer. "Changes since the lock" (./LockDiffPanel.tsx) holds
 * its own read and its own Read again; this one never reads it.
 */
export function BindingsPanel({ project, nodeRef, onFollow }: { project: string; nodeRef: string; onFollow: (event: MouseEvent<HTMLAnchorElement>) => void }) {
  const query = useNodeBindings(project, nodeRef);
  const failure = useRetainedFailure(query.error, query.isFetching);
  const region = useRef<HTMLDivElement>(null);
  const retried = useRetryFocus(query.data, () => region.current);
  const announce = useAnnounce();

  function readAgain() {
    if (query.isFetching) {
      return;
    }
    void query.refetch({ cancelRefetch: false }).then((result) => {
      if (result.data !== undefined && result.error === null) {
        announce(`The bindings of ${nodeRef} were read again.`);
      }
    });
  }

  if (query.data === undefined) {
    return failure === null ? (
      <div aria-busy="true">
        <Skeleton label={`Loading the bindings of ${nodeRef}`} lines={4} />
      </div>
    ) : (
      <ErrorPanel
        title={`The bindings of ${nodeRef} could not be read`}
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
  const many = view.nodes.length > 1;
  return (
    <div ref={region} tabIndex={-1} className="bindings-panel" aria-busy={query.isFetching}>
      <div className="bindings-head">
        <p className="muted">
          The code bound to {nodeRef} and to the sections within it, each binding with the state <code>spec verify --node</code> gives it now. Read when
          this tab first opens; a code edit shows after Read again.
        </p>
        <button type="button" className="button" aria-disabled={query.isFetching} onClick={readAgain}>
          <Icon name="retry" />
          <span>{query.isFetching ? "Reading" : "Read again"}</span>
        </button>
      </div>
      {view.reason !== null && <p className="verbatim">{view.reason}</p>}
      <Notes notes={view.notes.map(oneLine)} label="Notes from the daemon on the bindings" />
      {view.nodes.map((holder) => (
        <HolderBindings key={placeKey(holder)} project={project} holder={holder} many={many} onFollow={onFollow} />
      ))}
      {view.nodes.length > 0 && <LockDiffPanel project={project} nodeRef={nodeRef} onFollow={onFollow} />}
      {view.nodes.length > 0 && <NodeCommands nodeRef={nodeRef} holders={view.nodes} />}
      {query.error !== null && !query.isFetching && (
        <p className="note" role="alert">
          The bindings could not be read again: {apiErrorOf(query.error).message}
        </p>
      )}
    </div>
  );
}
