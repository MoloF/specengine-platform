import { useState } from "react";
import type { Card } from "../api/types";
import { DocLink } from "../doc/DocLink";
import { useTranslate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import { Icon } from "../ui/Icon";
import { ErrorPanel } from "../ui/states";
import { shownSlot } from "../ui/text";

// A card's Practices part, the work item's (src/board/CardPanel.tsx), after its targets
// (docs/features/department-practices.md "Acceptance criteria", DP21; ADR-0084): the documents
// whose `disciplines:` name the card's discipline, as its read with `?practices=true` serves them, by
// path, each a document link by its name, its title and its rule count. Information alone: a practice
// moves, refuses and gates nothing (ADR-0012), and nothing here judges a card against one. The daemon's
// notes on them (`practices: …`) stand here, verbatim, not among the card's: one that the read failed
// is the failure, with Retry; one past the first 32 stands under the list. Not asked (`null` with no
// such note), the part stays away.

/** The prefix the daemon starts a note on a card's practices with. */
const PRACTICES_NOTE = "practices: ";

/** Whether a card's note is about its practices: shown in their part, not among its notes. */
export function isPracticesNote(note: string): boolean {
  return note.startsWith(PRACTICES_NOTE);
}

/**
 * The practices of the card's discipline: a list, its empty state, or the read's failure with Retry.
 * Retry reads the card again (`onRetry`, settling when the read does) and is busy only after its own
 * press, never while a live event reads the card.
 */
export function PracticesPart({ project, card, onRetry }: { project: string; card: Card; onRetry: () => Promise<unknown> }) {
  const t = useTranslate();
  const [retries, setRetries] = useState(0);
  const [retrying, setRetrying] = useState(false);
  const notes = card.notes.filter(isPracticesNote);
  // Asked and not served: the read failed, and its note says why (`practices: not read: <cause>`); served, a note is the one past the first 32.
  const unread = card.practices === null ? notes[0] : undefined;
  if (card.practices === null && unread === undefined) {
    return null;
  }
  let body;
  if (card.practices === null) {
    body = (
      <ErrorPanel
        title={t("practices.failed")}
        message={unread ?? ""}
        retrying={retrying}
        attempt={retries}
        onRetry={() => {
          setRetries((count) => count + 1);
          setRetrying(true);
          void onRetry().finally(() => {
            setRetrying(false);
          });
        }}
      />
    );
  } else if (card.practices.length === 0) {
    body = (
      <div className="practices-empty">
        <p>
          {card.discipline === null
            ? t("practices.noDiscipline")
            : fill(t("practices.none", { discipline: slot("discipline") }), { discipline: <bdi className="mono">{shownSlot(card.discipline)}</bdi> })}
        </p>
        <p className="muted">{card.discipline === null ? t("practices.noDisciplineNext") : t("practices.next")}</p>
      </div>
    );
  } else {
    body = (
      <ul className="board-panel-list practices-list">
        {card.practices.map((practice) => (
          <li key={practice.path} className="practices-row">
            <DocLink project={project} docRef={practice.name}>
              <bdi className="mono">{shownSlot(practice.name)}</bdi>
            </DocLink>
            {practice.title !== null && <bdi className="practices-row-title">{shownSlot(practice.title)}</bdi>}
            <span className="muted">{t("practices.rules", { count: practice.rules })}</span>
          </li>
        ))}
      </ul>
    );
  }
  const more = notes.filter((note) => note !== unread);
  return (
    <section className="board-panel-part" aria-label={t("practices.title")}>
      <h3 className="issue-section-title">{t("practices.title")}</h3>
      {body}
      {more.map((note, index) => (
        <p key={`${String(index)}-${note}`} className="note">
          <Icon name="info" />
          <span lang="en">{note}</span>
        </p>
      ))}
    </section>
  );
}
