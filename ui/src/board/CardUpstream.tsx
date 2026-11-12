import type { ReactNode } from "react";
import type { CardUpstream, CardUpstreamChange } from "../api/types";
import { sectionHash } from "../app/routes";
import { DocLink } from "../doc/DocLink";
import { useLanguage, useTranslate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import { shownSlot } from "../ui/text";
import { formatUtc } from "../ui/time";
import { upstreamChangeLine, upstreamFieldWords, upstreamMarkLine } from "./labels";

// What changed above an open subtask (docs/features/upstream-signal.md "Acceptance criteria", US-13):
// the parent's mark, then each change oldest first, as the card's read serves them, and a count of the
// older ones. Information alone: it moves, refuses and gates nothing (ADR-0012). Every word is
// labels.ts's; one this build does not know stands raw, under lang="en". No `seq` is drawn.

/** A served word raw, as the daemon's words stand. */
function Raw({ text }: { text: string }) {
  return <span lang="en">{shownSlot(text)}</span>;
}

/** A node or record ID as stored. */
function Id({ id }: { id: string }) {
  return <bdi className="mono">{shownSlot(id)}</bdi>;
}

/** An edit's fields in the viewer's language, joined by `Intl.ListFormat` around slots (fill.ts); a field this build does not know raw. */
function Fields({ fields }: { fields: readonly string[] }) {
  const t = useTranslate();
  const { language } = useLanguage();
  const words = fields.map((field) => upstreamFieldWords(field, t));
  const joined = new Intl.ListFormat(language, { type: "conjunction" }).format(words.map((_word, index) => slot(String(index))));
  return fill(joined, Object.fromEntries(words.map((word, index) => [String(index), word.served ? <Raw text={word.text} /> : word.text])));
}

/** One change: its sentence, its time and who made it. */
function Change({ project, change }: { project: string; change: CardUpstreamChange }) {
  const t = useTranslate();
  const line = upstreamChangeLine(change, t);
  // The item's record as a document, else the item in the Inbox.
  const id =
    change.record !== null ? (
      <DocLink project={project} docRef={change.record}>
        <Id id={change.record} />
      </DocLink>
    ) : change.item !== null ? (
      <a href={sectionHash(project, "inbox", change.item)}>
        <Id id={change.item} />
      </a>
    ) : null;
  const nodes = change.on === null ? null : <Id id={change.on} />;
  // A slot with nothing served says "-", as the CLI prints a null; never the slot's name.
  const slots: Record<string, ReactNode> = { fields: <Fields fields={change.fields} />, id: id ?? "-", nodes: nodes ?? "-" };
  return (
    <li className="board-history-line">
      <span>
        {line.served ? (
          <>
            <Raw text={line.text} />
            {id !== null && <> {id}</>}
            {nodes !== null && <> {nodes}</>}
          </>
        ) : (
          fill(line.text, slots)
        )}
      </span>
      <span>
        <time dateTime={change.at}>{formatUtc(change.at, t)}</time>
        {change.by !== null && <> {fill(t("board.upstream.by", { name: slot("name") }), { name: <bdi>{shownSlot(change.by)}</bdi> })}</>}
      </span>
    </li>
  );
}

/** The work item's Changed above part: a list, the parent's mark first, then the changes and how many older ones are not listed. */
export function UpstreamPart({ project, upstream }: { project: string; upstream: CardUpstream }) {
  const t = useTranslate();
  const mark = upstreamMarkLine(upstream.mark, t);
  const on = <Id id={upstream.on ?? "-"} />;
  return (
    <ul className="board-panel-list">
      {mark !== null && (
        <li className="board-text">
          {mark.served ? (
            <>
              <Raw text={mark.text} /> {on}
            </>
          ) : (
            fill(mark.text, { on, correction: upstream.correction === null ? t("board.markLine.aCorrection") : <Id id={upstream.correction} /> })
          )}
        </li>
      )}
      {upstream.changes.map((change) => (
        <Change key={change.seq} project={project} change={change} />
      ))}
      {upstream.more > 0 && <li className="muted">{t("board.upstream.more", { count: upstream.more })}</li>}
    </ul>
  );
}
