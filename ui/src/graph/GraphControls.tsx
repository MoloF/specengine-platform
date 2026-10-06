import { useId, useState, type Ref, type SubmitEvent } from "react";
import { pushHash } from "../app/location";
import { sectionHash } from "../app/routes";
import { Icon } from "../ui/Icon";
import { directionLabel } from "../tree/labels";
import { directionIcon } from "./lines";
import { DEPTHS, type Chip, type GraphSettings } from "./settings";

const ALL = "all";

/**
 * The form "Graph options" (docs/features/ui-graph.md "Controls"): the REF field and Show, the
 * mode, the type chips, the depth and the archive. The typed REF is this form's own state: a
 * keystroke renders the form alone, never the canvas; Show moves the hash, each other control
 * changes the options at once (one read each).
 */
export function GraphControls({
  project,
  nodeRef,
  settings,
  chips,
  fieldRef,
  emptyHintId,
  onChange,
  onMode,
  onChip,
}: {
  project: string;
  nodeRef: string | null;
  settings: GraphSettings;
  chips: readonly Chip[];
  fieldRef: Ref<HTMLInputElement>;
  /** The empty state's words, read with the field while no REF is given. */
  emptyHintId: string | null;
  onChange: (next: (current: GraphSettings) => GraphSettings) => void;
  onMode: (impact: boolean) => void;
  onChip: (type: string) => void;
}) {
  const base = useId();
  const [typed, setTyped] = useState(nodeRef ?? "");
  const [shown, setShown] = useState(nodeRef);
  const [error, setError] = useState<string | null>(null);
  if (shown !== nodeRef) {
    // Storing information from previous renders (react.dev, useState): a new REF fills the field.
    setShown(nodeRef);
    setTyped(nodeRef ?? "");
    setError(null);
  }
  const pressed = chips.filter((chip) => chip.pressed).length;

  function submit(event: SubmitEvent<HTMLFormElement>) {
    event.preventDefault();
    const ref = typed.trim();
    if (ref === "") {
      setError("Type a REF: an ID, slug/ID, ID#SECTION or a root-relative .md path.");
      return;
    }
    setError(null);
    pushHash(sectionHash(project, "graph", ref));
  }

  const describedBy = [error === null ? null : `${base}-error`, emptyHintId].filter((id): id is string => id !== null).join(" ");

  return (
    <form className="graph-options" aria-label="Graph options" onSubmit={submit} noValidate>
      <div className="graph-ref">
        <label className="field-label" htmlFor={`${base}-ref`}>
          REF
        </label>
        <div className="search-row">
          <input
            id={`${base}-ref`}
            ref={fieldRef}
            className="field mono"
            type="text"
            autoComplete="off"
            spellCheck={false}
            placeholder="ID, slug/ID, ID#SECTION or a .md path"
            value={typed}
            aria-invalid={error !== null}
            aria-describedby={describedBy === "" ? undefined : describedBy}
            onChange={(event) => {
              setTyped(event.target.value);
              setError(null);
            }}
          />
          <button type="submit" className="button button-primary">
            Show
          </button>
        </div>
        {error !== null && (
          <p id={`${base}-error`} className="field-error" role="alert">
            {error}
          </p>
        )}
      </div>
      <fieldset className="graph-mode">
        <legend className="field-label">Mode</legend>
        <label className="graph-radio">
          <input
            type="radio"
            name={`${base}-mode`}
            checked={!settings.impact}
            onChange={() => {
              onMode(false);
            }}
          />
          <span>Outgoing</span>
        </label>
        <label className="graph-radio">
          <input
            type="radio"
            name={`${base}-mode`}
            checked={settings.impact}
            onChange={() => {
              onMode(true);
            }}
          />
          <span>Impact</span>
        </label>
      </fieldset>
      <div className="graph-depth">
        <label className="field-label" htmlFor={`${base}-depth`}>
          Depth
        </label>
        <select
          id={`${base}-depth`}
          className="field"
          value={settings.depth === null ? ALL : String(settings.depth)}
          onChange={(event) => {
            const value = event.target.value;
            onChange((current) => ({ ...current, depth: value === ALL ? null : Number(value) }));
          }}
        >
          {DEPTHS.map((depth) => (
            <option key={depth} value={String(depth)}>
              {depth}
            </option>
          ))}
          <option value={ALL}>All</option>
        </select>
      </div>
      <div className="archive-toggle graph-archive">
        <input
          id={`${base}-archive`}
          type="checkbox"
          checked={settings.archive}
          aria-describedby={`${base}-archive-hint`}
          onChange={(event) => {
            const archive = event.target.checked;
            onChange((current) => ({ ...current, archive }));
          }}
        />
        <label htmlFor={`${base}-archive`}>Include archive</label>
        <span id={`${base}-archive-hint`} className="sr-only">
          Links written in archived documents are followed too.
        </span>
      </div>
      <fieldset className="graph-types">
        <legend className="field-label">Types followed</legend>
        {chips.length === 0 ? (
          <p className="muted graph-types-pending">The types appear with the first answer of this mode.</p>
        ) : (
          <div className="graph-chips">
            {chips.map((chip) => {
              const last = chip.pressed && pressed === 1;
              return (
                <button
                  key={chip.type}
                  type="button"
                  className="graph-chip"
                  aria-pressed={chip.pressed}
                  aria-disabled={last ? true : undefined}
                  aria-describedby={last ? `${base}-last` : undefined}
                  onClick={() => {
                    if (!last) {
                      onChip(chip.type);
                    }
                  }}
                >
                  <span className="graph-chip-check" aria-hidden="true">
                    {chip.pressed && <Icon name="selected" />}
                  </span>
                  <span className="mono">{chip.type}</span>
                  {chip.direction !== null && (
                    <>
                      {" "}
                      <span className="graph-chip-direction">
                        <Icon name={directionIcon(chip.direction)} />
                        {directionLabel(chip.direction)}
                      </span>
                    </>
                  )}
                </button>
              );
            })}
          </div>
        )}
        {pressed === 1 && (
          <p id={`${base}-last`} className="note graph-types-note">
            At least one type is followed
          </p>
        )}
      </fieldset>
    </form>
  );
}
