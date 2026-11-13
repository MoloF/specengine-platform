import { memo, useId, type KeyboardEvent, type Ref } from "react";
import { useTranslate } from "../i18n";
import { Button } from "../ui/Button";
import { Icon } from "../ui/Icon";
import type { MapDirection } from "./colours";
import type { KindChip, MapFilters } from "./filters";

// The map's controls (docs/features/corpus-map.md "Acceptance criteria", CM8): the search, a chip per
// kind with its swatch, words and count, the Direction select that presses its kinds' chips, the
// switches, and the zoom. A chip or a switch hides; only Sections lays out again and Include archive
// reads again. Tab reaches each in turn, then the canvas.

/** The switches that only hide or show, in their order. */
const SWITCHES = ["sections", "parents", "mentions", "ghosts", "orphansOnly", "brokenOnly"] as const;

export type MapSwitch = (typeof SWITCHES)[number];

const ALL = "";

export const MapControls = memo(function MapControls({
  query,
  onQuery,
  onSearchKey,
  searchRef,
  chips,
  released,
  onChip,
  directions,
  direction,
  onDirection,
  filters,
  onSwitch,
  archive,
  onArchive,
  onZoomIn,
  onZoomOut,
  onFit,
}: {
  query: string;
  onQuery: (query: string) => void;
  onSearchKey: (event: KeyboardEvent<HTMLInputElement>) => void;
  searchRef: Ref<HTMLInputElement>;
  chips: readonly KindChip[];
  released: ReadonlySet<string | null>;
  onChip: (kind: string | null) => void;
  directions: readonly MapDirection[];
  direction: string | null;
  onDirection: (name: string | null) => void;
  filters: MapFilters;
  onSwitch: (name: MapSwitch, on: boolean) => void;
  archive: boolean;
  onArchive: (on: boolean) => void;
  onZoomIn: () => void;
  onZoomOut: () => void;
  onFit: () => void;
}) {
  const t = useTranslate();
  const base = useId();
  return (
    <div className="map-controls">
      <div className="map-search">
        <label className="field-label" htmlFor={`${base}-search`}>
          {t("map.search")}
        </label>
        <input
          id={`${base}-search`}
          ref={searchRef}
          className="field"
          type="search"
          autoComplete="off"
          spellCheck={false}
          value={query}
          onChange={(event) => {
            onQuery(event.target.value);
          }}
          onKeyDown={onSearchKey}
        />
      </div>
      <div className="map-direction">
        <label className="field-label" htmlFor={`${base}-direction`}>
          {t("map.direction")}
        </label>
        <select
          id={`${base}-direction`}
          className="field"
          value={direction ?? ALL}
          onChange={(event) => {
            onDirection(event.target.value === ALL ? null : event.target.value);
          }}
        >
          <option value={ALL}>{t("graph.controls.all")}</option>
          {directions.map((served) => (
            <option key={served.name} value={served.name}>
              {served.title}
            </option>
          ))}
        </select>
      </div>
      <div className="map-chips" role="group" aria-label={t("map.kinds")}>
        {chips.map((chip) => {
          const { kind } = chip;
          const pressed = !released.has(kind);
          return (
            <button
              key={kind ?? ""}
              type="button"
              className="chip map-chip"
              aria-pressed={pressed}
              onClick={() => {
                onChip(kind);
              }}
            >
              <span className="chip-check" aria-hidden="true">
                {pressed && <Icon name="selected" />}
              </span>
              <span className={`map-swatch map-slot-${String(chip.slot)}`} aria-hidden="true" />
              {kind === null ? <span>{t("map.noKind")}</span> : <span className="mono">{kind}</span>} <span className="chip-count">{chip.count}</span>
            </button>
          );
        })}
      </div>
      <fieldset className="map-switches">
        <legend className="field-label">{t("graph.controls.show")}</legend>
        {SWITCHES.map((name) => (
          <label key={name} className="map-switch">
            <input
              type="checkbox"
              checked={filters[name]}
              onChange={(event) => {
                onSwitch(name, event.target.checked);
              }}
            />
            <span>{t(`map.${name}`)}</span>
          </label>
        ))}
        <label className="map-switch">
          <input
            type="checkbox"
            checked={archive}
            onChange={(event) => {
              onArchive(event.target.checked);
            }}
          />
          <span>{t("graph.includeArchive")}</span>
        </label>
      </fieldset>
      <div className="map-zoom" role="group" aria-label={t("graph.aria.controls")}>
        <Button spacing="compact" onClick={onZoomIn}>
          {t("graph.aria.zoomIn")}
        </Button>
        <Button spacing="compact" onClick={onZoomOut}>
          {t("graph.aria.zoomOut")}
        </Button>
        <Button spacing="compact" icon="fit" onClick={onFit}>
          {t("map.fit")}
        </Button>
      </div>
    </div>
  );
});
