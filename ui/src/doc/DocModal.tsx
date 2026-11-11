import { useDocNode } from "../api/queries";
import { openDoc } from "../app/location";
import { docPageHash } from "../app/routes";
import { useTranslate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import { Button, buttonClasses } from "../ui/Button";
import { Dialog } from "../ui/Dialog";
import { Icon } from "../ui/Icon";
import { DocFrame } from "./DocFrame";

// The document modal (docs/features/doc-modal.md "Acceptance criteria", DM-9, DM-10): the `doc` pair
// of any project route, over the view and over any dialog beneath, always the top one. A work item
// labelled by its `h2`, the title once read (the REF until then), where focus starts; 1200 px or the
// viewport less 2rem, the whole screen under 36rem, its body scrolling (under 36rem the dialog scrolls
// as one). Esc, Close and the scrim close it as a card's modal closes; while a card write is out none
// of them does. The frame (./DocFrame.tsx) holds the dialog's own `h2` through the parts' loading.

export function DocModal({
  project,
  docRef,
  pending,
  insideOpen,
  onInside,
  onClose,
}: {
  project: string;
  docRef: string;
  /** A card write is out: nothing closes. */
  pending: boolean;
  insideOpen: boolean;
  onInside: (open: boolean) => void;
  onClose: () => void;
}) {
  const t = useTranslate();
  const read = useDocNode(project, docRef);
  const title = read.data?.nodes[0]?.title ?? null;

  function close() {
    if (!pending) {
      onClose();
    }
  }

  const tools = (
    <>
      <a className={buttonClasses("subtle", "compact")} href={docPageHash(project, docRef)}>
        <Icon name="cardPage" />
        <span>{t("doc.openPage")}</span>
      </a>
      <Button appearance="subtle" spacing="compact" icon="rejected" aria-disabled={pending} onClick={close}>
        <span>{fill(t("doc.close", { what: slot("what") }), { what: <span className="sr-only">{t("doc.closeWhat")}</span> })}</span>
      </Button>
    </>
  );

  return (
    <Dialog
      title={<bdi>{title ?? docRef}</bdi>}
      onClose={close}
      className="doc-modal"
      closeOnScrim
      focusTitle
      top
      frame={(heading) => <DocFrame project={project} docRef={docRef} frame="modal" heading={heading} tools={tools} insideOpen={insideOpen} onInside={onInside} onCrumb={openDoc} />}
    />
  );
}
