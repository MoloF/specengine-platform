import { useCallback, useEffect, useId, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { useMap, useWorkspaces } from "../api/queries";
import type { MapDocument } from "../api/types";
import { openDocFrom } from "../app/location";
import { useTranslate, type Translate } from "../i18n";
import { useAnnounce } from "../ui/announcer";
import { Disclosure } from "../ui/Disclosure";
import { EmptyState } from "../ui/EmptyState";
import { ReadFailure, Skeleton } from "../ui/states";
import { TabPanel, Tabs } from "../ui/Tabs";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { useRetryFocus } from "../ui/useRetryFocus";
import { directionsOf, kindColours, slotOf, type KindColours, type MapDirection } from "./colours";
import { clampZoom, fitView, GHOST_RADIUS, labelledOf, labelOf, radiusOf, type MapBox, type MapPicture, type MapView as View } from "./draw";
import { DEFAULT_FILTERS, ghostsOf, keepsNode, keptOf, kindChips, kindsOf, listOf, litMatches, litOf, matchesOf, pointsByName, releasedFor, sceneOf, type MapFilters, type Scene } from "./filters";
import { gridOf } from "./hit";
import { runSlice, startLayout } from "./layout";
import { UNMEASURED, ZOOM_STEP, type MapCommand } from "./MapCanvas";
import { MapControls, type MapSwitch } from "./MapControls";
import { MapLegend } from "./MapLegend";
import { MapList } from "./MapList";
import { MapStage, type Laid } from "./MapStage";

// The Graph's Map (docs/features/corpus-map.md "Acceptance criteria", CM6 to CM9): the whole live
// corpus as one picture, read from the daemon and laid out here by d3-force, never counted here. A
// chunk of its own, reached from graph/GraphHome.tsx by a dynamic import alone (CM11). The answer and
// the Sections switch make the node set, laid out in slices between frames with a progress line;
// every other filter hides and keeps positions; a new answer lays out again, keeping zoom and pan.
// The search dims all but its matches; the arrows step through them, Enter centres and lights one,
// Enter again opens it; a match lights its node in the picture shown by name, never by an index of
// another answer. Picture and List show the same kept nodes and ghosts. A hover is the stage's own
// state (MapStage): it renders nothing here, and the List is mounted only on its tab, its paging
// reset by any filter. A node's colour is its kind's, by how many nodes of the set hold it
// (colours.ts); the legend names each. Mentions start hidden. The notes show MAP_NOTES_SHOWN, the
// rest behind "Show all N".

type MapTab = "picture" | "list";

/** How many of the answer's notes show before "Show all N". */
export const MAP_NOTES_SHOWN = 3;

/** The next frame, or a timer where there is none; returns its cancel. */
function later(callback: () => void): () => void {
  if (typeof window.requestAnimationFrame === "function") {
    const handle = window.requestAnimationFrame(callback);
    return () => {
      window.cancelAnimationFrame(handle);
    };
  }
  const handle = window.setTimeout(callback, 0);
  return () => {
    window.clearTimeout(handle);
  };
}

/** Lays a scene out in slices between frames; the last laid scene stays until the next is done. */
function useLaidOut(scene: Scene | null, answer: MapDocument | undefined): Laid | null {
  const [laid, setLaid] = useState<Laid | null>(null);
  useEffect(() => {
    if (scene === null || answer === undefined) {
      return;
    }
    const layout = startLayout(
      scene.points.length,
      Array.from(scene.lineFrom, (from, at) => ({ source: from, target: scene.lineTo[at] ?? from })),
    );
    let cancel: () => void = () => undefined;
    const slice = () => {
      runSlice(layout, () => performance.now());
      if (layout.done) {
        setLaid({ scene, answer, positions: layout.positions() });
      } else {
        cancel = later(slice);
      }
    };
    cancel = later(slice);
    return () => {
      cancel();
    };
  }, [scene, answer]);
  return laid;
}

/** What the drawing reads of a laid scene: discs, colours (by kind, so a newer answer's colouring holds), marks and words per point. */
function pictureOf(laid: Laid, colours: KindColours): MapPicture {
  const { scene, answer } = laid;
  const count = scene.points.length;
  const radii = new Float64Array(count);
  const slots = new Uint8Array(count);
  const ghost = new Uint8Array(count);
  const orphan = new Uint8Array(count);
  const labels: string[] = [];
  scene.points.forEach((point, at) => {
    if (point.node === null) {
      ghost[at] = 1;
      radii[at] = GHOST_RADIUS;
      labels.push(labelOf(answer.ghosts[point.ghost]?.written ?? ""));
      return;
    }
    const node = answer.nodes[point.node];
    radii[at] = radiusOf(node?.used_by ?? 0);
    slots[at] = slotOf(node?.kind ?? null, colours);
    orphan[at] = node?.orphan === true ? 1 : 0;
    labels.push(labelOf(node?.title ?? node?.name ?? ""));
  });
  return { positions: laid.positions, radii, slots, ghost, orphan, labels, labelled: labelledOf(radii, ghost), lineFrom: scene.lineFrom, lineTo: scene.lineTo, lineStyle: scene.lineStyle };
}

/** The answer's notes, verbatim in English: the first MAP_NOTES_SHOWN, the rest behind a disclosure. */
function MapNotes({ notes }: { notes: readonly string[] }) {
  const t = useTranslate();
  const item = (note: string, index: number) => (
    <li key={`${String(index)}-${note}`} lang="en">
      {note}
    </li>
  );
  const rest = notes.slice(MAP_NOTES_SHOWN);
  return (
    <div className="map-notes">
      <ul className="note-list" aria-label={t("graph.notes")}>
        {notes.slice(0, MAP_NOTES_SHOWN).map(item)}
      </ul>
      {rest.length > 0 && (
        <Disclosure summary={t("map.notesAll", { count: notes.length })}>
          <ul className="note-list">{rest.map((note, at) => item(note, at + MAP_NOTES_SHOWN))}</ul>
        </Disclosure>
      )}
    </div>
  );
}

/** The List's key: every filter, so any change of one starts the List again at its first page. */
function listKey(filters: MapFilters): string {
  return JSON.stringify([filters.sections, filters.parents, filters.mentions, filters.ghosts, filters.orphansOnly, filters.brokenOnly, [...filters.released]]);
}

/** What the live region says of a lit node: its title, kind and counts. */
function litWords(answer: MapDocument, index: number, t: Translate): string {
  const node = answer.nodes[index];
  if (node === undefined) {
    return "";
  }
  return t("map.lit", { title: node.title ?? node.name, kind: node.kind ?? t("map.noKind"), usage: t("map.usage", { used: node.used_by, mentioned: node.mentioned_by }) });
}

/** The served answer drawn and listed, with its controls. */
function MapBody({ project, answer, directions, archive, onArchive }: { project: string; answer: MapDocument; directions: readonly MapDirection[]; archive: boolean; onArchive: (on: boolean) => void }) {
  const t = useTranslate();
  const announce = useAnnounce();
  const countId = useId();
  const tabsBase = useId();
  const search = useRef<HTMLInputElement>(null);
  const [filters, setFilters] = useState<MapFilters>(DEFAULT_FILTERS);
  const [direction, setDirection] = useState<string | null>(null);
  const [tab, setTab] = useState<MapTab>("picture");
  const [query, setQuery] = useState("");
  const [cursor, setCursor] = useState({ query: "", at: 0 });
  const [litName, setLitName] = useState<string | null>(null);
  const [view, setView] = useState<View | null>(null);
  const [box, setBox] = useState<Pick<MapBox, "width" | "height">>(UNMEASURED);
  const [gone, setGone] = useState(0);
  const [seen, setSeen] = useState(answer);

  // A new answer: a lit node it no longer holds is cleared, and the live region says so (CM8).
  if (seen !== answer) {
    setSeen(answer);
    if (litName !== null && !answer.nodes.some((node) => node.name === litName)) {
      setLitName(null);
      setGone((count) => count + 1);
    }
  }
  useEffect(() => {
    if (gone > 0) {
      announce(t("map.gone"));
    }
  }, [gone, announce, t]);

  const scene = useMemo(() => sceneOf(answer, filters.sections), [answer, filters.sections]);
  const laid = useLaidOut(scene, answer);
  const colours = useMemo(() => kindColours(kindsOf(answer, scene.folded.members)), [answer, scene]);
  const chips = useMemo(() => kindChips(answer, scene.folded.members, colours), [answer, scene, colours]);
  const rows = useMemo(() => listOf(answer, scene.folded, filters), [answer, scene, filters]);
  const listGhosts = useMemo(() => ghostsOf(answer, scene.folded, filters), [answer, scene, filters]);
  const keep = useCallback((index: number) => keepsNode(answer, scene.folded, index, filters), [answer, scene, filters]);
  const matches = useMemo(() => matchesOf(answer, scene.folded.members, keep, query), [answer, scene, keep, query]);
  const at = cursor.query === query ? Math.min(cursor.at, Math.max(0, matches.length - 1)) : 0;
  const current = matches[at];

  // The search's count, once per change of what it finds.
  const matchCount = query.trim() === "" ? null : matches.length;
  useEffect(() => {
    if (matchCount !== null) {
      announce(t("map.matches", { count: matchCount }));
    }
  }, [matchCount, announce, t]);

  const shownScene = laid?.scene ?? null;
  const picture = useMemo(() => (laid === null ? null : pictureOf(laid, colours)), [laid, colours]);
  const kept = useMemo(() => (laid === null ? null : keptOf(laid.answer, laid.scene, filters)), [laid, filters]);
  const grid = useMemo(() => (laid === null ? null : gridOf(laid.positions, laid.scene.points.length)), [laid]);
  // The matches' points in the picture shown, by name: the scene may still be the last answer's.
  const matchPoints = useMemo(() => (laid === null ? [] : pointsByName(laid.answer, laid.scene, matches.flatMap((index) => answer.nodes[index]?.name ?? []))), [laid, answer, matches]);
  const litIndex = litName === null ? -1 : answer.nodes.findIndex((node) => node.name === litName);
  /** A node's point in the picture shown, by name: the picture may still be the last answer's while the new one lays out. */
  const pointOfName = (name: string | undefined): number => {
    if (laid === null || name === undefined) {
      return -1;
    }
    const index = laid.answer.nodes.findIndex((node) => node.name === name);
    return index < 0 ? -1 : (laid.scene.pointOf[index] ?? -1);
  };
  const pointOfIndex = (index: number) => pointOfName(answer.nodes[index]?.name);
  const litPoint = pointOfIndex(litIndex) >= 0 ? pointOfIndex(litIndex) : null;
  const lit = useMemo(() => {
    if (shownScene === null || kept === null) {
      return null;
    }
    if (litPoint !== null) {
      return litOf(shownScene, kept, litPoint);
    }
    return query.trim() === "" ? null : litMatches(shownScene, matchPoints);
  }, [shownScene, kept, litPoint, query, matchPoints]);
  const searchFocus = current !== undefined && query.trim() !== "" ? pointOfIndex(current) : null;

  // The fit frames every laid-out point, so a filter never moves the view (D7).
  const fitted = useMemo(() => (laid === null || picture === null ? null : fitView(laid.positions, picture.radii, new Uint8Array(laid.scene.points.length).fill(1), box)), [laid, picture, box]);
  const shownView = view ?? fitted ?? { x: 0, y: 0, zoom: 1 };

  const onCommand = useCallback((command: MapCommand) => {
    if (command === "fit") {
      setView(null);
    } else {
      setLitName(null);
    }
  }, []);

  function centreOn(index: number) {
    const point = pointOfIndex(index);
    if (laid === null || point < 0) {
      return;
    }
    setView({ ...shownView, x: laid.positions[point * 2] ?? 0, y: laid.positions[point * 2 + 1] ?? 0 });
  }

  /** ArrowDown and ArrowUp step through the matches; Enter centres and lights one, Enter again opens it. */
  function onSearchKey(event: KeyboardEvent<HTMLInputElement>) {
    if (matches.length === 0) {
      return;
    }
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      const step = event.key === "ArrowDown" ? 1 : -1;
      setCursor({ query, at: (at + step + matches.length) % matches.length });
      return;
    }
    if (event.key !== "Enter" || current === undefined) {
      return;
    }
    event.preventDefault();
    const node = answer.nodes[current];
    if (node === undefined) {
      return;
    }
    if (litName === node.name) {
      openDocFrom(node.name, event.currentTarget);
      return;
    }
    setLitName(node.name);
    centreOn(current);
    announce(litWords(answer, current, t));
  }

  function onSwitch(name: MapSwitch, on: boolean) {
    setFilters((now) => ({ ...now, [name]: on }));
  }

  function onChip(kind: string | null) {
    setDirection(null);
    setFilters((now) => {
      const released = new Set(now.released);
      if (!released.delete(kind)) {
        released.add(kind);
      }
      return { ...now, released };
    });
  }

  function onDirection(name: string | null) {
    setDirection(name);
    const chosen = directions.find((served) => served.name === name) ?? null;
    setFilters((now) => ({ ...now, released: releasedFor(chips, chosen) }));
  }

  const counts = t("map.counts", {
    documents: t("map.documents", { count: answer.counts.documents }),
    links: t("map.links", { count: answer.counts.edges }),
    orphans: t("map.orphans", { count: answer.counts.orphans }),
  });
  const brokenEmpty = filters.brokenOnly && answer.ghosts.length === 0;
  const laying = laid === null || laid.scene !== scene;
  const litNode = litIndex >= 0 ? litIndex : null;

  return (
    <>
      <MapControls
        query={query}
        onQuery={setQuery}
        onSearchKey={onSearchKey}
        searchRef={search}
        chips={chips}
        released={filters.released}
        onChip={onChip}
        directions={directions}
        direction={direction}
        onDirection={onDirection}
        filters={filters}
        onSwitch={onSwitch}
        archive={archive}
        onArchive={onArchive}
        onZoomIn={() => {
          setView({ ...shownView, zoom: clampZoom(shownView.zoom * ZOOM_STEP) });
        }}
        onZoomOut={() => {
          setView({ ...shownView, zoom: clampZoom(shownView.zoom / ZOOM_STEP) });
        }}
        onFit={() => {
          setView(null);
        }}
      />
      <div className="map-summary">
        <p id={countId} className="map-count">
          {counts}
        </p>
        {scene.hidden > 0 && <p className="note map-capped">{t("map.capped", { shown: scene.total - scene.hidden, count: scene.total })}</p>}
        {answer.notes.length > 0 && <MapNotes notes={answer.notes} />}
      </div>
      <Tabs
        label={t("map.views")}
        base={tabsBase}
        selected={tab}
        onSelect={setTab}
        tabs={[
          { id: "picture", label: t("map.picture") },
          { id: "list", label: t("graph.tabList") },
        ]}
      />
      <TabPanel base={tabsBase} id="picture" selected={tab === "picture"}>
        {tab === "picture" &&
          (brokenEmpty ? (
            <EmptyState level={3} title={t("map.noBroken")} description={<p>{t("map.noBrokenNext")}</p>} />
          ) : (
            <div className="map-stage" aria-busy={laying}>
              {laying && (
                <p className="map-laying" role="status">
                  {t("map.laying", { count: scene.points.length })}
                </p>
              )}
              {picture !== null && kept !== null && grid !== null && laid !== null ? (
                <MapStage
                  project={project}
                  answer={answer}
                  laid={laid}
                  picture={picture}
                  kept={kept}
                  grid={grid}
                  directions={directions}
                  lit={lit}
                  litIndex={litNode}
                  searchFocus={searchFocus !== null && searchFocus >= 0 ? searchFocus : null}
                  litFocus={litPoint}
                  view={shownView}
                  label={t("map.canvas")}
                  describedBy={countId}
                  onView={setView}
                  onCommand={onCommand}
                  onBox={setBox}
                />
              ) : (
                <Skeleton label={t("map.loading")} lines={4} />
              )}
              <div className="map-side">
                <MapLegend colours={colours} />
              </div>
            </div>
          ))}
      </TabPanel>
      <TabPanel base={tabsBase} id="list" selected={tab === "list"}>
        {tab === "list" && <MapList key={listKey(filters)} project={project} answer={answer} rows={rows} ghosts={listGhosts} directions={directions} />}
      </TabPanel>
    </>
  );
}

