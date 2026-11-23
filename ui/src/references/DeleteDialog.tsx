import { useRef, useState } from "react";
import { apiErrorOf } from "../api/client";
import { useReferenceWrite } from "../api/referenceQueries";
import type { Reference } from "../api/types";
import { useTranslate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import { Dialog } from "../ui/Dialog";
import { Icon } from "../ui/Icon";
import { shownSlot } from "../ui/text";
import { isLink } from "./labels";

// Delete… (docs/features/work-references.md "UI", Delete;
// docs/canon/work-references.md "Delete and collection"): the page's dialog is the consent a terminal
// asks with `[y/N]`. It says what is erased,
// that a tombstone stays and that the file goes when no other reference names it; Cancel takes the
// focus first, so a stray Enter deletes nothing. One DELETE: while it is out nothing closes and a
// second press does nothing; a refusal keeps the dialog with the daemon's words (a hub's 403: only
// its author or an `owner` deletes it).

export function DeleteDialog({ project, reference, onClose, onDeleted }: { project: string; reference: Reference; onClose: () => void; onDeleted: () => void }) {
  const t = useTranslate();
  const write = useReferenceWrite(project);
  const out = useRef(false);
  const [pending, setPending] = useState(false);
  const [refusal, setRefusal] = useState<{ text: string; count: number } | null>(null);

  async function remove() {
    if (out.current) {
      return;
    }
    out.current = true;
    setPending(true);
    try {
      await write.mutateAsync({ write: "delete", id: reference.id });
      out.current = false;
      onDeleted();
    } catch (error) {
      out.current = false;
      setPending(false);
      setRefusal((last) => ({ text: apiErrorOf(error).message, count: (last?.count ?? 0) + 1 }));
    }
  }

  function cancel() {
    if (!out.current) {
      onClose();
    }
  }

  return (
    <Dialog title={t("references.delete.title", { id: reference.id })} onClose={cancel} className="decision-dialog card-dialog">
      <div className="card-form" aria-busy={pending}>
        <p className="dialog-text">
          {fill(isLink(reference) ? t("references.delete.textLink", { who: slot("who") }) : t("references.delete.text", { who: slot("who") }), { who: <bdi>{shownSlot(reference.actor)}</bdi> })}
        </p>
        {refusal !== null && (
          <div key={refusal.count} className="refusal" role="alert">
            <p className="refusal-title">
              <Icon name="alert" />
              <span>{t("board.view.refused")}</span>
            </p>
            <p className="verbatim" lang="en">
              {refusal.text}
            </p>
          </div>
        )}
        <div className="dialog-actions">
          <button
            type="button"
            className="button button-danger"
            aria-disabled={pending}
            onClick={() => {
              void remove();
            }}
          >
            {pending ? t("references.delete.sending") : t("references.delete.submit")}
          </button>
          <button type="button" className="button" aria-disabled={pending} data-autofocus="" onClick={cancel}>
            {t("board.dialog.cancel")}
          </button>
        </div>
      </div>
    </Dialog>
  );
}
