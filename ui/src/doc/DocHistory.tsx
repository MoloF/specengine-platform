import { useState, type ReactNode } from "react";
import { apiErrorOf } from "../api/client";
import { useNodeHistory } from "../api/queries";
import type { HistoryEntry } from "../api/types";
import { sectionHash } from "../app/routes";
import { useTranslate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import { Disclosure } from "../ui/Disclosure";
import { Lozenge } from "../ui/Lozenge";
import { ErrorPanel, Skeleton } from "../ui/states";
import { shownSlot } from "../ui/text";
import { formatUtc } from "../ui/time";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { DocLink } from "./DocLink";
import type { PartLevel } from "./DocLinks";
import { historyNotes, historyText, historyWhat } from "./history";

// A node's changes, newest first (docs/features/node-history.md "Acceptance criteria", NH7, NH13):
// collapsed, read on its first expansion only, `nodes/<REF>?with=history`, never on open; a second
// expansion reads nothing. The choice to keep it open is the Shell's, per project for the page's life.
// The first holder's list, in served order: what changed, when, who by name, and on an apply who
// proposed it and its proposal; the rationale or title on a line of its own. Information on the
// document, never a hold on work (ADR-0012); its words in ./history.ts.

/** What an entry did: its sentence, a task's ID a Board link, an item's record a document link; a word this build does not know, raw under lang="en". */
function What({ project, entry }: { project: string; entry: HistoryEntry }) {
  const t = useTranslate();
  const what = historyWhat(entry, t);
  if (what.raw) {
    return (
      <span className="doc-history-change" lang="en">
        {shownSlot(what.text)}
      </span>
    );
  }
  const { slot: held } = what;
  let filled: ReactNode = what.text;
  if (held?.name === "id") {
    filled = fill(what.text, { id: <a href={sectionHash(project, "board", held.id)}>{shownSlot(held.id)}</a> });
  } else if (held?.name === "record") {
    filled = fill(what.text, {
      record: (
        <DocLink project={project} docRef={held.record}>
          {shownSlot(held.record)}
        </DocLink>
      ),
    });
  }
  return <span className="doc-history-change">{filled}</span>;
}

/** An apply's proposer, under lang="en", and its proposal, an Inbox link; the link alone when no proposer was made of the trailer, never the trailer itself. */
function ProposedBy({ project, entry, proposal }: { project: string; entry: HistoryEntry; proposal: string }) {
  const t = useTranslate();
  const link = <a href={sectionHash(project, "inbox", proposal)}>{shownSlot(proposal)}</a>;
  const { proposer } = entry;
  if (proposer === null) {
    return <span>{link}</span>;
  }
  return <span>{fill(t("doc.history.proposedBy", { proposer: slot("proposer"), proposal: slot("proposal") }), { proposer: <span lang="en">{shownSlot(proposer)}</span>, proposal: link })}</span>;
}

/** One change: Created first when it made the node, what it did; the date, who by name, on an apply who proposed it; then its line. */
function Entry({ project, entry }: { project: string; entry: HistoryEntry }) {
  const t = useTranslate();
  const text = historyText(entry);
  return (
    <li className="doc-history-entry">
      <p className="doc-history-what">
        {entry.created && <Lozenge appearance="neutral" icon="cardNew" label={t("doc.history.created")} />}
        <What project={project} entry={entry} />
      </p>
      <p className="doc-history-meta">
        <time dateTime={entry.at}>{formatUtc(entry.at, t)}</time>
        <span>{fill(t("doc.history.by", { name: slot("name") }), { name: <bdi>{shownSlot(entry.by_name)}</bdi> })}</span>
        {entry.proposal !== null && <ProposedBy project={project} entry={entry} proposal={entry.proposal} />}
      </p>
      {text !== null && (
        <p className="doc-history-text">
          <bdi>{shownSlot(text)}</bdi>
        </p>
      )}
    </li>
  );
}

/** The read once asked: a skeleton, the daemon's words and Retry, its reason for a REF naming nothing; else the list, or the empty line when nothing more exists, then the truncated line and the read's history notes. */
function HistoryBody({ project, docRef, asked }: { project: string; docRef: string; asked: boolean }) {
  const t = useTranslate();
  const read = useNodeHistory(project, docRef, asked);
  const failure = useRetainedFailure(read.error, read.isFetching);
  if (!asked) {
    return null;
  }
  const data = read.data;
  if (data === undefined) {
    return failure === null ? (
      <div aria-busy="true">
        <Skeleton label={t("doc.history.reading")} lines={3} />
      </div>
    ) : (
      <ErrorPanel
        title={t("doc.history.failed")}
        message={apiErrorOf(failure).message}
        retrying={read.isFetching}
        attempt={read.errorUpdateCount}
        onRetry={() => {
          void read.refetch({ cancelRefetch: false });
        }}
      />
    );
  }
  if (data.reason !== null) {
    return (
      <p className="verbatim" lang="en">
        {data.reason}
      </p>
    );
  }
  const history = data.nodes[0]?.history ?? null;
  const notes = historyNotes(data.notes);
  return (
    <>
      {history !== null &&
        (history.entries.length === 0 ? (
          // None listed of more: only that older changes are not listed, never that none is recorded.
          !history.truncated && <p className="muted">{t("doc.history.none")}</p>
        ) : (
          <ol className="doc-history-list">
            {history.entries.map((entry, index) => (
              <Entry key={`${String(index)}:${entry.id}`} project={project} entry={entry} />
            ))}
          </ol>
        ))}
      {history?.truncated === true && <p className="muted">{t("doc.history.truncated")}</p>}
      {notes.length > 0 && (
        <ul className="note-list" aria-label={t("doc.technical.notes")}>
          {notes.map((note, index) => (
            <li key={`${String(index)}:${note}`} lang="en">
              {note}
            </li>
          ))}
        </ul>
      )}
    </>
  );
}

/** History: a disclosure at the part's level, held by the Shell; read once first expanded. */
export function DocHistory({ project, docRef, open, onToggle, level }: { project: string; docRef: string; open: boolean; onToggle: (open: boolean) => void; level: PartLevel }) {
  const t = useTranslate();
  const [asked, setAsked] = useState(open);
  if (open && !asked) {
    setAsked(true);
  }
  return (
    <Disclosure summary={t("doc.history.title")} open={open} onToggle={onToggle} level={level} className="doc-part doc-history">
      <HistoryBody project={project} docRef={docRef} asked={asked} />
    </Disclosure>
  );
}
