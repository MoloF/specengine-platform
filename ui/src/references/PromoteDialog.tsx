import { useState } from "react";
import { apiErrorOf } from "../api/client";
import { usePromote } from "../api/referenceQueries";
import type { Proposal, Reference } from "../api/types";
import { sectionHash } from "../app/routes";
import { actingName, useActing } from "../board/acting";
import { ActingNote, SelectField, TextArea, TextField, WriteDialog } from "../board/CardDialogs";
import { useTranslate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import { useAnnounce } from "../ui/announcer";
import { Icon } from "../ui/Icon";
import { shownSlot } from "../ui/text";

// Promote to canon… (docs/canon/work-references-promotion-surfaces.md "UI"; docs/canon/work-references-promotion.md
// "The rule"): a stored file's tile proposes it into git beside a document. The Document is chosen
// among what the part is under (a node's own; a card's targets, the first chosen; a proposal's
// target) or typed as a REF; the file's name, "what matters" and the rationale are prefilled, the
// first two left to the daemon's own when unchanged. One POST under the acting name stores a
// `reference` proposal and nothing else: the file and the document's entry are written by
// `apply_proposal` only once a person confirms it in the Inbox (ADR-0004). A 200 shows the stored
// proposal's ID, linked to its Inbox item, with the daemon's notes (the file it adds, LFS or plain);
// a refusal (a 409's rule, a 503's line, a 404, a 400 or a 403) keeps the dialog and the typed text
// with the daemon's words. Nothing waits on it: the reference and its card go on as they were (ADR-0012).

/** The Document choice that takes a typed REF: every other choice is a target's index. */
const TYPED = "typed";

/** A typed value as the body sends it: blank, or equal to what the daemon takes by default, null. */
function given(typed: string, own: string | null): string | null {
  const value = typed.trim();
  return value === "" || value === (own ?? "").trim() ? null : value;
}

export function PromoteDialog({
  project,
  reference,
  documents,
  onClose,
}: {
  project: string;
  reference: Reference;
  /** What the part is under, each a REF the daemon resolves to its document, the first chosen at first: a node; a card's targets; a proposal's targets. */
  documents: readonly string[];
  onClose: () => void;
}) {
  const t = useTranslate();
  const [acting] = useActing();
  const promote = usePromote(project);
  const announce = useAnnounce();
  const [choice, setChoice] = useState(documents.length > 0 ? "0" : TYPED);
  const [typed, setTyped] = useState("");
  const [name, setName] = useState(reference.name ?? "");
  const [matters, setMatters] = useState(reference.matters ?? "");
  // Null until the person types one: the default follows the Document chosen.
  const [rationale, setRationale] = useState<string | null>(null);
  const [checked, setChecked] = useState(false);
  const [proposed, setProposed] = useState<Proposal | null>(null);

  const to = choice === TYPED ? typed.trim() : (documents[Number(choice)] ?? "");
  const why = rationale ?? t("references.promote.rationaleDefault", { id: reference.id, ref: to });

  if (proposed !== null) {
    const id = proposed.id ?? "";
    return (
      <WriteDialog title={t("references.promote.title", { id: reference.id })} action={null} sending="" dismiss={t("references.promote.close")} onClose={onClose} onSubmit={() => Promise.resolve(false)}>
        <div className="reference-proposed">
          <p className="reference-proposed-line">
            <Icon name="open" />
            <span>
              {fill(t("references.promote.proposed", { id: slot("id") }), {
                id: (
                  <a className="mono" href={sectionHash(project, "inbox", id)}>
                    {shownSlot(id)}
                  </a>
                ),
              })}
            </span>
          </p>
          {proposed.notes.length > 0 && (
            <ul className="note-list" aria-label={t("references.promote.notes")}>
              {proposed.notes.map((note, index) => (
                <li key={`${String(index)}-${note}`} lang="en">
                  {shownSlot(note)}
                </li>
              ))}
            </ul>
          )}
        </div>
      </WriteDialog>
    );
  }

  return (
    <WriteDialog
      title={t("references.promote.title", { id: reference.id })}
      action={t("references.promote.submit")}
      sending={t("references.promote.sending")}
      onClose={onClose}
      check={() => {
        setChecked(true);
        if (to === "") {
          return t("references.promote.missing");
        }
        return why.trim() === "" ? t("references.promote.rationaleMissing") : null;
      }}
      onSubmit={async () => {
        try {
          const made = await promote.mutateAsync({
            id: reference.id,
            body: { to, name: given(name, reference.name), matters: given(matters, reference.matters), rationale: why.trim(), as: actingName(acting) },
          });
          setProposed(made);
          announce(t("references.promote.proposed", { id: made.id ?? "" }));
          // Kept open, showing what was stored: the dismiss takes the focus the submit had.
          return false;
        } catch (error) {
          return apiErrorOf(error).message;
        }
      }}
    >
      <p className="dialog-text">{t("references.promote.text")}</p>
      {documents.length > 0 && (
        <SelectField
          label={t("references.promote.document")}
          value={choice}
          onChange={setChoice}
          options={[...documents.map((document, index) => ({ value: String(index), label: shownSlot(document) })), { value: TYPED, label: t("references.promote.other") }]}
          hint={t("references.promote.documentHint")}
          required
          autoFocus
        />
      )}
      {choice === TYPED && (
        <TextField
          label={t("references.promote.typed")}
          value={typed}
          onChange={setTyped}
          hint={t("references.promote.typedHint")}
          required
          autoFocus={documents.length === 0}
          invalid={checked && to === ""}
        />
      )}
      <TextField label={t("references.promote.name")} value={name} onChange={setName} hint={t("references.promote.nameHint")} />
      <TextArea label={t("references.matter.label")} value={matters} onChange={setMatters} hint={t("references.promote.mattersHint")} />
      <TextArea label={t("references.promote.rationale")} value={why} onChange={setRationale} required invalid={checked && why.trim() === ""} />
      <ActingNote />
    </WriteDialog>
  );
}
