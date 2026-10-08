import { memo, useId, useRef, type ReactNode } from "react";
import type { RoundAnswerEntry, RoundItem } from "../api/types";
import { sectionHash } from "../app/routes";
import { severityLook } from "../inbox/labels";
import { StagedChoice } from "../inbox/StagedChoice";
import { Badge } from "../ui/Badge";
import { Icon } from "../ui/Icon";
import { useFocusLater } from "../ui/useFocusLater";
import { capOf, draftOf, offers, stateOf, textOf, type Draft, type DraftPick } from "./drafts";
import { shownSlot, shownText, utf8Bytes } from "./text";

/**
 * Agent text of several lines as the sheet prints it: verbatim, never markdown, its controls
 * written `\u{..}`; isolated from the page's direction, each line taking its own.
 */
function AgentText({ text }: { text: string }) {
  return (
    <p className="round-text" dir="auto">
      {shownText(text)}
    </p>
  );
}

/** A one-line slot of agent text (a label, a title): LF and CR as `\n`, `\r`, isolated from its neighbours. */
function Slot({ text }: { text: string }) {
  return <bdi>{shownSlot(text)}</bdi>;
}

/**
 * The text a pick sends, typed in full, never cut: its UTF-8 bytes against the cap beside it (a
 * text over the cap is sent all the same; the daemon's refusal decides).
 */
function DraftText({
  label,
  value,
  cap,
  sent,
  missing,
  onChange,
}: {
  label: string;
  value: string;
  cap: number;
  /** The text as it is sent, which the counter counts. */
  sent: string;
  /** What to write when nothing is written yet; null when something is. */
  missing: string | null;
  onChange: (value: string) => void;
}) {
  const fieldId = useId();
  const counterId = useId();
  const missingId = useId();
  const bytes = utf8Bytes(sent);
  const over = bytes > cap;
  return (
    <div className="round-field">
      <label className="field-label" htmlFor={fieldId}>
        {label}
      </label>
      <textarea
        id={fieldId}
        className="field"
        rows={3}
        dir="auto"
        spellCheck
        value={value}
        aria-invalid={over}
        aria-describedby={missing === null ? counterId : `${counterId} ${missingId}`}
        onChange={(event) => {
          onChange(event.target.value);
        }}
      />
      <p id={counterId} className={over ? "byte-count byte-count-over" : "byte-count"}>
        {over && <Icon name="alert" />}
        <span className="byte-count-value">{`${String(bytes)}/${String(cap)} bytes`}</span>
        {over && <span>: over the cap, so the daemon refuses it; nothing is cut</span>}
      </p>
      {missing !== null && (
        <p id={missingId} className="note">
          <Icon name="info" />
          <span>{missing}</span>
        </p>
      )}
    </div>
  );
}

/** A target on the Concerns line: `<title> (<ID or path>)`, linked to the spec tree. */
function Concern({ project, target }: { project: string; target: RoundItem["targets"][number] }) {
  const name = target.id ?? target.path;
  if (name === null) {
    return <span className="muted">A target no longer found</span>;
  }
  return (
    <a href={sectionHash(project, "tree", name)}>
      {target.title === null ? (
        <span className="mono">
          <Slot text={name} />
        </span>
      ) : (
        <>
          <Slot text={target.title} /> (<span className="mono">{shownSlot(name)}</span>)
        </>
      )}
    </a>
  );
}

/**
 * One item of the round, in the sheet's order and with the text sheet's fields
 * (`docs/canon/round.md` "Sheet"): its ID, kind (raw) and severity, text, working answer, the price
 * of another answer, options, Concerns and Waiting as links; no worktree, branch, gap type, author
 * or evidence. Its controls follow its structure: with options, each option, Reject with a reason,
 * Leave open; without, Take the working answer (when given), Your answer, Reject, Leave open. Nothing
 * is picked for the owner, the recommended option only marked. A staged choice shows as the Inbox
 * shows it, with its terminal command and Unstage. Memoised: typing in one item renders that item
 * alone (a sheet holds up to 200), so the callbacks take the item and are stable.
 */
