import { useEffect, useId, useState, type KeyboardEvent, type Ref } from "react";
import { apiErrorOf } from "../api/client";
import { useCachedInbox, useCachedTasks, useSearchOnActivation } from "../api/queries";
import type { Project } from "../api/types";
import { statusLook } from "../inbox/labels";
import { summaryOf } from "../inbox/summary";
import { taskStatusLook } from "../tasks/labels";
import { useAnnounce } from "../ui/announcer";
import { Badge } from "../ui/Badge";
import { Dialog } from "../ui/Dialog";
import { Icon } from "../ui/Icon";
import { groupLabel, needleOf, paletteGroups, type Held, type PaletteInput, type PaletteOption } from "./options";

/** What an option says; the ID and the words the text is matched on. */
function OptionText({ option }: { option: PaletteOption }) {
  switch (option.type) {
    case "section":
      return <span className="palette-option-main">{option.label}</span>;
    case "task":
      return (
        <>
          <span className="palette-option-main">
            <span className="mono">{option.entry.id}</span>:{" "}
            <span className={option.entry.title === null ? "muted" : undefined}>{option.entry.title ?? "Untitled"}</span>
          </span>
          <Badge name="State" look={taskStatusLook(option.entry.status)} />
        </>
      );
    case "proposal":
      return (
        <>
          <span className="palette-option-main">
            <span className="mono">{option.proposal.id}</span>: <span>{summaryOf(option.proposal)}</span>
          </span>
          <Badge name="Status" look={statusLook(option.proposal.status)} />
        </>
      );
    case "project":
      return (
        <span className="palette-option-main">
          <span>{option.project.name}</span> <span className="mono muted">{option.project.slug}</span>
        </span>
      );
    case "search":
      return (
        <span className="palette-option-main">
          <Icon name="search" />
          <span>Search the spec for &apos;{option.query}&apos;</span>
        </span>
      );
    case "ref":
      return (
        <span className="palette-option-main">
          <Icon name="arrowRight" />
          <span>Open &apos;{option.ref}&apos; in the spec tree</span>
        </span>
      );
    case "hit":
      return (
        <span className="palette-option-main">
          <span className="mono">{option.hit.id ?? option.hit.path}</span>
          {option.hit.title !== null && <>: {option.hit.title}</>}
        </span>
      );
  }
}

function held<T>(data: T | undefined, failed: boolean): Held<T> {
  if (data !== undefined) {
    return { state: "ready", value: data };
  }
  return failed ? { state: "failed" } : { state: "loading" };
}

/**
 * The jump palette (docs/features/ui-home.md "Palette"): a modal dialog with one combobox; focus
 * stays in it while the arrows, Home and End move the active option and Enter or a click takes it.
 * Esc, the Close button and a click on the scrim close it.
 * Tasks and the Inbox come from the cache, read once per opening only when missing or failed; the
 * node search runs once per activation. A jump goes to `onJump`; every failure is said under the list
 * in the daemon's words, the dialog and the text kept.
 */
