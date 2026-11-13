import { useId, useMemo, useRef, useState, type FocusEvent, type KeyboardEvent, type MouseEvent, type PointerEvent } from "react";
import type { Ring, RingSector } from "../api/types";
import { openDocFrom } from "../app/location";
import { rollupLook, ROLLUP_WORDS } from "../flow/labels";
import { CLICK_DISTANCE } from "../graph/geometry";
import { useTranslate, type Translate } from "../i18n";
import { Badge } from "../ui/Badge";
import { Icon } from "../ui/Icon";
import { hasModifier } from "../ui/keys";
import { drawnRings, sectorPath, VIEW_SIZE } from "./geometry";
import { firstSector, isDimmed, isWalkKey, sectorAt, statesOf, walk, type SectorAt } from "./view";

// The rings drawn (docs/features/scope-rings.md "Acceptance criteria", SR7): plain SVG, one ring
// per served ring, innermost first, its sectors in served order at equal angles, each filled by its
// state's tone as src/flow/labels.ts looks it up. The drawing is one tab stop: the sectors are links
// with a roving tabindex, named in words; arrows walk them, Enter or a press of at most
// CLICK_DISTANCE px opens the document, and so does a click no press began (assistive technology's,
// a script's). Nothing takes the wheel, pans or zooms: the drawing fits its box. Hovering or focusing
// a sector shows its card over the drawing's corner, out of the page's flow, so the legend never
// moves; the legend names each state and ring.

/** The viewBox, square. */
const VIEW_BOX = `0 0 ${String(VIEW_SIZE)} ${String(VIEW_SIZE)}`;

function keyOf(at: SectorAt): string {
  return `${String(at.ring)}:${String(at.index)}`;
}

/** Each sector's React key in a ring: its name, then how many before it share that name. */
export function sectorKeys(sectors: readonly { sector: RingSector }[]): string[] {
  const seen = new Map<string, number>();
  return sectors.map(({ sector }) => {
    const before = seen.get(sector.name) ?? 0;
    seen.set(sector.name, before + 1);
    return `${sector.name}\u0000${String(before)}`;
  });
}

/** A sector's accessible name: its title, else its name; its kind; its state in words; its ring. */
export function sectorName(sector: RingSector, ring: Ring, t: Translate): string {
  return t("rings.sector", { title: sector.title ?? sector.name, kind: sector.kind ?? t("graph.noKind"), state: rollupLook(sector.state, t).label, ring: ring.title });
}

/** What a hovered or focused sector shows: its title, kind, state, raw status and place. */
function SectorCard({ sector }: { sector: RingSector }) {
  const t = useTranslate();
  return (
    <div className="rings-card">
      <p className="rings-card-title">{sector.title ?? sector.name}</p>
      <p className="rings-card-facts">
        <span className="mono">{sector.kind ?? t("graph.noKind")}</span>
        <Badge look={rollupLook(sector.state, t)} />
        {sector.status !== null && (
          <span className="rings-card-status" lang="en">
            {sector.status}
          </span>
        )}
      </p>
      <p className="mono rings-card-place">{`${sector.path}:${String(sector.line)}`}</p>
    </div>
  );
}

/** Each state's swatch, icon and label, then the rings innermost first with their sector counts. */
function RingsLegend({ rings }: { rings: readonly Ring[] }) {
  const t = useTranslate();
  const headingId = useId();
  const words = [...ROLLUP_WORDS, ...statesOf(rings).map((entry) => entry.state).filter((state) => !ROLLUP_WORDS.includes(state))];
  return (
    <section className="rings-legend" aria-labelledby={headingId}>
      <h3 id={headingId} className="rings-legend-title">
        {t("rings.legend")}
      </h3>
      <ul className="rings-legend-states" aria-label={t("rings.states")}>
        {words.map((word) => {
          const look = rollupLook(word, t);
          return (
            <li key={word} className={`tone-${look.tone}`}>
              <span className="rings-swatch" aria-hidden="true" />
              <Icon name={look.icon} />
              <span lang={look.served === true ? "en" : undefined}>{look.label}</span>
            </li>
          );
        })}
      </ul>
      <ol className="rings-legend-rings" aria-label={t("rings.title")}>
        {rings.map((ring, index) => (
          <li key={index}>
            <span className="rings-legend-ring">{ring.title}</span> <span className="rings-legend-count">{t("rings.sectors", { count: ring.sectors.length })}</span>
          </li>
        ))}
      </ol>
    </section>
  );
}

