import { useId, useState, type Ref, type SubmitEvent } from "react";
import { pushHash } from "../app/location";
import { sectionHash } from "../app/routes";
import { Button } from "../ui/Button";
import { Icon } from "../ui/Icon";
import { directionLabel } from "../tree/labels";
import { directionIcon } from "./lines";
import { DEPTHS, type Chip, type GraphSettings } from "./settings";
import { useTranslate } from "../i18n";

const ALL = "all";

/**
 * The form "Graph options" (docs/features/ui-graph.md "Implementation", Controls): the REF field
 * and Show, the mode, the type chips, the depth and the archive. The typed REF is this form's own
 * state: a keystroke renders the form alone, never the canvas; Show moves the hash, each other
 * control changes the options at once (one read each).
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
  const t = useTranslate();
  const base = useId();
  const [typed, setTyped] = useState(nodeRef ?? "");
  const [shown, setShown] = useState(nodeRef);
  /** A Show pressed on an empty field: the field says what to type. */
  const [missing, setMissing] = useState(false);
  if (shown !== nodeRef) {
    // Storing information from previous renders (react.dev, useState): a new REF fills the field.
    setShown(nodeRef);
    setTyped(nodeRef ?? "");
    setMissing(false);
  }
  const pressed = chips.filter((chip) => chip.pressed).length;

  function submit(event: SubmitEvent<HTMLFormElement>) {
    event.preventDefault();
    const ref = typed.trim();
    if (ref === "") {
      setMissing(true);
      return;
    }
    setMissing(false);
    pushHash(sectionHash(project, "graph", ref));
  }

  const describedBy = [missing ? `${base}-error` : null, emptyHintId].filter((id): id is string => id !== null).join(" ");

  return (
    <form className="graph-options" aria-label={t("graph.controls.label")} onSubmit={submit} noValidate>
      <div className="graph-ref">
        <label className="field-label" htmlFor={`${base}-ref`}>
          {t("graph.controls.ref")}
        </label>
        <div className="search-row">
          <input
            id={`${base}-ref`}
            ref={fieldRef}
            className="field mono"
            type="text"
            autoComplete="off"
            spellCheck={false}
            placeholder={t("graph.controls.placeholder")}
            value={typed}
            aria-invalid={missing}
            aria-describedby={describedBy === "" ? undefined : describedBy}
            onChange={(event) => {
              setTyped(event.target.value);
              setMissing(false);
            }}
          />
          <Button appearance="primary" type="submit">
            {t("graph.controls.show")}
          </Button>
        </div>
        {missing && (
          <p id={`${base}-error`} className="field-error" role="alert">
            {t("graph.controls.typeRef")}
          </p>
        )}
      </div>
      <fieldset className="graph-mode">
        <legend className="field-label">{t("graph.controls.mode")}</legend>
        <label className="graph-radio">
          <input
            type="radio"
            name={`${base}-mode`}
            checked={!settings.impact}
            onChange={() => {
              onMode(false);
            }}
          />
          <span>{t("graph.controls.outgoing")}</span>
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
          <span>{t("graph.controls.impact")}</span>
        </label>
      </fieldset>
      <div className="graph-depth">
        <label className="field-label" htmlFor={`${base}-depth`}>
          {t("graph.controls.depth")}
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
          <option value={ALL}>{t("graph.controls.all")}</option>
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
        <label htmlFor={`${base}-archive`}>{t("graph.includeArchive")}</label>
        <span id={`${base}-archive-hint`} className="sr-only">
          {t("graph.controls.archiveHint")}
        </span>
      </div>
      <fieldset className="graph-types">
        <legend className="field-label">{t("graph.controls.types")}</legend>
        {chips.length === 0 ? (
          <p className="muted graph-types-pending">{t("graph.controls.typesPending")}</p>
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
                        {directionLabel(chip.direction, t)}
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
            {t("graph.controls.atLeastOne")}
          </p>
        )}
      </fieldset>
    </form>
  );
}
