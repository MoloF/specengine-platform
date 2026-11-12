import { memo, useId, useRef, type ReactNode } from "react";
import type { RoundAnswerEntry, RoundItem } from "../api/types";
import { sectionHash } from "../app/routes";
import { DocLink } from "../doc/DocLink";
import { AskedFold, ItemLead, RecommendedMark } from "../inbox/ItemLead";
import { severityLook } from "../inbox/labels";
import { StagedChoice } from "../inbox/StagedChoice";
import { Badge } from "../ui/Badge";
import { Button } from "../ui/Button";
import { Icon } from "../ui/Icon";
import { shownSlot, shownText } from "../ui/text";
import { useFocusLater } from "../ui/useFocusLater";
import { answersOf, capOf, draftOf, offers, stateOf, textOf, type Draft, type DraftPick } from "./drafts";
import { utf8Bytes } from "./text";
import { useTranslate } from "../i18n";
import { fill, slot } from "../i18n/fill";

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
  const t = useTranslate();
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
        <span className="byte-count-value">{t("round.bytes", { bytes, cap })}</span>
        {over && <span>{t("round.overCap")}</span>}
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
  const t = useTranslate();
  const name = target.id ?? target.path;
  if (name === null) {
    return <span className="muted">{t("round.targetGone")}</span>;
  }
  return (
    <DocLink project={project} docRef={name}>
      {target.title === null ? (
        <span className="mono">
          <Slot text={name} />
        </span>
      ) : (
        <>
          <Slot text={target.title} /> (<span className="mono">{shownSlot(name)}</span>)
        </>
      )}
    </DocLink>
  );
}

/**
 * One item of the round, in the sheet's order and with the text sheet's fields
 * (`docs/canon/round.md` "Sheet"): its ID, kind (raw) and severity, text, working answer, the price
 * of another answer, options, Concerns and Waiting as links; no worktree, branch, gap type, author
 * or evidence. Its controls follow its structure (answersOf): each option it holds; with a price of
 * another answer, or with no options, Take the working answer (when given) and Your answer; then
 * Reject with a reason, Leave open. Nothing is picked for the owner, the recommended option only
 * marked with who made it (docs/features/plain-questions.md "Acceptance criteria", PQ12). An item
 * holding a gist leads with it and why it matters, then its controls; its text, working answer and
 * price are folded under them, collapsed. A staged choice shows as the Inbox
 * shows it, with its terminal command and Unstage. Memoised: typing in one item renders that item
 * alone (a sheet holds up to 200), so the callbacks take the item and are stable. Where a hub's
 * `allowed` omits `stage` the item is to read: its choices stand off and no answer is written. Where
 * it lists `confirm` the staged choice has its own Confirm: this item's alone, a round staging many
 * and each taking its own click (`docs/features/hub-confirm.md`).
 */
