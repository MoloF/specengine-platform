import { useId, useRef, useState, type KeyboardEvent } from "react";
import type { Workspace } from "../api/types";
import { useTranslate } from "../i18n";
import { Icon } from "../ui/Icon";
import { ErrorPanel } from "../ui/states";

/**
 * The Workspace select in the shell's header (docs/features/workspaces.md "The UI", D6): named
 * Workspace, never "View as", its hint saying it changes no right. Its options are the served
 * titles in served order; shown only with two or more workspaces.
 */
export function WorkspaceSwitcher({
  workspaces,
  chosen,
  onChoose,
}: {
  workspaces: readonly Workspace[];
  chosen: Workspace;
  onChoose: (name: string) => void;
}) {
  const t = useTranslate();
  const id = useId();
  if (workspaces.length < 2) {
    return null;
  }
  return (
    <div className="workspace-switcher">
      <label htmlFor={id}>{t("workspace.workspaceLabel")}</label>
      <select
        id={id}
        className="field"
        value={chosen.name}
        // Seen on hover too; read aloud once, as the description (aria-describedby wins over title).
        title={t("workspace.workspaceHint")}
        aria-describedby={`${id}-hint`}
        onChange={(event) => {
          onChoose(event.target.value);
        }}
      >
        {workspaces.map((workspace) => (
          <option key={workspace.name} value={workspace.name}>
            {workspace.title}
          </option>
        ))}
      </select>
      <span id={`${id}-hint`} className="sr-only">
        {t("workspace.workspaceHint")}
      </span>
    </div>
  );
}

/**
 * "Config notes (n)" beside the switcher, or in its place (docs/features/workspaces.md "The UI"): a
 * disclosure listing each note of the reader verbatim, in served order; on a failed read the
 * daemon's message verbatim and Retry, under a line saying what the nav shows: every section, or the
 * last read's while one is kept. A config problem is never an error page and never a blank
 * nav, and nothing here is spoken unasked: the nav already shows every section. Not drawn with no
 * note and no failure.
 */
export function ConfigNotes({
  notes,
  failure,
  kept,
  retrying,
  attempt,
  onRetry,
}: {
  notes: readonly string[];
  /** The daemon's message when the workspaces could not be read; null otherwise. */
  failure: string | null;
  /** A last answer is kept and shown, notes and nav: the failure is a read again's. */
  kept: boolean;
  retrying: boolean;
  attempt: number;
  onRetry: () => void;
}) {
  const t = useTranslate();
  const panelId = useId();
  const button = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  const count = notes.length + (failure === null ? 0 : 1);
  if (count === 0) {
    return null;
  }

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key === "Escape" && open && !event.nativeEvent.isComposing) {
      event.preventDefault();
      setOpen(false);
      button.current?.focus();
    }
  }

  return (
    <div className="config-notes" onKeyDown={onKeyDown}>
      <button
        ref={button}
        type="button"
        className="button button-quiet config-notes-button"
        aria-expanded={open}
        aria-controls={open ? panelId : undefined}
        onClick={() => {
          setOpen(!open);
        }}
      >
        <Icon name={failure === null ? "info" : "alert"} />
        <span>{t("workspace.notesButton", { n: String(count) })}</span>
      </button>
      {/* Drawn only open: closed, its words are nowhere on the page, so nothing else's text repeats unseen. */}
      {open && (
        <div id={panelId} className="config-notes-panel">
          {failure !== null && <ErrorPanel title={kept ? t("workspace.readFailedKept") : t("workspace.readFailed")} message={failure} retrying={retrying} attempt={attempt} announce={false} onRetry={onRetry} />}
          {notes.length > 0 && (
            <ul className="note-list config-notes-list" aria-label={t("workspace.notesTitle")}>
              {notes.map((note, index) => (
                <li key={`${String(index)}-${note}`} className="verbatim-note" lang="en">
                  {note}
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
    </div>
  );
}
