import { useId, useRef, useState, type ReactNode } from "react";
import { useClient } from "../api/provider";
import type { Reference } from "../api/types";
import { ActOff } from "../board/ActOff";
import { useTranslate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import { byAgent } from "../inbox/labels";
import { Button } from "../ui/Button";
import { Dialog } from "../ui/Dialog";
import { Icon } from "../ui/Icon";
import { shownSlot, shownText } from "../ui/text";
import { formatUtc } from "../ui/time";
import { useFocusLater } from "../ui/useFocusLater";
import { classWord, factWords, isLink, isWebAddress, previewed, promotable, shownAs, sizeWords, thumbnailOf, thumbnailSize, viewable } from "./labels";

// One reference as the References part draws it (docs/features/work-references.md "UI", Tiles): its
// head (title or name, its class or a link's word, its ID), who attached it and when, its size and
// facts; then its preview: a raster image as an `<img loading="lazy">` of its agent copy, or of the
// file only when it reaches agents as it is, else an "Open image" button that loads nothing until
// pressed; either opens the file in a dialog, a sound `<audio controls preload="none">`, a video
// `<video controls preload="metadata">`, a model, a PDF or anything else a download link (a `.blend`
// with the daemon's warning), a link an anchor opening a new tab with no opener, no referrer and no
// follow, its address whole. Each file's address is the client's `blobUrl`, served sandboxed; every
// text is people's, drawn as React text, never Markdown and never HTML. A file the store lost says
// so; a deleted reference says who deleted it and when, and offers nothing. A stored file alone offers
// Promote to canon… (docs/canon/work-references-promotion-surfaces.md "UI"): never a link, a deleted or a lost one.

/** A stored UTC time, as stored to the minute. */
function At({ at }: { at: string }) {
  const t = useTranslate();
  return <time dateTime={at}>{formatUtc(at, t)}</time>;
}

/** What a tile's head names it by: its title, else its file's name, else a link's word. */
export function headOf(reference: Reference, t: ReturnType<typeof useTranslate>): string {
  return reference.title ?? reference.name ?? t("references.untitled");
}

/**
 * The opened image: the file itself, not its agent copy, with its download; an image past the pixel
 * budget (or of unknown size and large) shows its facts and Download alone, never decoded here.
 */
function ImageDialog({ title, src, facts, drawn, onClose }: { title: string; src: string; facts: readonly string[]; drawn: boolean; onClose: () => void }) {
  const t = useTranslate();
  return (
    <Dialog title={<bdi>{title}</bdi>} onClose={onClose} closeOnScrim className="reference-dialog">
      {drawn ? (
        <img className="reference-full" src={src} alt={title} />
      ) : (
        <p className="dialog-text reference-too-large">{facts.length === 0 ? t("references.tooLarge") : t("references.tooLargeFacts", { facts: facts.join(", ") })}</p>
      )}
      <div className="dialog-actions">
        <a className="button" href={src} download>
          {t("references.download")}
        </a>
        <Button data-autofocus="" onClick={onClose}>
          {t("references.close")}
        </Button>
      </div>
    </Dialog>
  );
}

/** The preview a tile draws, by how it is shown. */
function Preview({ project, reference, title }: { project: string; reference: Reference; title: string }) {
  const t = useTranslate();
  const client = useClient();
  const opener = useRef<HTMLButtonElement>(null);
  const focusLater = useFocusLater();
  const [open, setOpen] = useState(false);
  const shown = shownAs(reference);
  const b3 = reference.b3 ?? "";
  const file = client.blobUrl(project, b3);
  switch (shown) {
    case "deleted":
      return reference.deleted === null ? null : (
        <p className="reference-deleted muted">
          {fill(t("references.deleted", { by: slot("by"), at: slot("at") }), { by: <bdi>{shownSlot(reference.deleted.by)}</bdi>, at: <At at={reference.deleted.at} /> })}
        </p>
      );
    case "lost":
      return <p className="reference-lost muted">{t("references.lost")}</p>;
    case "link": {
      const url = reference.url ?? "";
      return isWebAddress(url) ? (
        <a className="reference-url" href={url} target="_blank" rel="noopener noreferrer nofollow">
          {shownSlot(url)}
        </a>
      ) : (
        <p className="reference-url mono">{shownSlot(url)}</p>
      );
    }
    case "image": {
      // A thumbnail from the copy, or from the file only when it reaches agents as it is; a large
      // original is loaded only when the person opens it.
      const thumbnail = thumbnailOf(reference);
      const reserved = thumbnailSize(reference);
      const facts = [...factWords(reference.meta, t), ...(reference.size === null ? [] : [sizeWords(reference.size, t)])];
      const show = () => {
        setOpen(true);
      };
      return (
        <>
          {thumbnail === null ? (
            <Button ref={opener} spacing="compact" className="reference-open" onClick={show}>
              {facts.length === 0 ? t("references.openImage") : t("references.openImageFacts", { facts: facts.join(", ") })}
            </Button>
          ) : (
            <button ref={opener} type="button" className="reference-thumb" aria-label={t("references.open", { name: title })} onClick={show}>
              <img
                loading="lazy"
                decoding="async"
                src={client.blobUrl(project, thumbnail)}
                alt={title}
                width={reserved?.width}
                height={reserved?.height}
              />
            </button>
          )}
          {open && (
            <ImageDialog
              title={title}
              src={file}
              facts={facts}
              drawn={viewable(reference)}
              onClose={() => {
                setOpen(false);
                focusLater(() => opener.current);
              }}
            />
          )}
        </>
      );
    }
    case "audio":
      return <audio className="reference-media" controls preload="none" src={file} aria-label={title} />;
    case "video":
      return <video className="reference-media" controls preload="metadata" src={file} aria-label={title} />;
    case "download":
      return (
        <a className="reference-download" href={file} download>
          <Icon name="document" />
          <span>{t("references.downloadNamed", { name: shownSlot(reference.name ?? b3) })}</span>
        </a>
      );
  }
}

/** The tile's line of facts: who, an agent's mark and role, when, a node's revision, the class, size, facts and the agent copy. */
function Facts({ reference }: { reference: Reference }) {
  const t = useTranslate();
  const { author } = reference;
  const parts: ReactNode[] = [];
  if (reference.class !== null && reference.deleted === null) {
    parts.push(
      <span key="class" className="kind-tag">
        {classWord(reference.class, t)}
      </span>,
    );
  }
  const words = [
    ...(reference.size !== null && reference.deleted === null ? [sizeWords(reference.size, t)] : []),
    ...factWords(reference.meta, t),
    ...(reference.agent_b3 !== null ? [t("references.agentCopy")] : []),
  ];
  return (
    <p className="reference-facts">
      <bdi className="reference-who">{shownSlot(reference.actor)}</bdi>
      {byAgent(author) && (
        <>
          <span className="plain-tag">{t("references.agent")}</span>
          {author.role !== null && (
            <span className="kind-tag" lang="en">
              {shownSlot(author.role)}
            </span>
          )}
        </>
      )}
      <span className="muted">
        <At at={reference.created_at} />
      </span>
      {reference.on_rev !== null && <span className="muted">{t("references.onRev", { rev: reference.on_rev })}</span>}
      {parts}
      {words.length > 0 && <span className="muted">{words.join(" · ")}</span>}
    </p>
  );
}

/** One reference, its preview and texts, its Promote to canon… on a stored file, its Delete (on as `deletes` says, else off with its line). */
export function ReferenceTile({
  project,
  reference,
  deletes,
  onDelete,
  onPromote,
}: {
  project: string;
  reference: Reference;
  deletes: boolean;
  onDelete: (trigger: HTMLElement) => void;
  onPromote: (trigger: HTMLElement) => void;
}) {
  const t = useTranslate();
  const headingId = useId();
  const live = reference.deleted === null;
  const title = headOf(reference, t);
  const shown = shownAs(reference);
  return (
    <article className="reference" aria-labelledby={headingId} data-reference={reference.id} tabIndex={-1}>
      <p className="reference-head">
        <bdi id={headingId} className={live ? "reference-title" : "reference-title mono"} dir="auto">
          {live ? shownSlot(title) : shownSlot(reference.id)}
        </bdi>
        {live && isLink(reference) && <span className="kind-tag">{t("references.link")}</span>}
        {live && <span className="mono muted reference-id">{shownSlot(reference.id)}</span>}
      </p>
      <Facts reference={reference} />
      {live && reference.title !== null && reference.name !== null && <p className="reference-name mono">{shownSlot(reference.name)}</p>}
      <Preview project={project} reference={reference} title={shownSlot(title)} />
      {live && previewed(shown) && reference.b3 !== null && <FileLink project={project} b3={reference.b3} />}
      {live && reference.matters !== null && (
        <div className="reference-matters">
          <p className="reference-matters-label">{t("references.matters")}</p>
          <p className="reference-matters-text" dir="auto">
            {shownText(reference.matters)}
          </p>
        </div>
      )}
      {live && reference.warning !== null && (
        <p className="note reference-warning">
          <Icon name="alert" />
          <span lang="en">{shownSlot(reference.warning)}</span>
        </p>
      )}
      {live && (
        <div className="reference-acts" role="group" aria-label={t("references.acts", { id: reference.id })}>
          {promotable(reference) && (
            <Button
              spacing="compact"
              appearance="subtle"
              aria-haspopup="dialog"
              onClick={(event) => {
                onPromote(event.currentTarget);
              }}
            >
              {t("references.promote.action")}
            </Button>
          )}
          <Button
            spacing="compact"
            appearance="subtle"
            aria-haspopup="dialog"
            aria-disabled={!deletes}
            onClick={(event) => {
              if (deletes) {
                onDelete(event.currentTarget);
              }
            }}
          >
            {t("references.delete.action")}
          </Button>
        </div>
      )}
      {live && !deletes && <ActOff act="delete_reference" />}
    </article>
  );
}

/** A previewed file's own download, beside its preview. */
function FileLink({ project, b3 }: { project: string; b3: string }) {
  const t = useTranslate();
  const client = useClient();
  return (
    <a className="reference-file-link" href={client.blobUrl(project, b3)} download>
      {t("references.download")}
    </a>
  );
}
