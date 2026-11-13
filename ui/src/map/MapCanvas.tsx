import { useEffect, useRef, useState, type KeyboardEvent, type PointerEvent, type RefObject } from "react";
import { CLICK_DISTANCE } from "../graph/geometry";
import { hasModifier } from "../ui/keys";
import { paletteOf, PEN_ROLES, SLOT_ROLES, type MapPalette } from "./colours";
import { clampZoom, composeMap, drawBase, toMap, type MapBox, type MapPicture, type MapView } from "./draw";
import type { Kept, Lit } from "./filters";
import { hitAt, type HitGrid } from "./hit";

// The map's one canvas (docs/features/corpus-map.md "Acceptance criteria", CM7; the canvas rules of
// `ui/README.md` "Screen rules"): sized to its box at the device's pixel ratio, its colours read
// from tokens.css through a hidden swatch per role and again when the theme changes, a redraw
// coalesced per frame. The base layer, every kept node and line, lives in an offscreen canvas,
// drawn again only when the picture, a filter, the theme or the size changes; a hover or a search
// composes it with the highlight alone (CM12). A pan or a zoom composes the base moved to the new
// view at once and draws it again for that view once the view has held for BASE_SETTLE_MS, so a
// held key or a drag never draws the whole map per step. It is read-only. The wheel is never taken: it scrolls the page. A press that
// moves at most CLICK_DISTANCE px is a click and opens the document under it; past that it pans.
// The picture is `aria-hidden` in a labelled region, the one tab stop, whose keys zoom (`+`, `=`,
// `-`), fit (`0`), pan (arrows) and clear (Esc) only while it holds focus. Unmounted, it gives the
// offscreen base's pixels back at once: its size set to zero, the layer let go.

/** How far an arrow pans, in screen pixels. */
export const PAN_STEP = 64;

/** A zoom key or button multiplies or divides the zoom by this. */
export const ZOOM_STEP = 1.25;

/** How long a new view holds before the base is drawn again for it, in ms. */
export const BASE_SETTLE_MS = 150;

/** A pointer this close to a disc's edge, in screen pixels, still hits it. */
const HIT_SLOP = 2;

/** The box before it is measured, as in a test's DOM, which measures none. */
export const UNMEASURED: Pick<MapBox, "width" | "height"> = { width: 960, height: 600 };

/** Every role the picture reads, each through its own swatch. */
const ROLES: readonly string[] = [...SLOT_ROLES, ...Object.values(PEN_ROLES)];

/** The canvas's acts that the view answers: a zoom, a fit, a clear. */
export type MapCommand = "fit" | "clear";

