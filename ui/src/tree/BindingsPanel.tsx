import { useId, useRef, type MouseEvent } from "react";
import { apiErrorOf } from "../api/client";
import { useNodeBindings } from "../api/queries";
import type { ShownBindings, ShownNode, TestedRow } from "../api/types";
import { CodeRowList, oneLine } from "../health/codeRows";
import { TestedLine } from "../health/DriftPart";
import { Notes } from "../ui/served";
import { useAnnounce } from "../ui/announcer";
import { Button } from "../ui/Button";
import { Icon } from "../ui/Icon";
import { ErrorPanel, Skeleton } from "../ui/states";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { useRetryFocus } from "../ui/useRetryFocus";
import { BindingLine } from "./BindingLine";
import { CommandList } from "./CommandList";
import { nodeCommands } from "./commands";
import { LockDiffPanel } from "./LockDiffPanel";
import { placeKey, rowName } from "./rows";
import { useTranslate } from "../i18n";
import { fill, slot } from "../i18n/fill";

/** The commands and the evidence table the tab names, as code. */
const VERIFY_NODE_COMMAND = "spec verify --node";
const LOCK_ACCEPT_COMMAND = "spec lock accept";
const PROPOSE_DRIFT_COMMAND = "spec propose drift";
const TESTED_TABLE = "[[tested]]";

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
  const t = useTranslate();
  const { rows, omitted } = bindings;
  let body;
  if (rows.length === 0 && omitted === 0) {
    body = (
      <p className="bindings-state">
        {fill(t("tree.bindings.none", { none: slot("none") }), { none: <span className="health-value health-measured">{t("tree.bindings.noneWord")}</span> })}
      </p>
    );
  } else {
    const counted = t("tree.bindings.rows", { count: rows.length });
    body = (
      <>
        <p className="bindings-count">{omitted > 0 ? t("tree.bindings.rowsMore", { rows: counted, more: omitted }) : counted}</p>
        <CodeRowList
          rows={rows}
          label={t("tree.bindings.listLabel", { name })}
          order={t("tree.bindings.order")}
          render={(row) => <BindingLine row={row} noId={t("tree.bindings.noId")} focusable />}
        />
      </>
    );
  }
  return (
    <section className="bindings-part">
      <Heading many={many}>{t("tree.bindings.title")}</Heading>
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
  const t = useTranslate();
  let body;
  if (tested === null) {
    body = (
      <p className="bindings-state" data-evidence-state="not-judged">
        {fill(t("tree.bindings.testedNotJudged", { word: slot("word"), table: slot("table") }), {
          word: <span className="health-value">{t("tree.notJudged")}</span>,
          table: <code>{TESTED_TABLE}</code>,
        })}
      </p>
    );
  } else if (tested.length === 0) {
    body = (
      <p className="bindings-state" data-evidence-state="none">
        {fill(t("tree.bindings.testedNone", { none: slot("none"), table: slot("table") }), {
          none: <span className="health-value health-measured">{t("tree.bindings.noneWord")}</span>,
          table: <code>{TESTED_TABLE}</code>,
        })}
      </p>
    );
  } else {
    body = (
      <CodeRowList
        rows={tested}
        label={t("tree.bindings.testedLabel", { name })}
        order={t("tree.bindings.byNode")}
        render={(entry) => <TestedLine project={project} entry={entry} onFollow={onFollow} />}
      />
    );
  }
  return (
    <section className="bindings-part">
      <Heading many={many}>{t("tree.bindings.testedTitle")}</Heading>
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
  const t = useTranslate();
  const name = rowName(holder);
  const place = `${holder.path}:${String(holder.line)}`;
  return (
    <section className="holder-bindings" aria-label={many ? t("tree.bindings.at", { place }) : undefined} data-holder={place}>
      {many && (
        <h2 className="holder-text-title mono">
          {holder.path}:{holder.line}-{holder.end_line}
        </h2>
      )}
      {holder.bindings === null ? (
        <p className="bindings-state bindings-not-read">
          <Icon name="info" />
          <span>
            {fill(t("tree.bindings.notRead", { word: slot("word") }), { word: <span className="health-value">{t("tree.bindings.notReadWord")}</span> })}
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
  const t = useTranslate();
  const headingId = useId();
  const several = holders.length > 1;
  const idless = holders.length === 1 && holders[0]?.id === null;
  const commands = nodeCommands(nodeRef, t, several || idless);
  const ref = <bdi className="mono">{oneLine(nodeRef)}</bdi>;
  const accept = <code>{LOCK_ACCEPT_COMMAND}</code>;
  const drift = <code>{PROPOSE_DRIFT_COMMAND}</code>;
  return (
    <section className="owner-commands" aria-labelledby={headingId}>
      <h2 id={headingId} className="owner-commands-title">
        <Icon name="terminal" />
        <span>{t("tree.commands.title")}</span>
      </h2>
      <p className="owner-commands-note">{fill(t("tree.commands.note", { accept: slot("accept"), drift: slot("drift") }), { accept, drift })}</p>
      {commands === null ? (
        <p className="muted">{fill(t("tree.commands.none", { ref: slot("ref") }), { ref })}</p>
      ) : (
        <>
          {several && (
            <p className="owner-commands-note">
              {fill(t("tree.commands.verifyOnly", { ref: slot("ref"), count: holders.length, drift: slot("drift"), accept: slot("accept") }), { ref, drift, accept })}
            </p>
          )}
          {idless && <p className="owner-commands-note">{fill(t("tree.commands.idless", { ref: slot("ref"), drift: slot("drift"), accept: slot("accept") }), { ref, drift, accept })}</p>}
          <CommandList commands={commands} label={t("tree.commands.label", { ref: nodeRef })} />
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
  const t = useTranslate();
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
        announce(t("tree.bindings.readAgainSaid", { ref: nodeRef }));
      }
    });
  }

  if (query.data === undefined) {
    return failure === null ? (
      <div aria-busy="true">
        <Skeleton label={t("tree.bindings.loading", { ref: nodeRef })} lines={4} />
      </div>
    ) : (
      <ErrorPanel
        title={t("tree.bindings.failed", { ref: nodeRef })}
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
        <p className="muted">{fill(t("tree.bindings.about", { ref: nodeRef, command: slot("command") }), { command: <code>{VERIFY_NODE_COMMAND}</code> })}</p>
        <Button icon="retry" aria-disabled={query.isFetching} onClick={readAgain}>
          <span>{query.isFetching ? t("tree.bindings.reading") : t("tree.bindings.readAgain")}</span>
        </Button>
      </div>
      {view.reason !== null && (
        <p className="verbatim" lang="en">
          {view.reason}
        </p>
      )}
      <Notes notes={view.notes.map(oneLine)} label={t("tree.bindings.notes")} />
      {view.nodes.map((holder) => (
        <HolderBindings key={placeKey(holder)} project={project} holder={holder} many={many} onFollow={onFollow} />
      ))}
      {view.nodes.length > 0 && <LockDiffPanel project={project} nodeRef={nodeRef} onFollow={onFollow} />}
      {view.nodes.length > 0 && <NodeCommands nodeRef={nodeRef} holders={view.nodes} />}
      {query.error !== null && !query.isFetching && (
        <p className="note" role="alert">
          {fill(t("tree.bindings.failedAgain", { message: slot("message") }), { message: <span lang="en">{apiErrorOf(query.error).message}</span> })}
        </p>
      )}
    </div>
  );
}
