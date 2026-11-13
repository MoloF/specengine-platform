import { useId } from "react";
import type { Evidence, Pair, PairSide } from "../api/types";
import { useTranslate } from "../i18n";
import { Icon } from "../ui/Icon";
import { shownSlot } from "../ui/text";
import { EvidenceItem } from "./EvidenceItem";

// A two-sided collision (docs/features/collision-pairs.md "Acceptance criteria", PP9, PP10): the two
// documents a discrepancy says contradict each other, side by side under their titles, each with the
// evidence the daemon served as that side's, then the rest. Its own chunk, loaded only for an item that
// holds a `pair`. Everything is laid out as served: the sides, their order and each side's positions in
// the item's `evidence`. Nothing here groups evidence, counts targets, matches a file to a path or reads
// what kind of item it is (docs/canon/architecture.md "Interface"); a pair decides nothing (ADR-0012).

/**
 * Opens the document modal on a REF from the button pressed, focus coming back to it on close: the
 * slot passes `openDocFrom` (src/app/location.ts), so this chunk takes no route module of its own and
 * first load keeps its chunks as they were (PP10).
 */
type OpenDoc = (ref: string, opener: HTMLElement) => void;

/** One evidence entry at a served position, keyed by its place in the served list. */
type Held = { key: string; item: Evidence };

/**
 * The entries at the served positions, in served order; a position the item does not hold (past its
 * end, negative, not a whole number) draws nothing.
 */
function heldAt(positions: readonly number[], evidence: readonly Evidence[]): Held[] {
  return positions.flatMap((position, index) => {
    const item = Number.isInteger(position) && position >= 0 ? evidence[position] : undefined;
    return item === undefined ? [] : [{ key: `${String(index)}-${String(position)}`, item }];
  });
}

function EvidenceList({ held }: { held: readonly Held[] }) {
  return (
    <ul className="evidence-list">
      {held.map(({ key, item }) => (
        <EvidenceItem key={key} item={item} />
      ))}
    </ul>
  );
}

/**
 * One side: its title as the heading, a button opening the document modal on its ID, else its path
 * (`docs/canon/doc-view.md` "History and focus": focus comes back to it); with no title now it says so,
 * and with no path either the heading opens nothing. Under it the ID and path but the one the heading
 * already names, then its evidence. A `section` with no name of its own: the pair is the one region,
 * its sides structured by their headings. Every served string is agent text or a stored path, written
 * as the CLI prints a one-line slot (`shownSlot`).
 */
function Side({ side, evidence, onOpen }: { side: PairSide; evidence: readonly Evidence[]; onOpen: OpenDoc }) {
  const t = useTranslate();
  const gone = side.title === null;
  const named = side.id ?? side.path;
  // What the heading opens: the ID, else the path; nothing when neither its title nor its file is known now.
  const opens = gone && side.path === null ? null : named;
  const name =
    side.title !== null ? (
      <bdi>{shownSlot(side.title)}</bdi>
    ) : named !== null ? (
      <bdi className="mono" lang="en">
        {shownSlot(named)}
      </bdi>
    ) : (
      t("inbox.pair.gone")
    );
  // With no title the heading is the ID, else the path: the line under it leaves that one out.
  const whereId = gone ? null : side.id;
  const wherePath = gone && side.id === null ? null : side.path;
  // With no position resolving, it says so in place of an empty list; a side with no file names no
  // file to cite, so it says nothing (docs/features/release-hardening.md "Acceptance criteria", RH-U3).
  const held = heldAt(side.evidence, evidence);
  const noEvidence = side.path === null ? null : <p className="muted">{t("inbox.pair.noEvidence")}</p>;
  return (
    <section className="collision-pair-side">
      <h3 className="collision-pair-title">
        {opens === null ? (
          name
        ) : (
          <button
            type="button"
            className="link-button collision-pair-open"
            aria-haspopup="dialog"
            onClick={(event) => {
              onOpen(opens, event.currentTarget);
            }}
          >
            {name}
          </button>
        )}
      </h3>
      {gone && named !== null && (
        <p className="collision-pair-gone">
          <Icon name="info" />
          <span>{t("inbox.pair.gone")}</span>
        </p>
      )}
      {(whereId !== null || wherePath !== null) && (
        <p className="collision-pair-where">
          {whereId !== null && <span className="mono">{shownSlot(whereId)}</span>}
          {wherePath !== null && (
            <span className="mono">
              <bdi>{shownSlot(wherePath)}</bdi>
            </span>
          )}
        </p>
      )}
      {held.length === 0 ? noEvidence : <EvidenceList held={held} />}
    </section>
  );
}

/**
 * The pair, one region named "What conflicts with what": the left side, then the right, side by side or
 * stacked; then the evidence citing neither, when any resolves, under a heading and no region of its own.
 */
export default function PairView({ pair, evidence, onOpen }: { pair: Pair; evidence: readonly Evidence[]; onOpen: OpenDoc }) {
  const t = useTranslate();
  const labelId = useId();
  const other = heldAt(pair.other, evidence);
  return (
    <section className="collision-pair" aria-labelledby={labelId}>
      <p id={labelId} className="card-section-title collision-pair-label">
        {t("inbox.pair.label")}
      </p>
      <div className="collision-pair-sides">
        <Side side={pair.left} evidence={evidence} onOpen={onOpen} />
        <Side side={pair.right} evidence={evidence} onOpen={onOpen} />
      </div>
      {other.length > 0 && (
        <section className="collision-pair-other">
          <h3 className="card-section-title collision-pair-label">{t("inbox.pair.other")}</h3>
          <EvidenceList held={other} />
        </section>
      )}
    </section>
  );
}