export function RingsPicture({ rings, query, released }: { rings: readonly Ring[]; query: string; released: ReadonlySet<string> }) {
  const t = useTranslate();
  const ids = useId();
  const drawn = useMemo(() => drawnRings(rings), [rings]);
  const keys = useMemo(() => drawn.map((ring) => sectorKeys(ring.sectors)), [drawn]);
  const [roving, setRoving] = useState<SectorAt | null>(null);
  const [hovered, setHovered] = useState<SectorAt | null>(null);
  const [focused, setFocused] = useState(false);
  const elements = useRef(new Map<string, SVGPathElement>());
  const pressedAt = useRef<{ x: number; y: number } | null>(null);

  // The roving place: the last one walked to or focused while the answer still has it, else the first.
  const at = roving !== null && sectorAt(rings, roving) !== undefined ? roving : firstSector(rings);
  const card = sectorAt(rings, hovered ?? (focused ? at : null));
  const focusPath = focused && at !== null ? drawn[at.ring]?.sectors[at.index]?.path : undefined;

  function open(place: SectorAt) {
    const sector = sectorAt(rings, place);
    if (sector !== undefined) {
      openDocFrom(sector.name, elements.current.get(keyOf(place)) ?? null);
    }
  }

  function onKeyDown(event: KeyboardEvent<SVGSVGElement>) {
    if (event.defaultPrevented || hasModifier(event) || at === null) {
      return;
    }
    if (event.key === "Enter") {
      event.preventDefault();
      open(at);
      return;
    }
    if (!isWalkKey(event.key)) {
      return;
    }
    event.preventDefault();
    const next = walk(rings, at, event.key);
    setRoving(next);
    elements.current.get(keyOf(next))?.focus();
  }

  function onBlur(event: FocusEvent<SVGSVGElement>) {
    if (!(event.relatedTarget instanceof Node && event.currentTarget.contains(event.relatedTarget))) {
      setFocused(false);
    }
  }

  function onPointerDown(event: PointerEvent<SVGPathElement>) {
    pressedAt.current = { x: event.clientX, y: event.clientY };
  }

  /**
   * A click opens the sector's document: one no press began (assistive technology's) and a press that
   * moved at most CLICK_DISTANCE px; a recorded press that moved farther is a drag and opens nothing.
   */
  function onClick(place: SectorAt, event: MouseEvent<SVGPathElement>) {
    const from = pressedAt.current;
    pressedAt.current = null;
    if (from !== null && Math.hypot(event.clientX - from.x, event.clientY - from.y) > CLICK_DISTANCE) {
      return;
    }
    setRoving(place);
    open(place);
  }

  return (
    <div className="rings-stage">
      <div className="rings-picture">
        {/* A press that ends off its sector, or leaves the drawing, is no one's: a later bare click is a click. */}
        <svg
          className="rings-svg"
          viewBox={VIEW_BOX}
          role="group"
          aria-label={t("rings.title")}
          onKeyDown={onKeyDown}
          onBlur={onBlur}
          onClick={() => {
            pressedAt.current = null;
          }}
          onPointerLeave={() => {
            pressedAt.current = null;
          }}
        >
          <defs>
            {drawn.map((ring, r) => (
              <path key={r} id={`${ids}-title-${String(r)}`} d={ring.titlePath} />
            ))}
          </defs>
          {drawn.map((ring, r) => (
            <g key={r} className="rings-ring">
              {ring.sectors.length === 0 && <path className="rings-band" d={sectorPath(ring.band, 0, 1)} aria-hidden="true" />}
              {ring.sectors.map(({ sector, path }, index) => {
                const place = { ring: r, index };
                const look = rollupLook(sector.state, t);
                const roved = at !== null && at.ring === r && at.index === index;
                return (
                  <path
                    key={keys[r]?.[index] ?? index}
                    ref={(element) => {
                      if (element === null) {
                        elements.current.delete(keyOf(place));
                      } else {
                        elements.current.set(keyOf(place), element);
                      }
                    }}
                    d={path}
                    role="link"
                    tabIndex={roved ? 0 : -1}
                    aria-label={sectorName(sector, ring.ring, t)}
                    className={`rings-sector tone-${look.tone}${isDimmed(sector, query, released) ? " is-outlined" : ""}`}
                    data-tone={look.tone}
                    onPointerDown={onPointerDown}
                    onClick={(event) => {
                      onClick(place, event);
                    }}
                    onPointerEnter={() => {
                      setHovered(place);
                    }}
                    onPointerLeave={() => {
                      setHovered(null);
                    }}
                    onFocus={() => {
                      setRoving(place);
                      setFocused(true);
                    }}
                  />
                );
              })}
            </g>
          ))}
          <g className="rings-words" aria-hidden="true">
            {drawn.map((ring, r) => (
              <g key={r}>
                {ring.sectors.map(({ sector, label }, index) =>
                  label === null ? null : (
                    <text
                      key={keys[r]?.[index] ?? index}
                      className={`rings-label${isDimmed(sector, query, released) ? " is-outlined" : ""}`}
                      x={label.x}
                      y={label.y}
                      transform={`rotate(${String(label.turn)} ${String(label.x)} ${String(label.y)})`}
                    >
                      {label.text}
                    </text>
                  ),
                )}
                <text className="rings-title">
                  <textPath href={`#${ids}-title-${String(r)}`} startOffset="50%">
                    {ring.title}
                  </textPath>
                </text>
                {ring.sectors.length === 0 && (
                  <text className="rings-count" x={ring.countAt.x} y={ring.countAt.y}>
                    {0}
                  </text>
                )}
              </g>
            ))}
            {focusPath !== undefined && <path className="rings-focus" d={focusPath} />}
          </g>
        </svg>
        {card !== undefined && (
          <div className="rings-overlay">
            <SectorCard sector={card} />
          </div>
        )}
      </div>
      <div className="rings-side">
        <RingsLegend rings={rings} />
      </div>
    </div>
  );
}
