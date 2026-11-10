import { useEffect, useRef, useState } from "react";
import type { FlowGap } from "../api/types";
import { useTranslate } from "../i18n";
import { Badge } from "../ui/Badge";
import { Disclosure } from "../ui/Disclosure";
import { Icon } from "../ui/Icon";
import { gapLook, gapMeaning, gapNotRead, gapsInOrder, gapsSorted, isMeasured, namesDocuments, needsRulesLine } from "./labels";

/** Items a row shows before "Show all". */
export const GAP_ITEMS_SHOWN = 8;

/** A member's words: its title where a read gives one, else its name (flow-scenario D12). */
export type TitleOf = (name: string) => string;

function GapItem({
  gap,
  item,
  marked,
  titleOf,
  onActivate,
  onHand,
}: {
  gap: FlowGap;
  item: readonly string[];
  marked: ReadonlySet<string>;
  titleOf: TitleOf;
  onActivate: (item: readonly string[]) => void;
  /** Null: this item has nothing to hand (`no-start` at the top). */
  onHand: ((gap: FlowGap, item: readonly string[]) => void) | null;
}) {
  const t = useTranslate();
  const words = item.map(titleOf).join(" + ");
  return (
    <li className="flow-gap-item">
      {namesDocuments(gap) ? (
        <button
          type="button"
          className="link-button flow-gap-members"
          aria-current={item.some((member) => marked.has(member)) ? "true" : undefined}
          onClick={() => {
            onActivate(item);
          }}
        >
          <bdi>{words}</bdi>
        </button>
      ) : (
        <span className="flow-gap-members">
          <bdi>{words}</bdi>
        </span>
      )}
      {onHand !== null && isMeasured(gap) && (
        <button
          type="button"
          className="button button-quiet flow-gap-hand"
          aria-haspopup="dialog"
          onClick={() => {
            onHand(gap, item);
          }}
        >
          <Icon name="cardNew" />
          <span>
            {t("flow.handToAi")} <span className="sr-only">{t("flow.handItem", { item: words })}</span>
          </span>
        </button>
      )}
    </li>
  );
}

function GapRow({
  gap,
  unread,
  rooted,
  marked,
  titleOf,
  onActivate,
  onHand,
}: {
  gap: FlowGap;
  unread: boolean;
  /** `no-start`'s item hands the scope's root under one, and nothing at the top. */
  rooted: boolean;
  marked: ReadonlySet<string>;
  titleOf: TitleOf;
  onActivate: (item: readonly string[]) => void;
  onHand: (gap: FlowGap, item: readonly string[]) => void;
}) {
  const t = useTranslate();
  const [all, setAll] = useState(false);
  const list = useRef<HTMLUListElement>(null);
  const meaning = gapMeaning(gap.code, t);
  // Show all: focus to the first item it showed, never the body; an item with no button takes it itself.
  useEffect(() => {
    if (!all) {
      return;
    }
    const first = list.current?.children[GAP_ITEMS_SHOWN];
    if (first instanceof HTMLElement) {
      const stop = first.querySelector<HTMLElement>("button, a[href]");
      if (stop === null) {
        first.tabIndex = -1;
        first.focus();
      } else {
        stop.focus();
      }
    }
  }, [all]);
  const items = unread ? [] : all ? gap.items : gap.items.slice(0, GAP_ITEMS_SHOWN);
  return (
    <li className="flow-gap">
      <p className="flow-gap-head">
        {meaning === null ? (
          <span className="flow-gap-meaning" lang="en">
            {gap.code}
          </span>
        ) : (
          <span className="flow-gap-meaning">{meaning}</span>
        )}
        <Badge name={t("flow.stateName")} look={unread ? gapNotRead(t) : gapLook(gap, t)} />
      </p>
      {items.length > 0 && (
        <ul ref={list} className="flow-gap-items">
          {items.map((item) => (
            <GapItem key={item.join("\n")} gap={gap} item={item} marked={marked} titleOf={titleOf} onActivate={onActivate} onHand={namesDocuments(gap) || rooted ? onHand : null} />
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
          {t("flow.showAll", { count: gap.items.length })} <span className="sr-only">{t("flow.showAllOf", { code: gap.code })}</span>
        </button>
      )}
      <Disclosure summary={t("ui.technicalDetails")} className="flow-gap-details">
        <p className="mono flow-gap-code">{gap.code}</p>
        {!unread && gap.cause !== null && (
          <p className="verbatim flow-gap-cause" lang="en">
            {gap.cause}
          </p>
        )}
      </Disclosure>
    </li>
  );
}

/**
 * The bar's Gaps control, open (docs/features/flow-scenario.md D10): the drawing read's ten rows in
 * plain words, those found first in served order, then those not measured, then one line counting the
 * rows at 0; each row's code and its `cause` folded under Technical details, the cause the daemon's
 * words. A row changes no status or control (ADR-0012): it says what was found, each item takes you
 * to it, and each measured item can go to your AI as a card, which changes no status either. `unread`:
 * the read's last fetch failed, so no row says a state, an item or a cause. It lies over the canvas
 * from its bottom edge, as the legend does.
 */
export function FlowGaps({
  id,
  gaps,
  unread,
  rooted,
  marked,
  titleOf,
  onActivate,
  onHand,
}: {
  id: string;
  gaps: readonly FlowGap[];
  unread: boolean;
  rooted: boolean;
  marked: ReadonlySet<string>;
  titleOf: TitleOf;
  onActivate: (item: readonly string[]) => void;
  onHand: (gap: FlowGap, item: readonly string[]) => void;
}) {
  const t = useTranslate();
  const rules = unread ? null : needsRulesLine(gaps, t);
  const { found, unmeasured, zero } = gapsSorted(gaps);
  const shown = unread ? gapsInOrder(gaps) : [...found, ...unmeasured];
  return (
    <section id={id} className="graph-legend flow-overlay flow-gaps" aria-label={t("flow.gaps.toggle")}>
      {rules !== null && (
        <p className="note flow-gaps-rules">
          <Icon name="info" />
          <span>{rules}</span>
        </p>
      )}
      {shown.length > 0 && (
        <ul className="flow-gap-list">
          {shown.map((gap) => (
            <GapRow key={gap.code} gap={gap} unread={unread} rooted={rooted} marked={marked} titleOf={titleOf} onActivate={onActivate} onHand={onHand} />
          ))}
        </ul>
      )}
      {!unread && zero > 0 && (
        <p className="flow-gaps-zero">
          <Icon name="checkClean" />
          <span>{t("flow.gaps.zero", { count: zero })}</span>
        </p>
      )}
    </section>
  );
}
