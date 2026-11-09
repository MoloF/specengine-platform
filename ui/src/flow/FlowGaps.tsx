import { useState } from "react";
import type { FlowGap } from "../api/types";
import { Badge } from "../ui/Badge";
import { Icon } from "../ui/Icon";
import { useTranslate } from "../i18n";
import { gapLook, gapMeaning, gapNotRead, gapsInOrder, namesDocuments, needsRulesLine } from "./labels";

/** Items a row shows before "Show all". */
export const GAP_ITEMS_SHOWN = 8;

/** Where the viewer's choice to hide the gaps is kept: per viewer, for every project. */
const HIDDEN_KEY = "specengine.flow.gaps-hidden";

/** The one value stored: hidden. Shown removes the key. */
const HIDDEN = "on";

/** Whether the viewer last hid the gaps: shown by default, and when the browser refuses its storage. */
export function recallGapsHidden(): boolean {
  try {
    return window.localStorage.getItem(HIDDEN_KEY) === HIDDEN;
  } catch {
    return false;
  }
}

/** Keeps the viewer's choice; a refused write keeps it for the view only, nothing shown. */
export function rememberGapsHidden(hidden: boolean): void {
  try {
    if (hidden) {
      window.localStorage.setItem(HIDDEN_KEY, HIDDEN);
    } else {
      window.localStorage.removeItem(HIDDEN_KEY);
    }
  } catch {
    // Storage refused or full: the choice holds in the view's state alone.
  }
}

function GapItem({ item, buttons, marked, onActivate }: { item: readonly string[]; buttons: boolean; marked: ReadonlySet<string>; onActivate: (item: readonly string[]) => void }) {
  return (
    <li className="flow-gap-item">
      {item.map((member, index) => (
        <span key={member} className="flow-gap-member">
          {index > 0 && " + "}
          {buttons ? (
            <button
              type="button"
              className="link-button mono"
              aria-current={marked.has(member) ? "true" : undefined}
              onClick={() => {
                onActivate(item);
              }}
            >
              {member}
            </button>
          ) : (
            <span className="mono">{member}</span>
          )}
        </span>
      ))}
    </li>
  );
}

function GapRow({ gap, unread, marked, onActivate }: { gap: FlowGap; unread: boolean; marked: ReadonlySet<string>; onActivate: (item: readonly string[]) => void }) {
  const t = useTranslate();
  const [all, setAll] = useState(false);
  const meaning = gapMeaning(gap.code, t);
  const items = unread ? [] : all ? gap.items : gap.items.slice(0, GAP_ITEMS_SHOWN);
  return (
    <li className="flow-gap">
      <p className="flow-gap-head">
        <span className="mono flow-gap-code">{gap.code}</span>
        <Badge name="State" look={unread ? gapNotRead(t) : gapLook(gap, t)} />
      </p>
      {meaning !== null && <p className="flow-gap-meaning">{meaning}</p>}
      {!unread && gap.cause !== null && <p className="verbatim flow-gap-cause">{gap.cause}</p>}
      {items.length > 0 && (
        <ul className="flow-gap-items">
          {items.map((item) => (
            <GapItem key={item.join("\n")} item={item} buttons={namesDocuments(gap)} marked={marked} onActivate={onActivate} />
          ))}
        </ul>
      )}
      {!unread && !all && gap.items.length > GAP_ITEMS_SHOWN && (
        <button
          type="button"
          className="link-button"
          onClick={() => {
            setAll(true);
          }}
        >
          Show all {gap.items.length} <span className="sr-only">of {gap.code}</span>
        </button>
      )}
    </li>
  );
}

/**
 * Every row of a fresh read says None: this line stands in the tools, in place of the rows and their
 * button (docs/features/screen-fit.md). A failed read never shows it: a None from the last read is
 * no None now.
 */
export function GapsClear() {
  return (
    <p className="note flow-gaps-clear" role="status">
      <Icon name="checkClean" />
      <span>No gaps: every row reads None</span>
    </p>
  );
}

/**
 * The gap rows beside the canvas, outside its tabs (docs/features/flow-canvas.md "Gap rows"): the
 * top read's ten as served, unknown states last. A row changes no status or control (ADR-0012): it
 * says what was found, and each name takes you to it. `unread`: the top read's last fetch failed, so
 * no row says a state, an item or a cause: a None from the last read is no None now.
 */
export function FlowGaps({ gaps, unread, marked, onActivate }: { gaps: readonly FlowGap[]; unread: boolean; marked: ReadonlySet<string>; onActivate: (item: readonly string[]) => void }) {
  const t = useTranslate();
  const rules = unread ? null : needsRulesLine(gaps, t);
  return (
    <section className="flow-gaps" aria-label="Gaps">
      <h2 className="card-section-title">Gaps</h2>
      {rules !== null && (
        <p className="note flow-gaps-rules">
          <Icon name="info" />
          <span>{rules}</span>
        </p>
      )}
      <ul className="flow-gap-list">
        {gapsInOrder(gaps).map((gap) => (
          <GapRow key={gap.code} gap={gap} unread={unread} marked={marked} onActivate={onActivate} />
        ))}
      </ul>
    </section>
  );
}
