import { useRef, useState } from "react";
import { apiErrorOf } from "../api/client";
import { useCommentWrite } from "../api/commentQueries";
import type { CommentDocument } from "../api/types";
import { useTranslate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import { Dialog } from "../ui/Dialog";
import { Icon } from "../ui/Icon";
import { shownSlot } from "../ui/text";

// Delete… (docs/features/comments.md "UI"; docs/canon/comments.md "Delete"): the page's dialog is the
// consent a terminal asks with `[y/N]`. It says that every version's text is erased and a tombstone
// stays; Cancel takes the focus first, so a stray Enter deletes nothing. One DELETE: while it is out
// nothing closes and a second press does nothing; a refusal keeps the dialog with the daemon's words
// (a hub's 403: only its author or an `owner` deletes it).

export function DeleteDialog({ project, comment, onClose, onDeleted }: { project: string; comment: CommentDocument; onClose: () => void; onDeleted: () => void }) {
  const t = useTranslate();
  const write = useCommentWrite(project);
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
      await write.mutateAsync({ write: "delete", id: comment.id });
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
    <Dialog title={t("comments.delete.title", { id: comment.id })} onClose={cancel} className="decision-dialog card-dialog">
      <div className="card-form" aria-busy={pending}>
        <p className="dialog-text">{fill(t("comments.delete.text", { who: slot("who") }), { who: <bdi>{shownSlot(comment.actor)}</bdi> })}</p>
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
            {pending ? t("comments.delete.sending") : t("comments.delete.submit")}
          </button>
          <button type="button" className="button" aria-disabled={pending} data-autofocus="" onClick={cancel}>
            {t("board.dialog.cancel")}
          </button>
        </div>
      </div>
    </Dialog>
  );
}
