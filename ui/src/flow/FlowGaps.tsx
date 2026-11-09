import { useState } from "react";
import type { FlowGap } from "../api/types";
import { useTranslate } from "../i18n";
import { Badge } from "../ui/Badge";
import { Icon } from "../ui/Icon";
import { gapLook, gapMeaning, gapNotRead, gapsInOrder, isMeasured, namesDocuments, needsRulesLine } from "./labels";

/** Items a row shows before "Show all". */
export const GAP_ITEMS_SHOWN = 8;

function GapItem({
  gap,
  item,
  marked,
  onActivate,
  onHand,
}: {
  gap: FlowGap;
  item: readonly string[];
  marked: ReadonlySet<string>;
  onActivate: (item: readonly string[]) => void;
  /** Null: this item has nothing to hand (`no-start` at the top). */
  onHand: ((gap: FlowGap, item: readonly string[]) => void) | null;
}) {
  const t = useTranslate();
  const buttons = namesDocuments(gap);
  return (
    <li className="flow-gap-item">
      <span className="flow-gap-members">
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
      </span>
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
            {t("flow.handToAi")} <span className="sr-only">{t("flow.handItem", { item: item.join(" + ") })}</span>
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
  onActivate,
  onHand,
}: {
  gap: FlowGap;
  unread: boolean;
  /** `no-start`'s item hands the scope's root under one, and nothing at the top. */
  rooted: boolean;
  marked: ReadonlySet<string>;
  onActivate: (item: readonly string[]) => void;
  onHand: (gap: FlowGap, item: readonly string[]) => void;
}) {
  const t = useTranslate();
  const [all, setAll] = useState(false);
  const meaning = gapMeaning(gap.code, t);
  const items = unread ? [] : all ? gap.items : gap.items.slice(0, GAP_ITEMS_SHOWN);
  return (
    <li className="flow-gap">
      <p className="flow-gap-head">
        <span className="mono flow-gap-code">{gap.code}</span>
        <Badge name={t("flow.stateName")} look={unread ? gapNotRead(t) : gapLook(gap, t)} />
      </p>
      {meaning !== null && <p className="flow-gap-meaning">{meaning}</p>}
      {!unread && gap.cause !== null && (
        <p className="verbatim flow-gap-cause" lang="en">
          {gap.cause}
        </p>
      )}
      {items.length > 0 && (
        <ul className="flow-gap-items">
          {items.map((item) => (
            <GapItem key={item.join("\n")} gap={gap} item={item} marked={marked} onActivate={onActivate} onHand={namesDocuments(gap) || rooted ? onHand : null} />
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
    </li>
  );
}

/**
 * Every row of a fresh read says None: this line stands in place of the rows (docs/features/screen-fit.md).
 * A failed read never shows it: a None from the last read is no None now.
 */
export function GapsClear() {
  const t = useTranslate();
  return (
    <p className="note flow-gaps-clear" role="status">
      <Icon name="checkClean" />
      <span>{t("flow.gapsClear")}</span>
    </p>
  );
}

/**
 * The drawing read's ten rows in the panel (docs/features/writer-studio.md D7), as served, unknown
 * states last. A row changes no status or control (ADR-0012): it says what was found, each name takes
 * you to it, and each measured item can go to your AI as a card (D11), which changes no status
 * either. `unread`: the read's last fetch failed, so no row says a state, an item or a cause.
 */
export function FlowGaps({
  gaps,
  unread,
  rooted,
  marked,
  headingLevel: Heading,
  onActivate,
  onHand,
}: {
  gaps: readonly FlowGap[];
  unread: boolean;
  rooted: boolean;
  marked: ReadonlySet<string>;
  headingLevel: "h2" | "h3";
  onActivate: (item: readonly string[]) => void;
  onHand: (gap: FlowGap, item: readonly string[]) => void;
}) {
  const t = useTranslate();
  const rules = unread ? null : needsRulesLine(gaps, t);
  return (
    <section className="flow-gaps" aria-label={t("flow.gapsHeading")}>
      <Heading className="card-section-title">{t("flow.gapsHeading")}</Heading>
      {rules !== null && (
        <p className="note flow-gaps-rules">
          <Icon name="info" />
          <span>{rules}</span>
        </p>
      )}
      <ul className="flow-gap-list">
        {gapsInOrder(gaps).map((gap) => (
          <GapRow key={gap.code} gap={gap} unread={unread} rooted={rooted} marked={marked} onActivate={onActivate} onHand={onHand} />
        ))}
      </ul>
    </section>
  );
}