export const RoundItemCard = memo(function RoundItemCard({
  project,
  item,
  draft,
  result,
  busy,
  unstageRefusal,
  onDraft,
  onUnstage,
}: {
  project: string;
  item: RoundItem;
  draft: Draft | undefined;
  /** What the last POST made of this item's answer when it was not staged. */
  result: RoundAnswerEntry | undefined;
  /** A write is on its way: Unstage stays focusable but does nothing. */
  busy: boolean;
  unstageRefusal: string | null;
  onDraft: (id: string, draft: Draft) => void;
  onUnstage: (item: RoundItem) => void;
}) {
  const headingId = useId();
  const choices = useRef<HTMLFieldSetElement>(null);
  const focusLater = useFocusLater();
  const state = stateOf(item, draft);
  const current = draft !== undefined && offers(item, draft) ? draft : undefined;
  const shown: DraftPick = current?.pick ?? "open";
  const hasOptions = item.options.length > 0;
  const group = `round-${item.id}`;

  /** A pick on the version shown: the texts typed are kept. */
  function pick(next: DraftPick, option: number | null = null) {
    const base = draft ?? draftOf(item);
    onDraft(item.id, { ...base, pick: next, option: next === "option" ? option : base.option, updatedAt: item.updated_at });
  }

  /** Text typed for the pick shown, on the version shown. */
  function write(field: "answer" | "reason", value: string) {
    const base = draft ?? draftOf(item);
    onDraft(item.id, { ...base, [field]: value, updatedAt: item.updated_at });
  }

  /** One radio of the item's group; a click on the checked one picks it again on the version shown. */
  function radio(next: DraftPick, label: ReactNode, option: number | null = null) {
    const checked = shown === next && (next !== "option" || current?.option === option);
    return (
      <label key={next === "option" ? `option-${String(option)}` : next} className={next === "option" ? "option-radio" : "option-radio round-radio"}>
        <input
          type="radio"
          name={group}
          value={next === "option" ? `option-${String(option)}` : next}
          checked={checked}
          onChange={() => {
            pick(next, option);
          }}
          onClick={() => {
            pick(next, option);
          }}
        />
        {label}
      </label>
    );
  }

  const cap = capOf(shown);
  const sentText = current === undefined ? "" : textOf(current);

  return (
    <li className="round-item" data-item={item.id}>
      <article className="round-card" aria-labelledby={headingId}>
        <header className="round-head">
          <h2 id={headingId} className="round-id mono" tabIndex={-1}>
            {item.id}
          </h2>
          <span className="kind-tag">
            <Slot text={item.kind} />
          </span>
          {item.severity !== null && <Badge name="Severity" look={severityLook(item.severity)} />}
          {item.staged_at !== null && (
            <span className="staged-mark" title={`Staged ${item.staged_at}`}>
              <Icon name="terminal" />
              Staged
            </span>
          )}
          <a className="round-open" href={sectionHash(project, "inbox", item.id)}>
            Open in Inbox<span className="sr-only">: {item.id}</span>
          </a>
        </header>

        <AgentText text={item.summary} />

        {item.working_answer !== null && (
          <div className="round-part">
            <p className="round-label">Working answer</p>
            <AgentText text={item.working_answer} />
          </div>
        )}
        {item.price_of_other !== null && (
          <div className="round-part">
            <p className="round-label">Another answer costs</p>
            <AgentText text={item.price_of_other} />
          </div>
        )}

        {(item.targets.length > 0 || item.tasks.length > 0) && (
          <dl className="round-links">
            {item.targets.length > 0 && (
              <div>
                <dt>Concerns</dt>
                <dd>
                  <ul className="round-link-list">
                    {item.targets.map((target, index) => (
                      <li key={`${String(index)}-${target.id ?? target.path ?? ""}`}>
                        <Concern project={project} target={target} />
                      </li>
                    ))}
                  </ul>
                </dd>
              </div>
            )}
            {item.tasks.length > 0 && (
              <div>
                <dt>Waiting</dt>
                <dd>
                  <ul className="round-link-list">
                    {item.tasks.map((task) => (
                      <li key={task.id}>
                        <a href={sectionHash(project, "tasks", task.id)}>
                          {task.title === null ? (
                            <span className="mono">{task.id}</span>
                          ) : (
                            <>
                              <Slot text={task.title} /> (<span className="mono">{task.id}</span>)
                            </>
                          )}
                        </a>
                      </li>
                    ))}
                  </ul>
                </dd>
              </div>
            )}
          </dl>
        )}

        {item.staged !== null && item.staged_at !== null && (
          <StagedChoice
            id={item.id}
            review={item}
            busy={busy}
            refusal={unstageRefusal}
            onUnstage={() => {
              onUnstage(item);
            }}
          />
        )}

        <fieldset ref={choices} className="option-choice round-choice">
          <legend>
            {item.staged_at === null ? "Answer" : "Answer again: replaces the staged choice"}{" "}
            <span className="sr-only">{item.id}</span>
          </legend>
          {hasOptions
            ? item.options.map((option, index) =>
                radio(
                  "option",
                  <span className="option-radio-body">
                    <span className="option-radio-label">
                      {/* The index as `spec approve --option` and the sheet write it. */}
                      <span className="mono">[{index}]</span> <Slot text={option.label} />
                      {index === item.recommendation && (
                        <span className="recommended-mark">
                          <Icon name="recommended" />
                          Recommended
                        </span>
                      )}
                    </span>
                    <span className="option-radio-detail">
                      <Slot text={option.effect} />
                    </span>
                    <span className="option-radio-detail">
                      Price: <Slot text={option.price} />
                    </span>
                  </span>,
                  index,
                ),
              )
            : (
                <>
                  {item.working_answer !== null && radio("working", <span className="option-radio-label">Take the working answer</span>)}
                  {radio("answer", <span className="option-radio-label">Your answer</span>)}
                </>
              )}
          {radio("reject", <span className="option-radio-label">Reject with a reason</span>)}
          {radio("open", <span className="option-radio-label">Leave open</span>)}
        </fieldset>

        {cap !== null && current !== undefined && (
          <DraftText
            label={shown === "reject" ? `Reason for rejecting ${item.id}` : `Your answer to ${item.id}`}
            value={shown === "reject" ? current.reason : current.answer}
            cap={cap}
            sent={sentText}
            missing={sentText === "" ? (shown === "reject" ? "Write a reason to include this item." : "Write your answer to include this item.") : null}
            onChange={(value) => {
              write(shown === "reject" ? "reason" : "answer", value);
            }}
          />
        )}

        {state === "changed" && draft !== undefined && (
          <div className="notice round-changed">
            <p className="notice-title">
              <Icon name="alert" />
              <span>Changed since you answered</span>
            </p>
            {current === undefined ? (
              // The revised item no longer offers the pick: nothing of it can be kept.
              <p>The item was revised after you picked your answer, and no longer offers it; it is left out until you pick again.</p>
            ) : (
              <>
                <p>The item was revised after you picked your answer; it is left out until you pick again.</p>
                <button
                  type="button"
                  className="button"
                  onClick={() => {
                    onDraft(item.id, { ...current, updatedAt: item.updated_at });
                    // The notice goes with its button: focus the answer kept.
                    focusLater(() => choices.current?.querySelector<HTMLElement>("input:checked") ?? choices.current);
                  }}
                >
                  Keep my answer for this version <span className="sr-only">of {item.id}</span>
                </button>
              </>
            )}
          </div>
        )}

        {result !== undefined && (
          <div className="refusal">
            <p className="refusal-title">
              <Icon name="alert" />
              <span>
                {result.outcome === "refused"
                  ? "The daemon refused this answer; nothing was staged for it."
                  : `The daemon answered ${result.outcome} for this answer.`}
              </span>
            </p>
            {result.reason !== null && <p className="verbatim">{result.reason}</p>}
          </div>
        )}
      </article>
    </li>
  );
});
