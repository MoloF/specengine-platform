import { useId, useRef, useState } from "react";
import { apiErrorOf } from "../api/client";
import { useReferences, type ReferencesQuery } from "../api/referenceQueries";
import type { Reference } from "../api/types";
import { useAllowed, useHubLogin } from "../board/acting";
import { useTranslate } from "../i18n";
// The references' English, merged as this chunk lands (src/i18n/enReferences.ts).
import "../i18n/enReferences";
import { useAnnounce } from "../ui/announcer";
import { Button } from "../ui/Button";
import { Icon } from "../ui/Icon";
import { ErrorPanel, Skeleton } from "../ui/states";
import { useFocusLater } from "../ui/useFocusLater";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { AddFileDialog } from "./AddFileDialog";
import { AddLinkDialog } from "./AddLinkDialog";
import { DeleteDialog } from "./DeleteDialog";
import { deleteStands } from "./labels";
import type { ReferencesProps } from "./lazy";
import { ReferenceTile } from "./ReferenceTile";

// The References part (docs/features/work-references.md "UI"; docs/canon/work-references.md), a chunk
// of its own above the Comments part under a card (modal and page), a document (modal and page) and a
// proposal (open or closed): people's material for the work, never truth, and no hold on anything
// (ADR-0012). Tiles newest first, Load older paging back by `next_before`; Add a file… and Add a link…
// at its head. Each live reference offers Delete (one's own, any where `allowed` lists
// `delete_reference`, any locally; else off with its line); a deleted one shows who deleted it and
// offers nothing. Every write is one call; the list shown is the daemon's, read again after each write
// and on a `reference.*` event. Who acts is the hub's login or the acting name as typed (the Board's).

/** The part's heading at its level: the card's and a proposal's `h3`, a document's `h2` on its page. */
function PartHeading({ level, id, children }: { level: 2 | 3; id: string; children: string }) {
  return level === 2 ? (
    <h2 id={id} className="issue-section-title">
      {children}
    </h2>
  ) : (
    <h3 id={id} className="issue-section-title">
      {children}
    </h3>
  );
}

/** The pages as one list, newest first: the newest page is read first, each page oldest first within. */
export function newestFirst(query: Pick<ReferencesQuery, "data">): Reference[] {
  return (query.data?.pages ?? []).flatMap((page) => [...page.references].reverse());
}

/** What the part has open. */
type Open = { what: "addFile" | "addLink"; trigger: HTMLElement } | { what: "delete"; reference: Reference; trigger: HTMLElement } | null;

export function ReferencesPart({ project, on, level }: ReferencesProps) {
  const t = useTranslate();
  const headingId = useId();
  const section = useRef<HTMLElement>(null);
  const read = useReferences(project, on);
  const failure = useRetainedFailure(read.error, read.isFetching);
  const announce = useAnnounce();
  const focusLater = useFocusLater();
  const login = useHubLogin();
  const allows = useAllowed(project);
  const deletesAny = allows("delete_reference");
  const [open, setOpen] = useState<Open>(null);
  const all = newestFirst(read);
  const first = read.data?.pages[0];
  const { hasNextPage, isFetchingNextPage, fetchNextPage } = read;

  function close(back: () => HTMLElement | null) {
    setOpen(null);
    focusLater(back);
  }

  /** A tile's article by its reference's ID, once drawn. */
  function tileOf(id: string): HTMLElement | null {
    return Array.from(section.current?.querySelectorAll<HTMLElement>("[data-reference]") ?? []).find((element) => element.dataset.reference === id) ?? null;
  }

  let body;
  if (read.data === undefined) {
    body =
      failure === null ? (
        <div aria-busy="true">
          <Skeleton label={t("app.lazy.references")} lines={2} />
        </div>
      ) : (
        <ErrorPanel
          title={t("references.failed")}
          message={apiErrorOf(failure).message}
          retrying={read.isFetching}
          attempt={read.errorUpdateCount}
          onRetry={() => {
            void read.refetch({ cancelRefetch: false });
          }}
        />
      );
  } else {
    body = (
      <>
        {first !== undefined && first.notes.length > 0 && (
          <ul className="note-list" aria-label={t("references.notes")}>
            {first.notes.map((note, index) => (
              <li key={`${String(index)}-${note}`} lang="en">
                {note}
              </li>
            ))}
          </ul>
        )}
        {read.error !== null && (
          <p className="note" role="alert">
            <Icon name="alert" />
            <span lang="en">{apiErrorOf(read.error).message}</span>
          </p>
        )}
        {all.length === 0 ? (
          <p className="muted">{t("references.none")}</p>
        ) : (
          <ul className="references-list">
            {all.map((reference) => (
              <li key={reference.id}>
                <ReferenceTile
                  project={project}
                  reference={reference}
                  deletes={deleteStands(reference, login, deletesAny)}
                  onDelete={(trigger) => {
                    setOpen({ what: "delete", reference, trigger });
                  }}
                />
              </li>
            ))}
          </ul>
        )}
        {hasNextPage && (
          <Button
            appearance="subtle"
            aria-disabled={isFetchingNextPage}
            onClick={() => {
              if (!isFetchingNextPage) {
                void fetchNextPage();
              }
            }}
          >
            {isFetchingNextPage ? t("references.loadingOlder") : t("references.older")}
          </Button>
        )}
      </>
    );
  }

  return (
    <section ref={section} className="references" aria-label={t("app.references")} data-on={on}>
      <PartHeading level={level} id={headingId}>
        {first === undefined ? t("app.references") : t("references.titleCount", { count: first.total })}
      </PartHeading>
      <p className="references-about muted">{t("references.about")}</p>
      <div className="references-adds" role="group" aria-label={t("references.adds")}>
        <Button
          spacing="compact"
          aria-haspopup="dialog"
          onClick={(event) => {
            setOpen({ what: "addFile", trigger: event.currentTarget });
          }}
        >
          {t("references.addFile")}
        </Button>
        <Button
          spacing="compact"
          aria-haspopup="dialog"
          onClick={(event) => {
            setOpen({ what: "addLink", trigger: event.currentTarget });
          }}
        >
          {t("references.addLink")}
        </Button>
      </div>
      {body}
      {open?.what === "addFile" && (
        <AddFileDialog
          project={project}
          on={on}
          onClose={() => {
            const { trigger } = open;
            close(() => trigger);
          }}
          onAttached={(id) => {
            announce(t("references.said.attached", { id }));
            const { trigger } = open;
            close(() => tileOf(id) ?? trigger);
          }}
        />
      )}
      {open?.what === "addLink" && (
        <AddLinkDialog
          project={project}
          on={on}
          onClose={() => {
            const { trigger } = open;
            close(() => trigger);
          }}
          onAdded={(id) => {
            announce(t("references.said.attached", { id }));
            const { trigger } = open;
            close(() => tileOf(id) ?? trigger);
          }}
        />
      )}
      {open?.what === "delete" && (
        <DeleteDialog
          project={project}
          reference={open.reference}
          onClose={() => {
            const { trigger } = open;
            close(() => (trigger.isConnected ? trigger : tileOf(open.reference.id)));
          }}
          onDeleted={() => {
            announce(t("references.said.deleted", { id: open.reference.id }));
            // The acts go with the texts: the tombstone's article takes the focus.
            const { id } = open.reference;
            close(() => tileOf(id));
          }}
        />
      )}
    </section>
  );
}
