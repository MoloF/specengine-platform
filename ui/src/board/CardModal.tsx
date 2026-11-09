import type { ReactNode } from "react";
import { cardPageHash } from "../app/routes";
import { useTranslate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import { Dialog } from "../ui/Dialog";
import { Icon } from "../ui/Icon";
import { shownSlot } from "../ui/text";

/**
 * A card over the board (`#/<p>/board/<C>`, `ui/README.md` "Screen rules", Dialogs): a modal dialog
 * labelled by its heading, the card's title, where focus starts; the board and its tools inert
 * beneath, so no column moves. Its body is the card's (CardPanel.tsx), its acts open their dialogs
 * over it. Esc, Close and a click on the scrim close it, the card holding no typed text; while a
 * write is out none of them does. "Open as a page" is the card's own page.
 */
export function CardModal({
  project,
  id,
  title,
  pending,
  onClose,
  children,
}: {
  project: string;
  id: string;
  /** The card's title once read; its ID until then. */
  title: string | null;
  /** A write is out: nothing closes. */
  pending: boolean;
  onClose: () => void;
  children: ReactNode;
}) {
  const t = useTranslate();

  function close() {
    if (!pending) {
      onClose();
    }
  }

  return (
    <Dialog title={<bdi>{title === null ? id : shownSlot(title)}</bdi>} onClose={close} className="card-modal" closeOnScrim focusTitle>
      <div className="card-modal-bar">
        <span className="mono board-card-id">{id}</span>
        <a className="card-modal-page" href={cardPageHash(project, id)}>
          <Icon name="cardPage" />
          <span>{t("board.modal.openPage")}</span>
        </a>
        <button type="button" className="button button-quiet" aria-disabled={pending} onClick={close}>
          <Icon name="rejected" />
          <span>{fill(t("board.modal.close", { what: slot("what") }), { what: <span className="sr-only">{t("board.modal.closeWhat")}</span> })}</span>
        </button>
      </div>
      <div className="card-modal-body">{children}</div>
    </Dialog>
  );
}
