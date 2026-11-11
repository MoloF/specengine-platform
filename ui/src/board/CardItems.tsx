import { lazy, Suspense, useId, useRef, useState } from "react";
import { apiErrorOf } from "../api/client";
import { useCardWriting, useProposal, useStageDecision, withActing, type ChildrenQuery } from "../api/queries";
import type { Card, CardItem, CardRaiseBody, CardRow, CardSubtaskItem, Proposal, Team } from "../api/types";
import { DocLink } from "../doc/DocLink";
import { useTranslate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import { confirmedLine, stagedAnnouncement } from "../inbox/decisions";
import { StagedChoice } from "../inbox/StagedChoice";
import { choiceOf, draftOf, type Draft, type DraftPick } from "../round/drafts";
import { useAnnounce } from "../ui/announcer";
import { Button } from "../ui/Button";
import { Icon } from "../ui/Icon";
import { Lozenge } from "../ui/Lozenge";
import { ErrorPanel, Skeleton } from "../ui/states";
import { shownSlot, shownText } from "../ui/text";
import { formatUtc } from "../ui/time";
import { useFocusLater } from "../ui/useFocusLater";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { actingName, useActing, useAllowed, useConfirmer } from "./acting";
import { ActOff } from "./ActOff";
import { ActingNote, questionBodyOf, QuestionFields, raiseAction, raiseFormOf, RaiseNotice, raiseProblem, raiseTextsOf, TargetChoice, TextArea, useRaise, WriteDialog, type RaiseForm } from "./CardDialogs";
import { CardLink, type CardLinks } from "./CardLink";
import { flowTitleOf } from "./filter";
import { decisionDeparts, decisionNeedsWhy, itemStatusLook, noteAnswers, RAISED_KINDS, subtaskMandateLook, takesAnswer } from "./labels";

// A card's items and the acts that raise and answer them (docs/canon/card-questions.md "The rule";
// docs/features/card-questions.md "Acceptance criteria", U1, U3, U4), a chunk of its own: the work
// item's Activity draws each item's block here, at the event that raised it, and the head's two
// acts open their dialogs from here. Nothing here judges a kind, a state or a hit: the block shows the
// item as the card's read serves it, its state looked up in labels.ts; the daemon answers each write.

/** The Confirm dialog a hub's `[team]` entitles, in the Inbox's chunk: loaded only when opened. */
const ConfirmDialog = lazy(() => import("../inbox/ConfirmDialog").then((module) => ({ default: module.ConfirmDialog })));

/** A stored UTC time, as stored to the minute. */
function At({ at }: { at: string }) {
  const t = useTranslate();
  return <time dateTime={at}>{formatUtc(at, t)}</time>;
}

/** Agent or person text of several lines: verbatim, wrapped, its controls written `\u{..}`, isolated. */
function Text({ text }: { text: string }) {
  return <bdi className="board-text">{shownText(text)}</bdi>;
}

/**
 * One item's block in the Activity: its ID, its kind raw in a neutral badge (a project's word, under
 * `lang="en"`), its state's lozenge and its text; under them the working answer and the why. Then how
 * it stands: staged when it is; its record, a document link by the record's title, and who decided;
 * else a decision's note, which is a rejected item's answer. An open item takes Answer…, which a hub's
 * `allowed` without `stage` keeps idle with its line, and which opens nothing while a card write of the
 * project is out (`ui/README.md` "Screen rules", Dialogs).
 */
export function ItemBlock({ project, item }: { project: string; item: CardItem }) {
  const t = useTranslate();
  const headingId = useId();
  const allows = useAllowed(project);
  const writing = useCardWriting(project);
  const focusLater = useFocusLater();
  const trigger = useRef<HTMLButtonElement>(null);
  const [answering, setAnswering] = useState(false);
  const stages = allows("stage");
  const open = takesAnswer(item.status);
  return (
    <article className="board-item" aria-labelledby={headingId} data-item={item.id}>
      <p className="board-item-head">
        <span id={headingId} className="mono">
          {shownSlot(item.id)}
        </span>
        <span className="kind-tag" lang="en">
          {shownSlot(item.kind)}
        </span>
        <Lozenge {...itemStatusLook(item.status, t)} />
      </p>
      {item.summary !== null && (
        <p className="board-item-summary">
          <Text text={item.summary} />
        </p>
      )}
      <dl className="pairs board-item-facts">
        {item.working_answer !== null && (
          <>
            <dt>{t("board.items.workingAnswer")}</dt>
            <dd>
              <Text text={item.working_answer} />
            </dd>
          </>
        )}
        {item.why !== null && (
          <>
            <dt>{t("board.items.why")}</dt>
            <dd>
              <Text text={item.why} />
            </dd>
          </>
        )}
        {item.record_id !== null && (
          <>
            <dt>{t("board.items.record")}</dt>
            <dd>
              <DocLink project={project} docRef={item.record_id}>
                <bdi>{shownSlot(item.record_title ?? item.record_id)}</bdi>
              </DocLink>
            </dd>
          </>
        )}
        {item.decision_note !== null && (
          // A rejected item's reason is its answer; any other's note is the decider's note (labels.ts).
          <>
            <dt>{noteAnswers(item.status) ? t("board.items.answer") : t("inbox.card.decisionNote")}</dt>
            <dd>
              <Text text={item.decision_note} />
            </dd>
          </>
        )}
        {item.decided_by !== null && (
          <>
            <dt>{t("board.items.decidedBy")}</dt>
            <dd>
              <bdi>{shownSlot(item.decided_by)}</bdi>
            </dd>
          </>
        )}
      </dl>
      {item.staged_at !== null && (
        <p className="board-item-staged">
          <Icon name="terminal" />
          <span>{fill(t("board.items.staged", { at: slot("at") }), { at: <At at={item.staged_at} /> })}</span>
        </p>
      )}
      {open && (
        <div className="board-panel-actions">
          <Button
            ref={trigger}
            aria-disabled={!stages}
            onClick={() => {
              if (stages && !writing) {
                setAnswering(true);
              }
            }}
          >
            {t("board.answer.action")}
          </Button>
        </div>
      )}
      {open && !stages && <ActOff act="stage" />}
      {answering && (
        <AnswerDialog
          project={project}
          item={item}
          onClose={() => {
            setAnswering(false);
            focusLater(() => trigger.current);
          }}
        />
      )}
    </article>
  );
}

/** The picks an item offers by its structure, the Questions screen's (`docs/canon/round-ui.md` "Screen"): each option, else the working answer when given and an own answer; Reject with a reason always. */
function picksOf(review: Proposal): { pick: DraftPick; option: number | null }[] {
  const answers: { pick: DraftPick; option: number | null }[] =
    review.options.length > 0
      ? review.options.map((_option, index) => ({ pick: "option", option: index }))
      : [...(review.working_answer === null ? [] : [{ pick: "working" as const, option: null }]), { pick: "answer", option: null }];
  return [...answers, { pick: "reject", option: null }];
}

/**
 * Answer… on an open item (docs/features/card-questions.md "Acceptance criteria", U4): its review
 * document read once, its controls the Questions screen's by structure, never its kind, staged through
 * the decision route with the acting name. Staged, the dialog stays and shows the staged choice as the
 * Inbox does: when, the terminal command and Copy (fixed words and a checked ID), Unstage; on a hub,
 * Confirm where `[team]` lists `confirm`, the Inbox's dialog. A refusal keeps the dialog and the pick.
 */
function AnswerDialog({ project, item, onClose }: { project: string; item: CardItem; onClose: () => void }) {
  const t = useTranslate();
  const group = useId();
  const read = useProposal(project, item.id);
  const decide = useStageDecision(project);
  const confirmer = useConfirmer(project);
  const announce = useAnnounce();
  const [acting] = useActing();
  const [draft, setDraft] = useState<Draft | null>(null);
  const [staged, setStaged] = useState(false);
  const [unstageRefusal, setUnstageRefusal] = useState<string | null>(null);
  const [confirming, setConfirming] = useState<Team | null>(null);
  const failure = useRetainedFailure(read.error, read.isFetching);
  const review = read.data !== undefined && read.data.id !== null ? read.data : undefined;
  // The pick and its texts stay through a re-read; a stage goes on the version read now.
  const shown = draft ?? (review === undefined ? null : draftOf(review));

  function pick(next: DraftPick, option: number | null) {
    if (shown !== null) {
      setDraft({ ...shown, pick: next, option });
    }
  }

  function write(field: "answer" | "reason", value: string) {
    if (shown !== null) {
      setDraft({ ...shown, [field]: value });
    }
  }

  async function stage(): Promise<string | null | false> {
    const choice = shown === null ? null : choiceOf(shown);
    if (review === undefined || review.updated_at === null || choice === null) {
      return false;
    }
    let answered: Proposal;
    try {
      answered = await decide.mutateAsync(withActing({ id: item.id, stage: choice, updatedAt: review.updated_at }, actingName(acting)));
    } catch (error) {
      return apiErrorOf(error).message;
    }
    setStaged(true);
    setDraft(null);
    setUnstageRefusal(null);
    // Said as the Inbox says it, the choice sent and the time stored.
    announce(stagedAnnouncement(item.id, choice.decision, answered, t));
    return false;
  }

  async function unstage() {
    setUnstageRefusal(null);
    try {
      await decide.mutateAsync({ id: item.id, stage: null });
    } catch (error) {
      setUnstageRefusal(apiErrorOf(error).message);
    }
  }

  let body;
  if (read.data === undefined) {
    body =
      failure === null ? (
        <div aria-busy="true">
          <Skeleton label={t("inbox.card.reviewLoading", { id: item.id })} lines={3} />
        </div>
      ) : (
        <ErrorPanel
          title={t("inbox.card.reviewFailed", { id: item.id })}
          message={apiErrorOf(failure).message}
          retrying={read.isFetching}
          attempt={read.errorUpdateCount}
          onRetry={() => {
            void read.refetch({ cancelRefetch: false });
          }}
        />
      );
  } else if (review === undefined || shown === null) {
    body = (
      <ul className="note-list">
        {read.data.notes.map((note) => (
          <li key={note} lang="en">
            {note}
          </li>
        ))}
      </ul>
    );
  } else {
    const text = shown.pick === "reject" ? shown.reason : shown.answer;
    body = (
      <>
        {review.summary !== null && (
          <p className="dialog-text">
            <Text text={review.summary} />
          </p>
        )}
        {review.working_answer !== null && (
          <div className="round-part">
            <p className="round-label">{t("round.workingAnswer")}</p>
            <p className="dialog-text">
              <Text text={review.working_answer} />
            </p>
          </div>
        )}
        {review.price_of_other !== null && (
          <div className="round-part">
            <p className="round-label">{t("round.priceOfOther")}</p>
            <p className="dialog-text">
              <Text text={review.price_of_other} />
            </p>
          </div>
        )}
        {review.staged !== null && (
          <StagedChoice
            id={item.id}
            review={review}
            busy={decide.isPending}
            refusal={unstageRefusal}
            onUnstage={() => {
              void unstage();
            }}
            onConfirm={
              confirmer === null
                ? undefined
                : () => {
                    setConfirming(confirmer);
                  }
            }
            confirmBusy={decide.isPending}
          />
        )}
        <fieldset className="option-choice round-choice">
          <legend>{fill(t(review.staged === null ? "round.answerLegend" : "round.answerAgainLegend", { id: slot("id") }), { id: <span className="sr-only">{item.id}</span> })}</legend>
          {picksOf(review).map(({ pick: offered, option }) => {
            const choice = option === null ? null : review.options[option];
            return (
              <label key={option === null ? offered : `option-${String(option)}`} className={offered === "option" ? "option-radio" : "option-radio round-radio"}>
                <input
                  type="radio"
                  name={group}
                  checked={shown.pick === offered && (offered !== "option" || shown.option === option)}
                  onChange={() => {
                    pick(offered, option);
                  }}
                />
                {choice === null || choice === undefined ? (
                  <span className="option-radio-label">{offered === "working" ? t("round.takeWorking") : offered === "answer" ? t("round.yourAnswer") : t("round.reject")}</span>
                ) : (
                  <span className="option-radio-body">
                    <span className="option-radio-label">
                      <span className="mono">[{option}]</span> <bdi>{shownSlot(choice.label)}</bdi>
                      {option === review.recommendation && (
                        <span className="recommended-mark">
                          <Icon name="recommended" />
                          {t("round.recommended")}
                        </span>
                      )}
                    </span>
                    <span className="option-radio-detail">
                      <bdi>{shownSlot(choice.effect)}</bdi>
                    </span>
                    <span className="option-radio-detail">{fill(t("round.price", { price: slot("price") }), { price: <bdi>{shownSlot(choice.price)}</bdi> })}</span>
                  </span>
                )}
              </label>
            );
          })}
        </fieldset>
        {(shown.pick === "answer" || shown.pick === "reject") && (
          <TextArea
            label={shown.pick === "reject" ? t("round.reasonFor", { id: item.id }) : t("round.answerTo", { id: item.id })}
            value={text}
            onChange={(value) => {
              write(shown.pick === "reject" ? "reason" : "answer", value);
            }}
            required
          />
        )}
        <ActingNote />
      </>
    );
  }

  return (
    <>
      <WriteDialog
        title={t("board.answer.title", { id: item.id })}
        action={review === undefined ? null : t("round.stage", { count: 1 })}
        sending={t("round.staging")}
        dismiss={staged ? t("inbox.dialog.close") : undefined}
        // Staged on a hub that lets this login confirm, Confirm is the dialog's one primary (`docs/canon/ui-design.md` "Primitives").
        primary={review?.staged === null || confirmer === null}
        onClose={onClose}
        check={() => {
          if (shown === null || choiceOf(shown) === null) {
            return t("round.nonePicked");
          }
          if (shown.pick === "answer" && shown.answer.trim() === "") {
            return t("round.writeAnswer");
          }
          return shown.pick === "reject" && shown.reason.trim() === "" ? t("round.writeReason") : null;
        }}
        onSubmit={stage}
      >
        {body}
      </WriteDialog>
      {confirming !== null && (
        <Suspense fallback={null}>
          <ConfirmDialog
            project={project}
            id={item.id}
            team={confirming}
            onClose={() => {
              setConfirming(null);
            }}
            onConfirmed={(answered) => {
              setConfirming(null);
              announce(confirmedLine(item.id, answered, t));
              onClose();
            }}
          />
        </Suspense>
      )}
    </>
  );
}

/** What a raise dialog is told: the card raised on, how to close, and what to say once one is stored. */
interface RaiseDialogProps {
  project: string;
  card: Pick<Card, "id" | "targets">;
  onClose: () => void;
  /** Stored: the words to say; the caller closes the dialog. */
  onRaised: (said: string) => void;
  /** The daemon's line on the acting name, for the field that holds it. */
  onActingLine?: (line: string) => void;
}

/**
 * Ask a question… on a card with a target, open or closed (docs/features/card-questions.md
 * "Acceptance criteria", U3): the question, a working answer and what another answer would cost, all
 * required, about the card's targets, each checked at first. One `POST .../items` with `kind`
 * `question`; hits keep the dialog with Ask anyway; a refusal keeps it with its words. `question`
 * starts the question's text: a parent's objection names the decision it is about
 * (docs/features/task-mandate.md "Acceptance criteria", V3).
 */
export function AskDialog({ project, card, question = "", onClose, onRaised, onActingLine }: RaiseDialogProps & { question?: string }) {
  const t = useTranslate();
  const [acting] = useActing();
  const [form, setForm] = useState<RaiseForm>(() => ({ ...raiseFormOf(card.targets), text: question }));
  const [checked, setChecked] = useState(false);
  const raise = useRaise(project, card.id, onActingLine);
  return (
    <WriteDialog
      title={t("board.ask.title", { id: card.id })}
      action={raiseAction(raise.outcome, t("board.ask.submit"), t("board.ask.anyway"), t("board.ask.again"))}
      sending={t("board.ask.sending")}
      onClose={onClose}
      check={() => {
        setChecked(true);
        return raiseProblem(form, t("board.ask.needsTexts"), t("board.ask.needsTarget"));
      }}
      onSubmit={async () => {
        const raised = await raise.send(questionBodyOf(form, card.targets, actingName(acting)));
        if ("stored" in raised) {
          onRaised(t("board.ask.asked", { id: raised.stored }));
          return null;
        }
        return "kept" in raised ? false : raised.refusal;
      }}
    >
      <QuestionFields form={form} labels={{ text: t("board.ask.question"), working: t("board.ask.workingAnswer"), price: t("board.ask.price") }} checked={checked} autoFocus onChange={setForm} />
      <TargetChoice
        targets={card.targets}
        chosen={form.chosen}
        onChange={(chosen) => {
          setForm((current) => ({ ...current, chosen }));
        }}
      />
      <ActingNote />
      <RaiseNotice outcome={raise.outcome} />
    </WriteDialog>
  );
}

/** A decision's body: its texts and targets, `why` when given, else null, never an empty string (U3). */
function decisionBodyOf(form: RaiseForm, targets: readonly string[], as: string | null): CardRaiseBody {
  const why = form.why.trim();
  return { kind: RAISED_KINDS.decision, ...raiseTextsOf(form, targets), why: why === "" ? null : why, as };
}

/**
 * Record a decision… on a card with a target, open or closed (docs/features/card-questions.md
 * "Acceptance criteria", U3): what had to be decided, the decision and what another answer would
 * cost, required; why, optional; about the card's targets. One `POST .../items` with `kind`
 * `decision`, raised for the owner to confirm; hits and refusals as Ask a question…. The task's
 * mandate frames it (docs/features/task-mandate.md "Acceptance criteria", V2): on a binding task a
 * note that a decision departs from its brief, why still optional; on one its department decides,
 * a why the form asks, a blank one stopped before anything is sent. The daemon keeps `why` optional
 * either way.
 */
export function DecideDialog({ project, card, onClose, onRaised, onActingLine }: Omit<RaiseDialogProps, "card"> & { card: Pick<Card, "id" | "targets" | "mandate"> }) {
  const t = useTranslate();
  const [acting] = useActing();
  const [form, setForm] = useState<RaiseForm>(() => raiseFormOf(card.targets));
  const [checked, setChecked] = useState(false);
  const raise = useRaise(project, card.id, onActingLine);
  const departs = decisionDeparts(card.mandate);
  const needsWhy = decisionNeedsWhy(card.mandate);
  return (
    <WriteDialog
      title={t("board.decide.title", { id: card.id })}
      action={raiseAction(raise.outcome, t("board.decide.submit"), t("board.decide.anyway"), t("board.decide.again"))}
      sending={t("board.decide.sending")}
      onClose={onClose}
      check={() => {
        setChecked(true);
        const problem = raiseProblem(form, t("board.decide.needsTexts"), t("board.ask.needsTarget"));
        return problem ?? (needsWhy && form.why.trim() === "" ? t("board.mandate.whyNeeded") : null);
      }}
      onSubmit={async () => {
        const raised = await raise.send(decisionBodyOf(form, card.targets, actingName(acting)));
        if ("stored" in raised) {
          onRaised(t("board.decide.raised", { id: raised.stored }));
          return null;
        }
        return "kept" in raised ? false : raised.refusal;
      }}
    >
      {departs && (
        <p className="note">
          <Icon name="mandateBinding" />
          <span>{t("board.mandate.decideBinding")}</span>
        </p>
      )}
      <QuestionFields form={form} labels={{ text: t("board.decide.matter"), working: t("board.decide.decision"), price: t("board.decide.price") }} checked={checked} autoFocus onChange={setForm} />
      <TextArea
        label={needsWhy ? t("board.mandate.whyRequired") : t("board.decide.why")}
        value={form.why}
        required={needsWhy}
        invalid={needsWhy && checked && form.why.trim() === ""}
        onChange={(why) => {
          setForm((current) => ({ ...current, why }));
        }}
      />
      <TargetChoice
        targets={card.targets}
        chosen={form.chosen}
        onChange={(chosen) => {
          setForm((current) => ({ ...current, chosen }));
        }}
      />
      <ActingNote />
      <RaiseNotice outcome={raise.outcome} />
    </WriteDialog>
  );
}

/** The "Ask about it…" open: the decision it is about, the subtask's row, and the button that opened it. */
interface Objection {
  item: CardSubtaskItem;
  row: CardRow;
  trigger: HTMLElement;
}

/**
 * A parent's Decisions from subtasks (docs/canon/card-mandate.md "The rule";
 * docs/features/task-mandate.md "Acceptance criteria", V3): each decision its card's read serves in
 * `subtask_items`, in served order. Its head names the subtask, a link, with its title and flow's title
 * from the parent's children read, the ID alone while that read is out, and the subtask's mandate as
 * that row has it: a binding task's decision marked as a departure, an open one's as the department's.
 * Then the item's block; then Ask about it…, once the row is read and has a target: Ask a question… on
 * the subtask, its question naming the decision; while a card write of the project is out it opens
 * nothing (`ui/README.md` "Screen rules", Dialogs). Reads nothing of its own: the card's one read and
 * the children read Subtasks already holds. Confirming stays where it is: nothing here approves.
 */
export function SubtaskDecisions({ project, decisions, subtasks, links }: { project: string; decisions: readonly CardSubtaskItem[]; subtasks: ChildrenQuery; links: CardLinks }) {
  const t = useTranslate();
  const announce = useAnnounce();
  const focusLater = useFocusLater();
  const writing = useCardWriting(project);
  const [asking, setAsking] = useState<Objection | null>(null);
  const list = subtasks.data;
  const rowOf = (id: string) => list?.cards.find((row) => row.id === id);
  const flowOf = (name: string) => list?.flows.find((flow) => flow.name === name);

  function close() {
    const trigger = asking?.trigger ?? null;
    setAsking(null);
    focusLater(() => (trigger?.isConnected === true ? trigger : null));
  }

  return (
    <>
      <ul className="board-panel-list board-from-subtasks">
        {decisions.map((item) => {
          const row = rowOf(item.card_id);
          const look = row === undefined ? null : subtaskMandateLook(row.mandate, t);
          return (
            <li key={item.id} className="board-from-subtask">
              <p className="board-handoff">
                <CardLink id={item.card_id} links={links} />
                {row !== undefined && (
                  <>
                    <bdi className="board-handoff-title">{shownSlot(row.title)}</bdi>
                    <bdi className="muted">{shownSlot(flowTitleOf(flowOf(row.flow), row.flow))}</bdi>
                  </>
                )}
                {look !== null && <Lozenge {...look} />}
              </p>
              <ItemBlock project={project} item={item} />
              {row !== undefined && row.targets.length > 0 && (
                <div className="board-panel-actions">
                  <Button
                    aria-haspopup="dialog"
                    onClick={(event) => {
                      if (!writing) {
                        setAsking({ item, row, trigger: event.currentTarget });
                      }
                    }}
                  >
                    {t("board.fromSubtasks.ask")}
                  </Button>
                </div>
              )}
            </li>
          );
        })}
      </ul>
      {asking !== null && (
        <AskDialog
          project={project}
          card={{ id: asking.row.id, targets: asking.row.targets }}
          question={t("board.fromSubtasks.about", { id: asking.item.id })}
          onClose={close}
          onRaised={(said) => {
            announce(said);
            close();
          }}
        />
      )}
    </>
  );
}
