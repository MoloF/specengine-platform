import { lazy, Suspense, useState } from "react";
import { useCommentRaise } from "../api/commentQueries";
import type { CardRaiseBody, CommentDocument, CommentRaiseBody, Mandate } from "../api/types";
import { actingName, useActing } from "../board/acting";
import { ActingNote, raiseAction, RaiseNotice, SelectField, TargetChoice, TextArea, TextField, useRaiseWith, WriteDialog } from "../board/CardDialogs";
import { MoveMenu, type MoveChoice } from "../board/MoveMenu";
import { useTranslate } from "../i18n";
import { gapLabel, KNOWN_GAP_TYPES } from "../inbox/labels";
import { firstBytes, PREFILL_BYTES, TURNS, type Turn } from "./labels";

// Turn into… (docs/features/comments.md "UI"; docs/canon/comments.md "Turning into an item"): a menu
// of three, each a dialog prefilled from the comment (its text the body's first 1 024 bytes, its
// targets what the comment is about) posting to the comment's one door, `.../comments/:id/items`.
// Ask a question… and Record a decision… are the card's own dialogs on that door; Report a
// discrepancy… is this file's. The intake's rules and dedup are the daemon's: hits keep the dialog
// with "anyway", a refusal its words. The item is decided as any item, by a person (ADR-0004).

/** The card's two raise dialogs, in the items' chunk. */
const AskDialog = lazy(() => import("../board/CardItems").then((module) => ({ default: module.AskDialog })));
const DecideDialog = lazy(() => import("../board/CardItems").then((module) => ({ default: module.DecideDialog })));

/** The menu's lines, in the spec's order. */
export function turnChoices(t: ReturnType<typeof useTranslate>): (MoveChoice & { turn: Turn })[] {
  return [
    { key: TURNS.question, turn: TURNS.question, label: t("board.ask.action"), detail: null, icon: "open" },
    { key: TURNS.decision, turn: TURNS.decision, label: t("board.decide.action"), detail: null, icon: "approved" },
    { key: TURNS.discrepancy, turn: TURNS.discrepancy, label: t("comments.turn.discrepancy"), detail: null, icon: "cardDeparted" },
  ];
}

/** The menu under its button. */
export function TurnMenu({ id, anchor, onChoose, onClose }: { id: string; anchor: DOMRect; onChoose: (turn: Turn) => void; onClose: () => void }) {
  const t = useTranslate();
  return (
    <MoveMenu
      label={t("comments.turn.menu", { id })}
      anchor={anchor}
      choices={turnChoices(t)}
      empty=""
      onChoose={(choice) => {
        onChoose(choice.turn);
      }}
      onClose={onClose}
    />
  );
}

/**
 * The dialog of one turn. `targets` are what the item is about, each checked at first: a node
 * comment's node, a proposal comment's targets, a card comment's card's targets. `mandate` a card
 * comment's task's, which frames a decision as the card's own dialog does (docs/features/task-mandate.md
 * "Acceptance criteria", V2); null elsewhere. `onRaised` hears what to say once stored; the caller closes.
 */
export function TurnDialog({
  project,
  comment,
  turn,
  targets,
  mandate,
  onClose,
  onRaised,
}: {
  project: string;
  comment: CommentDocument;
  turn: Turn;
  targets: readonly string[];
  mandate: Mandate | null;
  onClose: () => void;
  onRaised: (said: string) => void;
}) {
  const t = useTranslate();
  const raise = useCommentRaise(project);
  const text = firstBytes(comment.body ?? "", PREFILL_BYTES);
  const about = { id: comment.id, targets: [...targets] };
  const post = (body: CardRaiseBody) => raise.mutateAsync({ id: comment.id, body });
  if (turn === TURNS.discrepancy) {
    return <DiscrepancyDialog project={project} comment={comment} text={text} targets={targets} onClose={onClose} onRaised={onRaised} />;
  }
  return (
    <Suspense fallback={null}>
      {turn === TURNS.question ? (
        <AskDialog project={project} card={about} question={text} post={post} title={t("comments.turn.askTitle", { id: comment.id })} onClose={onClose} onRaised={onRaised} />
      ) : (
        <DecideDialog project={project} card={{ ...about, mandate }} question={text} post={post} title={t("comments.turn.decideTitle", { id: comment.id })} onClose={onClose} onRaised={onRaised} />
      )}
    </Suspense>
  );
}

/** A discrepancy's form as typed. */
interface DiscrepancyForm {
  summary: string;
  observed: string;
  documented: string;
  gap: string;
  chosen: readonly string[];
}

/**
 * Report a discrepancy… from a comment: its summary (the body's first bytes), what is observed and
 * what is documented, required; the gap, chosen; the targets, each checked at first. One POST with
 * `kind` the discrepancy's word (labels.ts `TURNS`); the daemon builds its evidence and options (docs/features/comments.md "HTTP").
 */
function DiscrepancyDialog({ project, comment, text, targets, onClose, onRaised }: { project: string; comment: CommentDocument; text: string; targets: readonly string[]; onClose: () => void; onRaised: (said: string) => void }) {
  const t = useTranslate();
  const [acting] = useActing();
  const commentRaise = useCommentRaise(project);
  const raise = useRaiseWith<CommentRaiseBody>((body) => commentRaise.mutateAsync({ id: comment.id, body }));
  const [form, setForm] = useState<DiscrepancyForm>({ summary: text, observed: "", documented: "", gap: "", chosen: [...targets] });
  const [checked, setChecked] = useState(false);
  const set = (key: "summary" | "observed" | "documented" | "gap") => (value: string) => {
    setForm((current) => ({ ...current, [key]: value }));
  };
  const blank = (value: string) => checked && value.trim() === "";
  return (
    <WriteDialog
      title={t("comments.turn.reportTitle", { id: comment.id })}
      action={raiseAction(raise.outcome, t("comments.turn.report"), t("comments.turn.reportAnyway"), t("comments.turn.reportAgain"))}
      sending={t("comments.turn.reporting")}
      onClose={onClose}
      check={() => {
        setChecked(true);
        if ([form.summary, form.observed, form.documented, form.gap].some((value) => value.trim() === "")) {
          return t("comments.turn.needsTexts");
        }
        return form.chosen.length === 0 ? t("board.ask.needsTarget") : null;
      }}
      onSubmit={async () => {
        const body: CommentRaiseBody = {
          kind: TURNS.discrepancy,
          node_ids: targets.filter((target) => form.chosen.includes(target)),
          text: form.summary.trim(),
          gap_type: form.gap,
          observed: form.observed.trim(),
          documented: form.documented.trim(),
          as: actingName(acting),
        };
        const raised = await raise.send(body);
        if ("stored" in raised) {
          onRaised(t("comments.turn.reported", { id: raised.stored }));
          return null;
        }
        return "kept" in raised ? false : raised.refusal;
      }}
    >
      <TextField label={t("board.field.summary")} value={form.summary} onChange={set("summary")} required autoFocus invalid={blank(form.summary)} />
      <TextArea label={t("inbox.card.observed")} value={form.observed} onChange={set("observed")} required invalid={blank(form.observed)} />
      <TextArea label={t("inbox.card.documented")} value={form.documented} onChange={set("documented")} required invalid={blank(form.documented)} />
      <SelectField
        label={t("board.dialog.gap")}
        value={form.gap}
        onChange={set("gap")}
        placeholder={t("comments.turn.choose")}
        options={KNOWN_GAP_TYPES.map((gap) => ({ value: gap, label: gapLabel(gap, t) ?? gap }))}
        required
        invalid={blank(form.gap)}
      />
      <TargetChoice
        targets={targets}
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