export const RoundItemCard = memo(function RoundItemCard({
  project,
  item,
  draft,
  result,
  busy,
  canStage,
  canConfirm,
  unstageRefusal,
  onDraft,
  onUnstage,
  onConfirm,
}: {
  project: string;
  item: RoundItem;
  draft: Draft | undefined;
  /** What the last POST made of this item's answer when it was not staged. */
  result: RoundAnswerEntry | undefined;
  /** A write is on its way: Unstage stays focusable but does nothing. */
  busy: boolean;
  /** False: a hub's `allowed` omits `stage`. */
  canStage: boolean;
  /** True: a hub's `allowed` lists `confirm`. */
  canConfirm: boolean;
  unstageRefusal: string | null;
  onDraft: (id: string, draft: Draft) => void;
  onUnstage: (item: RoundItem) => void;
  onConfirm: (item: RoundItem) => void;
}) {
  const t = useTranslate();
  const headingId = useId();
  const choices = useRef<HTMLFieldSetElement>(null);
  const focusLater = useFocusLater();
  const state = stateOf(item, draft);
  const current = draft !== undefined && offers(item, draft) ? draft : undefined;
  const shown: DraftPick = current?.pick ?? "open";
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

  // The item's parts: in the sheet's order without a gist, in the lead's with one (PQ12).
  const asked = (
    <>
      <AgentText text={item.summary} />

      {item.working_answer !== null && (
        <div className="round-part">
          <p className="round-label">{t("round.workingAnswer")}</p>
          <AgentText text={item.working_answer} />
        </div>
      )}
      {item.price_of_other !== null && (
        <div className="round-part">
          <p className="round-label">{t("round.priceOfOther")}</p>
          <AgentText text={item.price_of_other} />
        </div>
      )}
    </>
  );

  const links = (item.targets.length > 0 || item.tasks.length > 0) && (
    <dl className="round-links">
      {item.targets.length > 0 && (
        <div>
          <dt>{t("round.concerns")}</dt>
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
          <dt>{t("round.waiting")}</dt>
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
  );

  const staged = item.staged !== null && item.staged_at !== null && (
    <StagedChoice
      id={item.id}
      review={item}
      busy={busy || !canStage}
      refusal={unstageRefusal}
      onUnstage={() => {
        onUnstage(item);
      }}
      onConfirm={
        canConfirm
          ? () => {
              onConfirm(item);
            }
          : undefined
      }
      confirmBusy={busy}
      confirmLook="default"
    />
  );

  const controls = (
    <>
      <fieldset ref={choices} className="option-choice round-choice" disabled={!canStage}>
        <legend>
          {fill(t(item.staged_at === null ? "round.answerLegend" : "round.answerAgainLegend", { id: slot("id") }), { id: <span className="sr-only">{item.id}</span> })}
        </legend>
        {answersOf(item).map((offered) => {
          if (offered.pick !== "option") {
            return radio(offered.pick, <span className="option-radio-label">{offered.pick === "working" ? t("round.takeWorking") : t("round.yourAnswer")}</span>);
          }
          const index = offered.option;
          const option = item.options[index];
          return (
            option !== undefined &&
            radio(
              "option",
              <span className="option-radio-body">
                <span className="option-radio-label">
                  {/* The index as `spec approve --option` and the sheet write it. */}
                  <span className="mono">[{index}]</span> <Slot text={option.label} />
                  {index === item.recommendation && <RecommendedMark author={item.author} />}
                </span>
                <span className="option-radio-detail">
                  <Slot text={option.effect} />
                </span>
                <span className="option-radio-detail">{fill(t("round.price", { price: slot("price") }), { price: <Slot text={option.price} /> })}</span>
              </span>,
              index,
            )
          );
        })}
        {radio("reject", <span className="option-radio-label">{t("round.reject")}</span>)}
        {radio("open", <span className="option-radio-label">{t("round.leaveOpen")}</span>)}
      </fieldset>

      {canStage && cap !== null && current !== undefined && (
        <DraftText
          label={shown === "reject" ? t("round.reasonFor", { id: item.id }) : t("round.answerTo", { id: item.id })}
          value={shown === "reject" ? current.reason : current.answer}
          cap={cap}
          sent={sentText}
          missing={sentText === "" ? (shown === "reject" ? t("round.writeReason") : t("round.writeAnswer")) : null}
          onChange={(value) => {
            write(shown === "reject" ? "reason" : "answer", value);
          }}
        />
      )}
    </>
  );

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
          {item.severity !== null && <Badge name={t("round.severity")} look={severityLook(item.severity, t)} />}
          {item.staged_at !== null && (
            <span className="staged-mark" title={t("round.stagedAt", { at: item.staged_at })}>
              <Icon name="terminal" />
              {t("round.staged")}
            </span>
          )}
          <a className="round-open" href={sectionHash(project, "inbox", item.id)}>
            {fill(t("round.openInInbox", { id: slot("id") }), { id: <span className="sr-only">{t("round.openInInboxId", { id: item.id })}</span> })}
          </a>
        </header>

        {item.gist === null ? (
          <>
            {asked}
            {links}
            {staged}
            {controls}
          </>
        ) : (
          <>
            <ItemLead gist={item.gist} matters={item.matters} />
            {staged}
            {controls}
            <AskedFold>{asked}</AskedFold>
            {links}
          </>
        )}

        {state === "changed" && draft !== undefined && (
          <div className="notice round-changed">
            <p className="notice-title">
              <Icon name="alert" />
              <span>{t("round.changed")}</span>
            </p>
            {current === undefined ? (
              // The revised item no longer offers the pick: nothing of it can be kept.
              <p>{t("round.changedGone")}</p>
            ) : (
              <>
                <p>{t("round.changedKept")}</p>
                <Button
                  onClick={() => {
                    onDraft(item.id, { ...current, updatedAt: item.updated_at });
                    // The notice goes with its button: focus the answer kept.
                    focusLater(() => choices.current?.querySelector<HTMLElement>("input:checked") ?? choices.current);
                  }}
                >
                  {fill(t("round.keep", { id: slot("id") }), { id: <span className="sr-only">{t("round.keepOf", { id: item.id })}</span> })}
                </Button>
              </>
            )}
          </div>
        )}

        {result !== undefined && (
          <div className="refusal">
            <p className="refusal-title">
              <Icon name="alert" />
              <span>{result.outcome === "refused" ? t("round.answerRefused") : t("round.answerOutcome", { outcome: result.outcome })}</span>
            </p>
            {result.reason !== null && (
              <p className="verbatim" lang="en">
                {result.reason}
              </p>
            )}
          </div>
        )}
      </article>
    </li>
  );
});
