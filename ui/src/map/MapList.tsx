import { memo, useId, useRef, useState } from "react";
import type { MapDocument } from "../api/types";
import { DocLink } from "../doc/DocLink";
import { useTranslate } from "../i18n";
import { Badge } from "../ui/Badge";
import { Button } from "../ui/Button";
import { useFocusLater } from "../ui/useFocusLater";
import { directionOf, type MapDirection } from "./colours";
import { MAP_LIST_PAGE, type ListRow } from "./filters";

// The picture's text twin (docs/features/corpus-map.md "Acceptance criteria", CM8, CM9): every node
// the filters keep, drawn or capped out, in served order, MAP_LIST_PAGE at a time. Each row names the
// node (its title, else its name, the name in mono) as a link to its document, its kind with its
// direction's title, its uses as served, its links as drawn out and in, and its marks in words.
// "Show N more" moves focus to the first row it shows, so the last page never drops it on the page.
// Then the ghosts the picture keeps, as its ghost card tells them: what is written, as text; the
// daemon's reason, in English; its places; each source a link to its document.

/** The List's Broken links: each kept ghost, its words as served, each source a document link. */
function GhostTable({ project, answer, ghosts }: { project: string; answer: MapDocument; ghosts: readonly number[] }) {
  const t = useTranslate();
  const headingId = useId();
  return (
    <section className="map-ghosts" aria-labelledby={headingId}>
      <h3 id={headingId} className="map-ghosts-title">
        {t("map.brokenLinks")}
      </h3>
      <div className="map-table-wrap">
        <table className="map-table">
          <thead>
            <tr>
              <th scope="col">{t("map.ghost.written")}</th>
              <th scope="col">{t("map.ghost.reason")}</th>
              <th scope="col">{t("map.ghost.placesColumn")}</th>
              <th scope="col">{t("map.ghost.sources")}</th>
            </tr>
          </thead>
          <tbody>
            {ghosts.map((at) => {
              const ghost = answer.ghosts[at];
              if (ghost === undefined) {
                return null;
              }
              return (
                <tr key={at}>
                  <td className="mono map-ghost-written">{ghost.written}</td>
                  <td>{ghost.reason !== null && <span lang="en">{ghost.reason}</span>}</td>
                  <td className="map-ghost-places">{t("map.ghost.places", { count: ghost.places })}</td>
                  <td>
                    <ul className="map-ghost-sources">
                      {ghost.sources.map((source) => {
                        const node = answer.nodes[source];
                        if (node === undefined) {
                          return null;
                        }
                        return (
                          <li key={source}>
                            <DocLink project={project} docRef={node.name}>
                              {node.title ?? node.name}
                            </DocLink>
                          </li>
                        );
                      })}
                    </ul>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </section>
  );
}

export const MapList = memo(function MapList({
  project,
  answer,
  rows,
  ghosts,
  directions,
}: {
  project: string;
  answer: MapDocument;
  rows: readonly ListRow[];
  /** The ghosts the filters keep, by their place in `answer.ghosts`. */
  ghosts: readonly number[];
  directions: readonly MapDirection[];
}) {
  const t = useTranslate();
  const focusLater = useFocusLater();
  const body = useRef<HTMLTableSectionElement>(null);
  const [shown, setShown] = useState(MAP_LIST_PAGE);
  const visible = rows.slice(0, shown);
  const rest = rows.length - visible.length;

  function more() {
    const first = visible.length;
    setShown(first + MAP_LIST_PAGE);
    focusLater(() => body.current?.rows.item(first)?.querySelector("a"));
  }

  return (
    <div className="map-list">
      <div className="map-table-wrap">
        <table className="map-table">
          <thead>
            <tr>
              <th scope="col">{t("map.column.name")}</th>
              <th scope="col">{t("graph.details.kind")}</th>
              <th scope="col" className="map-number">
                {t("map.column.usedBy")}
              </th>
              <th scope="col" className="map-number">
                {t("map.column.mentionedBy")}
              </th>
              <th scope="col" className="map-number">
                {t("map.column.links")}
              </th>
              <th scope="col">{t("map.column.marks")}</th>
            </tr>
          </thead>
          <tbody ref={body}>
            {visible.map((row) => {
              const node = answer.nodes[row.index];
              if (node === undefined) {
                return null;
              }
              const direction = directionOf(node.kind, directions);
              return (
                <tr key={row.index}>
                  <td className="map-name">
                    <DocLink project={project} docRef={node.name}>
                      {node.title ?? node.name}
                    </DocLink>
                    {node.title !== null && <span className="mono map-name-id">{node.name}</span>}
                  </td>
                  <td>
                    {direction !== undefined && <span className="map-kind-direction">{direction.title}</span>}
                    <span className="mono">{node.kind ?? t("map.noKind")}</span>
                  </td>
                  <td className="map-number">{node.used_by}</td>
                  <td className="map-number">{node.mentioned_by}</td>
                  <td className="map-number">{`${String(row.out)} · ${String(row.in)}`}</td>
                  <td>
                    <span className="map-marks">
                      {node.orphan === true && <Badge look={{ label: t("map.orphan"), tone: "severity-unknown", icon: "info" }} />}
                      {row.broken > 0 && <Badge look={{ label: t("map.broken", { count: row.broken }), tone: "link-dangling", icon: "linkDangling" }} />}
                    </span>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
      {rest > 0 && <Button onClick={more}>{t("map.more", { rows: Math.min(MAP_LIST_PAGE, rest) })}</Button>}
      {ghosts.length > 0 && <GhostTable project={project} answer={answer} ghosts={ghosts} />}
    </div>
  );
});