export function Palette({
  routeProject,
  projects,
  projectsFailure,
  inputRef,
  onJump,
  onClose,
}: {
  /** The route's project, or null on a route without one. */
  routeProject: string | null;
  /** The shell's read of the projects; never read again here. */
  projects: readonly Project[] | undefined;
  projectsFailure: string | null;
  inputRef: Ref<HTMLInputElement>;
  onJump: (hash: string) => void;
  onClose: () => void;
}) {
  // While the projects are on their way or failed, the route's project stands (the shell shows its
  // view meanwhile), so Sections, Tasks, Inbox and the spec tree still work; once read, the route's
  // when listed, else the first.
  const project =
    projects === undefined || projects.some((candidate) => candidate.slug === routeProject) ? routeProject : (projects[0]?.slug ?? null);
  const tasks = useCachedTasks(project ?? "", project !== null);
  const inbox = useCachedInbox(project ?? "", project !== null);
  const search = useSearchOnActivation(project ?? "");
  const announce = useAnnounce();
  const base = useId();
  const listboxId = `${base}-list`;
  const [text, setText] = useState("");
  /** The active option's key; null: the first. */
  const [active, setActive] = useState<string | null>(null);

  function inputFor(typed: string, searched: boolean): PaletteInput {
    let found: PaletteInput["search"] = null;
    if (searched && search.isPending) {
      found = { state: "loading" };
    } else if (searched && search.data !== undefined && search.variables === typed) {
      found = { state: "ready", value: search.data.hits };
    }
    return {
      project,
      text: typed,
      tasks: held(tasks.data?.tasks, tasks.error !== null),
      inbox: held(inbox.data?.proposals, inbox.error !== null),
      projects: held(projects, projectsFailure !== null),
      search: found,
    };
  }

  const groups = paletteGroups(inputFor(text, true));
  const options = groups.flatMap((group) => group.options);
  const current = options.find((option) => option.key === active) ?? options[0];
  // An option's element ID comes from its key, escaped (no space, any script), so the active
  // descendant keeps its ID while answers arrive and the list around it moves.
  const idOf = (option: PaletteOption) => `${base}-option-${encodeURIComponent(option.key)}`;
  const activeId = current === undefined ? undefined : idOf(current);
  const signature = options.map((option) => option.key).join("\n");
  const count = options.length;

  // Each change of the list is counted in the shell's polite live region, outside the inert page.
  useEffect(() => {
    announce(`${String(count)} ${count === 1 ? "option" : "options"}`);
  }, [signature, count, announce]);

  useEffect(() => {
    if (activeId !== undefined) {
      document.getElementById(activeId)?.scrollIntoView({ block: "nearest" });
    }
  }, [activeId]);

  function activate(option: PaletteOption) {
    if (option.type === "search") {
      setActive(option.key);
      search.mutate(option.query);
      return;
    }
    onJump(option.hash);
  }

  function onChange(typed: string) {
    setText(typed);
    if (!search.isIdle) {
      search.reset();
    }
    const first = paletteGroups(inputFor(typed, false)).flatMap((group) => group.options)[0];
    setActive(first?.key ?? null);
  }

  function onKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (event.nativeEvent.isComposing || event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) {
      return;
    }
    const at = current === undefined ? -1 : options.indexOf(current);
    let next: PaletteOption | undefined;
    switch (event.key) {
      case "ArrowDown":
        next = options[Math.min(options.length - 1, at + 1)];
        break;
      case "ArrowUp":
        next = options[Math.max(0, at - 1)];
        break;
      case "Home":
        next = options[0];
        break;
      case "End":
        next = options[options.length - 1];
        break;
      case "Enter":
        event.preventDefault();
        if (current !== undefined) {
          activate(current);
        }
        return;
      default:
        return;
    }
    event.preventDefault();
    if (next !== undefined) {
      setActive(next.key);
    }
  }

  const typed = needleOf(text) !== "";
  const failures: [string, string][] = [];
  if (projects === undefined && projectsFailure !== null) {
    failures.push(["Projects", projectsFailure]);
  }
  if (typed && project !== null && tasks.data === undefined && tasks.error !== null) {
    failures.push(["Tasks", apiErrorOf(tasks.error).message]);
  }
  if (typed && project !== null && inbox.data === undefined && inbox.error !== null) {
    failures.push(["Inbox", apiErrorOf(inbox.error).message]);
  }
  if (search.error !== null) {
    failures.push(["Search", apiErrorOf(search.error).message]);
  }
  const noHits = search.data !== undefined && search.variables === text && search.data.hits.length === 0;

  return (
    <Dialog title="Jump to" onClose={onClose} className="palette" closeOnScrim>
      <label className="sr-only" htmlFor={`${base}-input`}>
        Jump to a section, task, proposal, node or project
      </label>
      <input
        id={`${base}-input`}
        ref={inputRef}
        data-autofocus=""
        className="field palette-input"
        type="text"
        role="combobox"
        aria-expanded="true"
        aria-autocomplete="list"
        aria-controls={listboxId}
        aria-activedescendant={activeId}
        autoComplete="off"
        spellCheck={false}
        placeholder="Section, task, proposal, node or project"
        value={text}
        onChange={(event) => {
          onChange(event.target.value);
        }}
        onKeyDown={onKeyDown}
      />
      <div
        id={listboxId}
        role="listbox"
        aria-label="Places to jump to"
        className="palette-list"
        onMouseDown={(event) => {
          // A click takes the option; focus stays in the field.
          event.preventDefault();
        }}
      >
        {groups.map((group) => {
          const labelId = `${base}-group-${group.name.replace(/\s/g, "-")}`;
          return (
            <div key={group.name} role="group" aria-labelledby={labelId} aria-busy={group.loading} className="palette-group">
              <div id={labelId} role="presentation" className="palette-group-label">
                {groupLabel(group)}
              </div>
              {group.options.map((option) => (
                <div
                  key={option.key}
                  id={idOf(option)}
                  role="option"
                  aria-selected={option === current}
                  className="palette-option"
                  onClick={() => {
                    activate(option);
                  }}
                >
                  <OptionText option={option} />
                </div>
              ))}
            </div>
          );
        })}
      </div>
      {project === null && projects !== undefined && <p className="muted">No project yet.</p>}
      {noHits && <p className="muted">The spec search found no node for &apos;{text}&apos;.</p>}
      {failures.map(([name, message]) => (
        <p key={name} className="palette-error" role="alert">
          <Icon name="alert" />
          <span>
            {name} could not be read: <span className="verbatim">{message}</span>
          </span>
        </p>
      ))}
      <div className="palette-foot">
        <p className="palette-hint muted">
          <kbd>Up</kbd> and <kbd>Down</kbd> move, <kbd>Enter</kbd> opens, <kbd>Esc</kbd> closes.
        </p>
        <button type="button" className="button button-quiet" onClick={onClose}>
          Close
        </button>
      </div>
    </Dialog>
  );
}
