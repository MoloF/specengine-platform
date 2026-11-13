import { memo, useCallback, useLayoutEffect, useMemo, useRef, useState, type ReactNode, type RefObject } from "react";
import type { MapDocument } from "../api/types";
import { openDocFrom, pushHash } from "../app/location";
import { sectionHash } from "../app/routes";
import { useTranslate } from "../i18n";
import { Button } from "../ui/Button";
import { directionOf, type MapDirection } from "./colours";
import type { MapBox, MapPicture, MapView } from "./draw";
import { litOf, type Kept, type Lit, type Scene } from "./filters";
import type { HitGrid } from "./hit";
import { MapCanvas, type MapCommand } from "./MapCanvas";

// The map's stage (docs/features/corpus-map.md "Acceptance criteria", CM7, CM12): the canvas and the
// cards over it. The hovered point is this component's own state, set only when the point under the
// pointer changes, so a hover renders the stage and nothing around it: never the controls, the
// legend or the List. Each card is an overlay inside the canvas's box, never in the page's flow, so
// showing one moves nothing: no layout shift, no resize, no base drawn again. The lit node's pane,
// with its three acts, stays while the node is lit; a hover card shows beside it, never in its
// place, and lets the pointer through to the canvas. A ghost's hover card tells what is written, the
// daemon's reason, its places and its sources. When the pane goes with focus in it (Esc, a new answer
// without the node), focus goes back to the canvas's region, never to the page.

/** How many of a ghost's sources its hover card names; the List names them all. */
export const GHOST_CARD_SOURCES = 8;

/** A scene laid out: its positions and the answer it was laid out from. */
export interface Laid {
  scene: Scene;
  answer: MapDocument;
  positions: Float64Array;
}

/** A node's card: its title, else its name; its kind in words; its uses as served; Orphan; its place. */
function NodeCard({ answer, index, directions, children }: { answer: MapDocument; index: number; directions: readonly MapDirection[]; children?: ReactNode }) {
  const t = useTranslate();
  const node = answer.nodes[index];
  if (node === undefined) {
    return null;
  }
  const direction = directionOf(node.kind, directions);
  return (
    <div className="map-card">
      <p className="map-card-title">{node.title ?? node.name}</p>
      <p className="map-card-facts">
        {direction !== undefined && <span>{direction.title}</span>}
        <span className="mono">{node.kind ?? t("map.noKind")}</span>
        {node.orphan === true && <span className="map-card-orphan">{t("map.orphan")}</span>}
      </p>
      <p>{t("map.usage", { used: node.used_by, mentioned: node.mentioned_by })}</p>
      <p className="mono map-card-place">{`${node.path}:${String(node.line)}`}</p>
      {children}
    </div>
  );
}

/** A ghost's card: what is written, as text; the daemon's reason, in English; its places; its first sources by title. */
function GhostCard({ answer, ghost }: { answer: MapDocument; ghost: number }) {
  const t = useTranslate();
  const served = answer.ghosts[ghost];
  if (served === undefined) {
    return null;
  }
  const named = served.sources.slice(0, GHOST_CARD_SOURCES);
  const rest = served.sources.length - named.length;
  return (
    <div className="map-card">
      <p className="map-card-facts">{t("map.marks.ghost")}</p>
      <p className="mono map-card-title">{served.written}</p>
      {served.reason !== null && <p lang="en">{served.reason}</p>}
      <p>{t("map.ghost.places", { count: served.places })}</p>
      <p className="map-card-facts">{t("map.ghost.sources")}</p>
      <ul className="map-card-sources">
        {named.map((source) => {
          const node = answer.nodes[source];
          return node === undefined ? null : <li key={source}>{node.title ?? node.name}</li>;
        })}
        {rest > 0 && <li>{t("map.ghost.more", { count: rest })}</li>}
      </ul>
    </div>
  );
}

/**
 * The lit node's pane and its acts. When it goes while focus is in it, focus moves to the canvas's
 * region first: its layout cleanup runs before its box leaves the page, so focus never falls to it.
 */
