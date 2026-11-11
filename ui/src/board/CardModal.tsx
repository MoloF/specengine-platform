import type { ReactNode } from "react";
import { cardPageHash } from "../app/routes";
import { useTranslate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import { Button, buttonClasses } from "../ui/Button";
import { Dialog } from "../ui/Dialog";
import { Icon } from "../ui/Icon";
import { shownSlot } from "../ui/text";

/**
 * A card over the board (`#/<p>/board/<C>`, `ui/README.md` "Screen rules", Dialogs): a modal dialog
 * labelled by its heading, the card's title, where focus starts; the board and its tools inert
 * beneath, so no column moves. It is the work item (CardPanel.tsx `CardView`), 1200 px wide: `view`
 * lays its heading out under the breadcrumbs, with "Open as a page" and Close on the breadcrumb line
 * as subtle buttons; the children (its Move menu) follow. Esc, Close and a click on the scrim close
 * it, the card holding no typed text; while a write is out none of them does.
 */
export function CardModal({
  project,
  id,
  title,
  pending,
  onClose,
  view,
  children,
}: {
  project: string;
  id: string;
  /** The card's title once read; its ID until then. */
  title: string | null;
  /** A write is out: nothing closes. */
  pending: boolean;
  onClose: () => void;
  /** The work item around the dialog's heading and the modal's tools. */
  view: (frame: { heading: ReactNode; tools: ReactNode }) => ReactNode;
  children?: ReactNode;
}) {
  const t = useTranslate();

  function close() {
    if (!pending) {
      onClose();
    }
  }

  const tools = (
    <>
      <a className={buttonClasses("subtle", "compact")} href={cardPageHash(project, id)}>
        <Icon name="cardPage" />
        <span>{t("board.modal.openPage")}</span>
      </a>
      <Button appearance="subtle" spacing="compact" icon="rejected" aria-disabled={pending} onClick={close}>
        <span>{fill(t("board.modal.close", { what: slot("what") }), { what: <span className="sr-only">{t("board.modal.closeWhat")}</span> })}</span>
      </Button>
    </>
  );

  return (
    <Dialog
      title={<bdi>{title === null ? id : shownSlot(title)}</bdi>}
      onClose={close}
      className="card-modal"
      closeOnScrim
      focusTitle
      frame={(heading) => view({ heading, tools })}
    >
      {children}
    </Dialog>
  );
}
