import { memo, useId } from "react";
import { useTranslate } from "../i18n";
import type { KindColours } from "./colours";

// The map's legend (docs/features/corpus-map.md "Acceptance criteria", CM7): each kind that takes a
// colour, in slot order (most held first), its swatch, its name as served and how many nodes of the
// set hold it; then the grey's "Other kinds" with its count; then the marks as the canvas draws them.
// A colour never stands without these words.

/** A line's or a mark's look, drawn as the canvas draws it. */
function Mark({ kind }: { kind: "orphan" | "ghost" | "strong" | "mention" | "parent" }) {
  return (
    <svg className={`map-mark map-mark-${kind}`} viewBox="0 0 28 14" width="28" height="14" aria-hidden="true" focusable="false">
      {kind === "orphan" || kind === "ghost" ? <circle cx="14" cy="7" r={kind === "orphan" ? 5.5 : 3.5} /> : <line x1="1" y1="7" x2="27" y2="7" />}
      {kind === "orphan" && <circle className="map-mark-ring" cx="14" cy="7" r="6.5" />}
    </svg>
  );
}

export const MapLegend = memo(function MapLegend({ colours }: { colours: KindColours }) {
  const t = useTranslate();
  const headingId = useId();
  return (
    <section className="map-legend" aria-labelledby={headingId}>
      <h3 id={headingId} className="map-legend-title">
        {t("graph.legend.title")}
      </h3>
      <ul className="map-legend-list" aria-label={t("map.kinds")}>
        {colours.coloured.map((each) => (
          <li key={each.kind} className="map-legend-item">
            <span className={`map-swatch map-slot-${String(each.slot)}`} aria-hidden="true" />
            <span className="mono map-legend-kind">{each.kind}</span>
            <span className="map-legend-count">{t("map.legendCount", { count: each.count })}</span>
          </li>
        ))}
        <li className="map-legend-item">
          <span className="map-swatch map-slot-0" aria-hidden="true" />
          <span className="map-legend-kind">{t("map.otherKinds")}</span>
          <span className="map-legend-count">{t("map.legendCount", { count: colours.other })}</span>
        </li>
      </ul>
      <ul className="map-legend-list">
        <li className="map-legend-item">
          <Mark kind="orphan" />
          <span>{t("map.marks.orphan")}</span>
        </li>
        <li className="map-legend-item">
          <Mark kind="ghost" />
          <span>{t("map.marks.ghost")}</span>
        </li>
        <li className="map-legend-item">
          <Mark kind="strong" />
          <span>{t("map.marks.strong")}</span>
        </li>
        <li className="map-legend-item">
          <Mark kind="mention" />
          <span>{t("map.marks.mention")}</span>
        </li>
        <li className="map-legend-item">
          <Mark kind="parent" />
          <span>{t("map.marks.parent")}</span>
        </li>
      </ul>
    </section>
  );
});