function LitPane({ project, answer, index, name, directions, region }: { project: string; answer: MapDocument; index: number; name: string; directions: readonly MapDirection[]; region: RefObject<HTMLDivElement | null> }) {
  const t = useTranslate();
  const pane = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    // The region is the canvas's one box for its whole life, there before the pane.
    const element = pane.current;
    const back = region.current;
    return () => {
      if (element !== null && element.contains(document.activeElement)) {
        back?.focus();
      }
    };
  }, [region]);
  return (
    <div ref={pane} className="map-overlay">
      <NodeCard answer={answer} index={index} directions={directions}>
        <div className="map-card-acts">
          <Button
            spacing="compact"
            onClick={(event) => {
              openDocFrom(name, event.currentTarget);
            }}
          >
            {t("map.open")}
          </Button>
          <Button
            spacing="compact"
            onClick={() => {
              pushHash(sectionHash(project, "graph", name));
            }}
          >
            {t("map.inGraph")}
          </Button>
          <Button
            spacing="compact"
            onClick={() => {
              pushHash(sectionHash(project, "tree", name));
            }}
          >
            {t("map.inTree")}
          </Button>
        </div>
      </NodeCard>
    </div>
  );
}

export interface MapStageProps {
  project: string;
  /** The answer read last: the lit node's pane reads it. */
  answer: MapDocument;
  /** The scene shown and the answer it was laid out from: the hover card reads that one. */
  laid: Laid;
  picture: MapPicture;
  kept: Kept;
  grid: HitGrid;
  directions: readonly MapDirection[];
  /** What the search or the lit node lights, when nothing is hovered. */
  lit: Lit | null;
  /** The lit node in `answer`, for its pane; null when none. */
  litIndex: number | null;
  /** The search's current match, else the lit node: where the focus ring goes when nothing is hovered. */
  searchFocus: number | null;
  litFocus: number | null;
  view: MapView;
  label: string;
  describedBy: string;
  onView: (view: MapView) => void;
  onCommand: (command: MapCommand) => void;
  onBox: (box: Pick<MapBox, "width" | "height">) => void;
}

export const MapStage = memo(function MapStage({
  project,
  answer,
  laid,
  picture,
  kept,
  grid,
  directions,
  lit,
  litIndex,
  searchFocus,
  litFocus,
  view,
  label,
  describedBy,
  onView,
  onCommand,
  onBox,
}: MapStageProps) {
  const region = useRef<HTMLDivElement>(null);
  const [hovered, setHovered] = useState<{ scene: Scene; point: number } | null>(null);
  const scene = laid.scene;
  // A pointer still over the same point changes nothing: no render.
  const onHover = useCallback(
    (point: number | null) => {
      setHovered((current) => {
        if (point === null) {
          return null;
        }
        return current !== null && current.scene === scene && current.point === point ? current : { scene, point };
      });
    },
    [scene],
  );
  const onOpen = useCallback(
    (point: number, opener: HTMLElement) => {
      const index = scene.points[point]?.node ?? null;
      const node = index === null ? undefined : laid.answer.nodes[index];
      if (node !== undefined) {
        openDocFrom(node.name, opener);
      }
    },
    [scene, laid],
  );
  const shown = hovered !== null && hovered.scene === scene ? hovered.point : null;
  const shownLit = useMemo(() => (shown === null ? lit : litOf(scene, kept, shown)), [shown, lit, scene, kept]);
  const focus = searchFocus ?? shown ?? litFocus;
  const hoveredPoint = shown === null ? undefined : scene.points[shown];
  const litName = litIndex === null ? null : (answer.nodes[litIndex]?.name ?? null);

  // The hover card: a node's (unless it is the lit one, whose pane shows), else a ghost's.
  let card: ReactNode = null;
  if (hoveredPoint !== undefined) {
    if (hoveredPoint.node === null) {
      card = <GhostCard answer={laid.answer} ghost={hoveredPoint.ghost} />;
    } else if (laid.answer.nodes[hoveredPoint.node]?.name !== litName) {
      card = <NodeCard answer={laid.answer} index={hoveredPoint.node} directions={directions} />;
    }
  }
  const paneShown = litIndex !== null && litName !== null;

  return (
    <div className="map-canvas-wrap">
      <MapCanvas
        regionRef={region}
        picture={picture}
        kept={kept}
        lit={shownLit}
        focus={focus !== null && focus >= 0 ? focus : null}
        view={view}
        grid={grid}
        label={label}
        describedBy={describedBy}
        onView={onView}
        onCommand={onCommand}
        onHover={onHover}
        onOpen={onOpen}
        onBox={onBox}
      />
      {paneShown && <LitPane project={project} answer={answer} index={litIndex} name={litName} directions={directions} region={region} />}
      {card !== null && <div className={`map-overlay is-passive${paneShown ? " is-below" : ""}`}>{card}</div>}
    </div>
  );
});