/** The next frame, or a timer where there is none. */
function schedule(callback: () => void): () => void {
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

/** The roles as the browser resolves them now; a role it does not resolve stays its name. */
function readPalette(swatches: HTMLElement, canvas: HTMLCanvasElement): MapPalette {
  const colours = new Map<string, string>();
  for (const swatch of swatches.querySelectorAll<HTMLElement>("[data-role]")) {
    const role = swatch.dataset.role ?? "";
    colours.set(role, window.getComputedStyle(swatch).color || role);
  }
  return paletteOf((role) => colours.get(role) ?? role, window.getComputedStyle(canvas).fontFamily || "sans-serif");
}

/** Calls `changed` whenever the theme may have changed: the pinned choice, or the system's scheme. */
function onThemeChange(changed: () => void): () => void {
  const observer = new MutationObserver(changed);
  observer.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
  const scheme = typeof window.matchMedia === "function" ? window.matchMedia("(prefers-color-scheme: dark)") : null;
  scheme?.addEventListener("change", changed);
  return () => {
    observer.disconnect();
    scheme?.removeEventListener("change", changed);
  };
}

export interface MapCanvasProps {
  /** The labelled region, the one tab stop: the stage's, so its pane can hand focus back to it. */
  regionRef?: RefObject<HTMLDivElement | null>;
  picture: MapPicture;
  kept: Kept;
  lit: Lit | null;
  /** The point the focus ring circles. */
  focus: number | null;
  view: MapView;
  grid: HitGrid;
  label: string;
  describedBy: string;
  onView: (view: MapView) => void;
  onCommand: (command: MapCommand) => void;
  /** The point under the pointer, null when none. */
  onHover: (point: number | null) => void;
  /** A click on a point: its document opens, focus coming back to the region. */
  onOpen: (point: number, opener: HTMLElement) => void;
  onBox: (box: Pick<MapBox, "width" | "height">) => void;
}

export function MapCanvas({ regionRef, picture, kept, lit, focus, view, grid, label, describedBy, onView, onCommand, onHover, onOpen, onBox }: MapCanvasProps) {
  const ownRegion = useRef<HTMLDivElement>(null);
  const region = regionRef ?? ownRegion;
  const canvas = useRef<HTMLCanvasElement>(null);
  const swatches = useRef<HTMLSpanElement>(null);
  const box = useRef<Pick<MapBox, "width" | "height">>(UNMEASURED);
  const press = useRef<{ x: number; y: number; view: MapView; moved: boolean } | null>(null);
  const [palette, setPalette] = useState<MapPalette | null>(null);
  const [measured, setMeasured] = useState(0);
  /** The base layer as last drawn, with what it was drawn from. */
  const base = useRef<{ canvas: HTMLCanvasElement; picture: MapPicture; kept: Kept; palette: MapPalette; view: MapView; box: MapBox } | null>(null);
  /** The frame's inputs, for a base drawn again once a view has held. */
  const latest = useRef<{ picture: MapPicture; kept: Kept; lit: Lit | null; focus: number | null; view: MapView; palette: MapPalette } | null>(null);
  const settle = useRef<number | null>(null);

  // The colours, read on mount and again on a theme change.
  useEffect(() => {
    const read = () => {
      if (swatches.current !== null && canvas.current !== null) {
        setPalette(readPalette(swatches.current, canvas.current));
      }
    };
    const cancel = schedule(read);
    const stop = onThemeChange(read);
    return () => {
      cancel();
      stop();
    };
  }, []);

  // The box, measured as it changes; a DOM that measures nothing keeps UNMEASURED.
  useEffect(() => {
    const element = region.current;
    if (element === null) {
      return;
    }
    const observer = new ResizeObserver(() => {
      const rect = element.getBoundingClientRect();
      if (rect.width > 0 && rect.height > 0) {
        box.current = { width: rect.width, height: rect.height };
        onBox(box.current);
        setMeasured((count) => count + 1);
      }
    });
    observer.observe(element);
    return () => {
      observer.disconnect();
    };
  }, [onBox, region]);

  /**
   * One frame: the base drawn again when what it holds changed (or, with `fresh`, when the view
   * moved), then composed with the highlight on the visible canvas. A view change alone composes the
   * moved base and waits BASE_SETTLE_MS of stillness to draw it again.
   */
  const paint = useRef((fresh: boolean) => {
    const element = canvas.current;
    const now = latest.current;
    const pen = element?.getContext("2d") ?? null;
    if (element === null || now === null || pen === null) {
      return;
    }
    const ratio = window.devicePixelRatio || 1;
    const size: MapBox = { ...box.current, ratio };
    const width = Math.round(size.width * ratio);
    const height = Math.round(size.height * ratio);
    if (element.width !== width || element.height !== height) {
      element.width = width;
      element.height = height;
    }
    const held = base.current;
    const stale =
      held === null || held.picture !== now.picture || held.kept !== now.kept || held.palette !== now.palette || held.box.width !== size.width || held.box.height !== size.height || held.box.ratio !== ratio;
    const moved = held !== null && (held.view.x !== now.view.x || held.view.y !== now.view.y || held.view.zoom !== now.view.zoom);
    if (stale || (fresh && moved)) {
      const layer = held?.canvas ?? document.createElement("canvas");
      layer.width = width;
      layer.height = height;
      const basePen = layer.getContext("2d");
      if (basePen === null) {
        return;
      }
      drawBase(basePen, now.picture, now.kept, now.view, size, now.palette);
      base.current = { canvas: layer, picture: now.picture, kept: now.kept, palette: now.palette, view: now.view, box: size };
    } else if (moved) {
      if (settle.current !== null) {
        window.clearTimeout(settle.current);
      }
      settle.current = window.setTimeout(() => {
        settle.current = null;
        paint.current(true);
      }, BASE_SETTLE_MS);
    }
    const drawn = base.current;
    if (drawn !== null) {
      composeMap(pen, { image: drawn.canvas, view: drawn.view, box: drawn.box }, now.picture, now.kept, now.lit, now.focus, now.view, size, now.palette);
    }
  });

  // One frame per change, whatever changed in it.
  useEffect(() => {
    if (palette === null) {
      return;
    }
    latest.current = { picture, kept, lit, focus, view, palette };
    return schedule(() => {
      paint.current(false);
    });
  }, [picture, kept, lit, focus, view, palette, measured]);

  // A pending redraw of the base goes with the canvas, and the base's pixels with it.
  useEffect(
    () => () => {
      if (settle.current !== null) {
        window.clearTimeout(settle.current);
      }
      const held = base.current;
      if (held !== null) {
        held.canvas.width = 0;
        held.canvas.height = 0;
      }
      base.current = null;
    },
    [],
  );

  /** The map point under a pointer event. */
  function pointAt(event: PointerEvent<HTMLElement>): number {
    const rect = event.currentTarget.getBoundingClientRect();
    const at = toMap(view, box.current, event.clientX - rect.left, event.clientY - rect.top);
    return hitAt(grid, picture.positions, picture.radii, kept.points, at.x, at.y, HIT_SLOP / view.zoom);
  }

  function onPointerDown(event: PointerEvent<HTMLDivElement>) {
    if (event.button !== 0) {
      return;
    }
    press.current = { x: event.clientX, y: event.clientY, view, moved: false };
  }

  function onPointerMove(event: PointerEvent<HTMLDivElement>) {
    const held = press.current;
    if (held === null) {
      const point = pointAt(event);
      onHover(point < 0 ? null : point);
      return;
    }
    const dx = event.clientX - held.x;
    const dy = event.clientY - held.y;
    if (held.moved || Math.hypot(dx, dy) > CLICK_DISTANCE) {
      held.moved = true;
      onView({ ...held.view, x: held.view.x - dx / held.view.zoom, y: held.view.y - dy / held.view.zoom });
    }
  }

  /** A press that moved at most CLICK_DISTANCE px is a click: the document under it opens. */
  function onPointerUp(event: PointerEvent<HTMLDivElement>) {
    const held = press.current;
    press.current = null;
    if (held === null || held.moved || Math.hypot(event.clientX - held.x, event.clientY - held.y) > CLICK_DISTANCE) {
      return;
    }
    const point = pointAt(event);
    if (point >= 0) {
      onOpen(point, event.currentTarget);
    }
  }

  /** Keys act only with the region itself focused, unmodified: never in the search field. */
  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.target !== event.currentTarget || event.defaultPrevented || hasModifier(event)) {
      return;
    }
    const pan = PAN_STEP / view.zoom;
    switch (event.key) {
      case "+":
      case "=":
        onView({ ...view, zoom: clampZoom(view.zoom * ZOOM_STEP) });
        break;
      case "-":
        onView({ ...view, zoom: clampZoom(view.zoom / ZOOM_STEP) });
        break;
      case "0":
        onCommand("fit");
        break;
      case "ArrowLeft":
        onView({ ...view, x: view.x - pan });
        break;
      case "ArrowRight":
        onView({ ...view, x: view.x + pan });
        break;
      case "ArrowUp":
        onView({ ...view, y: view.y - pan });
        break;
      case "ArrowDown":
        onView({ ...view, y: view.y + pan });
        break;
      case "Escape":
        onCommand("clear");
        break;
      default:
        return;
    }
    event.preventDefault();
  }

  return (
    <div
      ref={region}
      className="map-canvas"
      role="region"
      aria-label={label}
      aria-describedby={describedBy}
      tabIndex={0}
      onKeyDown={onKeyDown}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerLeave={() => {
        press.current = null;
        onHover(null);
      }}
    >
      <canvas ref={canvas} aria-hidden="true" />
      <span ref={swatches} className="map-swatches" aria-hidden="true">
        {ROLES.map((role) => (
          <span key={role} data-role={role} style={{ color: `var(${role})` }} />
        ))}
      </span>
    </div>
  );
}