export function MapView({ project }: { project: string }) {
  const t = useTranslate();
  const headingId = useId();
  const region = useRef<HTMLElement>(null);
  const [archive, setArchive] = useState(false);
  const read = useMap(project, archive);
  const workspaces = useWorkspaces(project);
  const directions = useMemo(() => directionsOf(workspaces.data?.modules ?? []), [workspaces.data]);
  const failure = useRetainedFailure(read.error, read.isFetching);
  const retried = useRetryFocus(read.data, () => region.current);
  const answer = read.data;

  let body;
  if (answer === undefined) {
    body =
      failure === null ? (
        <Skeleton label={t("map.loading")} lines={6} />
      ) : (
        <ReadFailure
          title={t("map.failed")}
          failure={failure}
          retrying={read.isFetching}
          attempt={read.errorUpdateCount}
          onRetry={() => {
            retried();
            void read.refetch({ cancelRefetch: false });
          }}
        />
      );
  } else if (answer.nodes.length === 0) {
    body = <EmptyState level={3} title={t("map.none")} description={<p>{t("map.noneNext")}</p>} />;
  } else {
    body = <MapBody project={project} answer={answer} directions={directions} archive={archive} onArchive={setArchive} />;
  }

  return (
    <section ref={region} tabIndex={-1} className="map-view" aria-labelledby={headingId} aria-busy={read.isFetching}>
      <h2 id={headingId} className="map-heading">
        {t("map.title")}
      </h2>
      {body}
    </section>
  );
}
