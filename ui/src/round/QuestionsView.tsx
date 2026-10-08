import { lazy, Suspense, type KeyboardEvent } from "react";
import { useRound } from "../api/queries";
import { SECTIONS } from "../app/sections";
import { useOpenShortcuts } from "../app/shortcuts";
import { hasModifier, isTextField } from "../ui/keys";
import { Skeleton } from "../ui/states";
import { useRoundEntry, type RoundMemory } from "./memory";

/** The sheet, its drafts and their POST arrive in a chunk of their own; the heading stays put meanwhile. */
const RoundScreen = lazy(() => import("./RoundScreen").then((module) => ({ default: module.RoundScreen })));

const ABOUT = SECTIONS.find((section) => section.id === "questions")?.about ?? "";

/**
 * Questions (`#/<project>/questions`, `docs/features/ui-round.md`): the open questions and
 * discrepancies on one sheet, in round order, answered here and staged at once; each answer is
 * confirmed on a terminal by `spec round answer --staged`, one `[y/N]` each (ADR-0035, ADR-0038).
 * One `getRound` on entering, read again on every proposal or task event (useLiveQueue). No hotkey
 * answers or stages (W-18): `?` lists the keys, as on every screen.
 */
export function QuestionsView({ project, memory }: { project: string; memory: RoundMemory }) {
  const openShortcuts = useOpenShortcuts();
  const { limit } = useRoundEntry(memory, project);
  const round = useRound(project, limit);

  function onKeyDown(event: KeyboardEvent<HTMLElement>) {
    if (event.defaultPrevented || hasModifier(event) || isTextField(event.target)) {
      return;
    }
    if (event.key === "?") {
      event.preventDefault();
      openShortcuts();
    }
  }

  return (
    <section
      className="view questions-view"
      aria-labelledby="questions-title"
      aria-busy={round.data === undefined && round.error === null}
      onKeyDown={onKeyDown}
    >
      <header className="view-head">
        <h1 id="questions-title" tabIndex={-1}>
          Questions
        </h1>
        <p className="view-about">
          {ABOUT} Pick an answer per item and stage them at once; each is asked again on a terminal, one at a time.
          Nothing waits on them: agents work on with their working answer.
        </p>
      </header>
      <Suspense fallback={<Skeleton label={`Loading the question round of ${project}`} lines={5} />}>
        <RoundScreen project={project} round={round} memory={memory} />
      </Suspense>
    </section>
  );
}
