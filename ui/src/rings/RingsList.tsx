import { useId } from "react";
import type { Ring } from "../api/types";
import { DocLink } from "../doc/DocLink";
import { rollupLook } from "../flow/labels";
import { useTranslate } from "../i18n";
import { Badge } from "../ui/Badge";
import { isDimmed, ringWhere } from "./view";

// The picture's text twin (docs/features/scope-rings.md "Acceptance criteria", SR7): a section per
// ring, innermost first, its config's filters in its own words, then its sectors in served order,
// each a link to its document with its kind and status raw and its state as a badge. The search and
// the state chips keep the rows they keep in the picture undimmed.

function RingSection({ project, ring, query, released }: { project: string; ring: Ring; query: string; released: ReadonlySet<string> }) {
  const t = useTranslate();
  const headingId = useId();
  const rows = ring.sectors.filter((sector) => !isDimmed(sector, query, released));
  const where = ringWhere(ring);
  return (
    <section className="rings-list-ring" aria-labelledby={headingId}>
      <h3 id={headingId} className="rings-list-title">
        {ring.title}
      </h3>
      {where !== "" && (
        <p className="mono rings-list-where" lang="en">
          {where}
        </p>
      )}
      <p className="rings-list-count">{t("rings.sectors", { count: rows.length })}</p>
      {rows.length > 0 && (
        <ul className="rings-list-rows">
          {rows.map((sector, index) => (
            <li key={`${sector.name}-${String(index)}`} className="rings-list-row">
              <DocLink project={project} docRef={sector.name}>
                {sector.title ?? sector.name}
              </DocLink>
              <span className="mono rings-list-kind">{sector.kind ?? t("graph.noKind")}</span>
              <Badge look={rollupLook(sector.state, t)} />
              {sector.status !== null && (
                <span className="rings-list-status" lang="en">
                  {sector.status}
                </span>
              )}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

export function RingsList({ project, rings, query, released }: { project: string; rings: readonly Ring[]; query: string; released: ReadonlySet<string> }) {
  return (
    <div className="rings-list">
      {rings.map((ring, index) => (
        <RingSection key={index} project={project} ring={ring} query={query} released={released} />
      ))}
    </div>
  );
}
