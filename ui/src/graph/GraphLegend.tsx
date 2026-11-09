import type { FollowedType } from "../api/types";
import { Icon } from "../ui/Icon";
import { patternAt } from "./layout";
import { directionIcon, followedLabel } from "./lines";
import { useTranslate } from "../i18n";

/** A type's stroke pattern, drawn as the canvas draws it. */
export function PatternSwatch({ position }: { position: number }) {
  return (
    <svg className="graph-pattern" viewBox="0 0 40 8" width="40" height="8" aria-hidden="true" focusable="false">
      <line x1="1" y1="4" x2="39" y2="4" style={{ strokeDasharray: patternAt(position) }} />
    </svg>
  );
}

/**
 * The canvas's legend (docs/features/ui-graph.md "Implementation", Legend): what an arrow and a
 * column mean; each followed type in the answer's order with its pattern (by its place in the
 * mode's chip `order`, as the canvas draws it), direction icon and words; the marks; the drawing
 * library's credit.
 */
export function GraphLegend({ types, order }: { types: readonly FollowedType[]; order: readonly string[] }) {
  const t = useTranslate();
  return (
    <section className="graph-legend" aria-label={t("graph.legend.title")}>
      <h2 className="card-section-title">{t("graph.legend.title")}</h2>
      <p className="graph-legend-about">{t("graph.legend.about")}</p>
      <ul className="graph-legend-list">
        {types.map((followed) => (
          <li key={followed.type} className="graph-legend-item">
            <PatternSwatch position={order.indexOf(followed.type)} />
            <Icon name={directionIcon(followed.direction)} />
            <span className="mono">{followedLabel(followed.type, followed.direction, t)}</span>
          </li>
        ))}
      </ul>
      <ul className="graph-legend-list">
        <li className="graph-legend-item">
          <span className="graph-mark graph-mark-focus">
            <Icon name="focus" />
            {t("graph.focus")}
          </span>
          <span>{t("graph.legend.focus")}</span>
        </li>
        <li className="graph-legend-item">
          <span className="graph-mark graph-mark-selected">
            <Icon name="selected" />
            {t("graph.selected")}
          </span>
          <span>{t("graph.legend.selected")}</span>
        </li>
        <li className="graph-legend-item">
          <span className="graph-box-flag">
            <Icon name="archived" />
            {t("graph.archivedWord")}
          </span>
          <span>{t("graph.legend.archived")}</span>
        </li>
        <li className="graph-legend-item">
          <span className="graph-box-flag graph-legend-stub">
            <Icon name="stub" />
            {t("graph.notWalked")}
          </span>
          <span>{t("graph.legend.notWalked")}</span>
        </li>
        <li className="graph-legend-item">
          <span className="graph-box-flag">
            <Icon name="more" />
            {t("graph.legend.moreMark")}
          </span>
          <span>{t("graph.legend.more")}</span>
        </li>
      </ul>
      <p className="graph-legend-credit">{t("graph.legend.credit")}</p>
    </section>
  );
}
