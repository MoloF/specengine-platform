import { useTranslate } from "../i18n";
import { Badge } from "../ui/Badge";
import { Icon } from "../ui/Icon";
import { countsText, dashOf, FLOW_KINDS, rollupMeanings, startWord, typeColour, typeDash } from "./labels";

/** A stroke as the canvas draws it. */
function Sample({ dash, colour }: { dash: string; colour: string }) {
  return (
    <svg className="flow-sample" viewBox="0 0 48 8" width="48" height="8" aria-hidden="true" focusable="false">
      <line x1="1" y1="4" x2="47" y2="4" style={{ stroke: colour, strokeDasharray: dash }} />
    </svg>
  );
}

/** An edge of a kind, as its dash is looked up: the colour stays the edge's own. */
const kindDashOf = (kind: string) => dashOf({ from: "", to: "", type: "", kind, when: null, count: 1, path: "", line: 0 }, []);

/** The sample a group's counts make. */
const SAMPLE_COUNTS = { no_card: 0, in_work: 0, suspect: 1, departed: 0, confirmed: 3 };

/**
 * The canvas's legend (docs/features/writer-studio.md "Description and interactions"): each type and kind with its
 * sample, in the daemon's words; what an arc means; the marks; what each state of a box's work says,
 * when a read served any, so a flow drawn bare names no state; the drawing library's credit. It lies
 * over the canvas from its bottom edge and scrolls inside.
 */
export function FlowLegend({ id, types, states }: { id: string; types: readonly string[]; states: boolean }) {
  const t = useTranslate();
  return (
    <section id={id} className="graph-legend flow-legend" aria-label={t("flow.legendButton")}>
      <p className="graph-legend-about">{t("flow.legend.about")}</p>
      <ul className="graph-legend-list">
        {types.map((type) => (
          <li key={type} className="graph-legend-item">
            <Sample dash={typeDash(type, types)} colour={typeColour(type, types)} />
            <span className="mono">{type}</span>
          </li>
        ))}
        {FLOW_KINDS.map((kind) => (
          <li key={kind} className="graph-legend-item">
            <Sample dash={kindDashOf(kind)} colour="var(--graph-edge)" />
            <span className="mono">{kind}</span>
          </li>
        ))}
        <li className="graph-legend-item">
          <svg className="flow-sample" viewBox="0 0 48 14" width="48" height="14" aria-hidden="true" focusable="false">
            <path d="M47,2 H42 Q38,2 38,6 V8 Q38,12 34,12 H14 Q10,12 10,8 V6 Q10,2 6,2 H1" style={{ stroke: "var(--graph-edge)" }} />
          </svg>
          <span>{t("flow.legend.arc")}</span>
        </li>
      </ul>
      <ul className="graph-legend-list">
        <li className="graph-legend-item">
          <span className="flow-mark flow-mark-start">
            <Icon name="flowStart" />
            {startWord(false, t)}
          </span>
          <span>{t("flow.legend.start")}</span>
        </li>
        <li className="graph-legend-item">
          <span className="flow-mark flow-mark-end">
            <Icon name="flowEnd" />
            {t("flow.end")}
          </span>
          <span>{t("flow.legend.end")}</span>
        </li>
        <li className="graph-legend-item">
          <span className="flow-mark flow-mark-gap">
            <Icon name="flowGap" />
            {t("flow.gapMark.shown")}
          </span>
          <span>{t("flow.legend.gap")}</span>
        </li>
        <li className="graph-legend-item">
          <span className="flow-mark">
            <Icon name="chevronRight" />
            {t("flow.legend.groupWord")}
          </span>
          <span>{t("flow.legend.group")}</span>
        </li>
      </ul>
      {states && (
        <ul className="graph-legend-list">
          {rollupMeanings(t).map(({ word, look, meaning }) => (
            <li key={word} className="graph-legend-item">
              <Badge look={look} />
              <span>{meaning}</span>
            </li>
          ))}
          <li className="graph-legend-item">
            <span className="flow-mark">{countsText(SAMPLE_COUNTS, t)}</span>
            <span>{t("flow.legend.counts")}</span>
          </li>
        </ul>
      )}
      <p className="graph-legend-credit">{t("flow.legend.credit")}</p>
    </section>
  );
}
