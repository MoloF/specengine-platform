import { lazy, Suspense } from "react";
import { RegionBoundary } from "../app/RegionBoundary";
import { useTranslate } from "../i18n";
import { Skeleton } from "../ui/states";
import type { GraphHomeView } from "./settings";

// The Graph section's home, `#/<project>/graph` with no REF
// (docs/features/scope-rings.md "Acceptance criteria", SR7, SR8; docs/features/corpus-map.md
// "Acceptance criteria", CM8, CM11): its views below the REF field, each a chunk of its own. Two
// views, the Map and the Rings: a "View" group of pressed buttons picks one, the choice kept per
// project for the page's life in the Shell's graph memory.

/** The Map view and its layout engine load when the home shows it, never with the app or the local graph. */
const MapView = lazy(() => import("../map/MapView").then((module) => ({ default: module.MapView })));

/** The Rings view and its drawing load when the home shows, never with the app or the local graph. */
const RingsView = lazy(() => import("../rings/RingsView").then((module) => ({ default: module.RingsView })));

/** The views in the group's order. */
const VIEWS: readonly GraphHomeView[] = ["map", "rings"];

export function GraphHome({ project, view, onView }: { project: string; view: GraphHomeView; onView: (view: GraphHomeView) => void }) {
  const t = useTranslate();
  return (
    <div className="graph-home">
      <div className="graph-home-views" role="group" aria-label={t("graph.home.view")}>
        {VIEWS.map((name) => (
          <button
            key={name}
            type="button"
            className="chip graph-home-view"
            aria-pressed={view === name}
            onClick={() => {
              onView(name);
            }}
          >
            {t(`graph.home.${name}`)}
          </button>
        ))}
      </div>
      {view === "map" ? (
        <RegionBoundary name={t("graph.home.map")}>
          <Suspense fallback={<Skeleton label={t("map.loading")} lines={6} />}>
            <MapView project={project} />
          </Suspense>
        </RegionBoundary>
      ) : (
        <RegionBoundary name={t("rings.boundary")}>
          <Suspense fallback={<Skeleton label={t("rings.loading")} lines={6} />}>
            <RingsView project={project} />
          </Suspense>
        </RegionBoundary>
      )}
    </div>
  );
}
