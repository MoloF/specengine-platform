import { useEffect, useId, useRef, useState, type SubmitEvent } from "react";
import { useTranslate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import { Prose } from "../markdown/Prose";
import { Button } from "../ui/Button";
import { Icon } from "../ui/Icon";
import { shownSlot } from "../ui/text";

// A comment's text as it is written (docs/features/comments.md "UI", Composer): a textarea and its
// Preview, the preview rendered only through src/markdown/ exactly as the thread will draw it. Under
// it the name it goes out under (a hub's login, else the acting name as typed), then the send. A send
// is one call: while it is out the text holds still and a second send does nothing. A refusal keeps
// the text and says the daemon's words in an alert; a stale edit (`stale`) says so first, the thread
// read again meanwhile, so the next send goes on the version now stored.

/** What a send came to: sent (the caller clears or closes), or the daemon's words, a stale version's with its notice. */
export type Sent = null | { refusal: string; stale: boolean };

/** The send's words and the text's label, which say what the composer is for: a comment, a reply, an edit. */
export interface ComposerWords {
  label: string;
  send: string;
  sending: string;
}

export function Composer({
  project,
  words,
  as,
  initial = "",
  focus = false,
  onSend,
  onCancel,
}: {
  project: string;
  words: ComposerWords;
  /** Who the text goes out as, shown. */
  as: string;
  /** The text it opens with: an edit's stored body. */
  initial?: string;
  /** Takes the focus as it opens: a reply's and an edit's. */
  focus?: boolean;
  /** Sends the text as typed; the answer says what became of it. */
  onSend: (body: string) => Promise<Sent>;
  /** A reply's and an edit's way out, with nothing sent; a new comment's composer has none. */
  onCancel?: () => void;
}) {
  const t = useTranslate();
  const id = useId();
  const field = useRef<HTMLTextAreaElement>(null);
  const out = useRef(false);
  const [draft, setDraft] = useState(initial);
  const [preview, setPreview] = useState(false);
  const [pending, setPending] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);
  const [refusal, setRefusal] = useState<{ text: string; stale: boolean; count: number } | null>(null);
  // Each blank send asks for the text once it is drawn again: Preview gives way to it first.
  const [refocus, setRefocus] = useState(0);

  useEffect(() => {
    if (focus) {
      field.current?.focus();
    }
  }, [focus]);

  useEffect(() => {
    if (refocus > 0) {
      field.current?.focus();
    }
  }, [refocus]);

  async function submit(event: SubmitEvent<HTMLFormElement>) {
    event.preventDefault();
    if (out.current) {
      return;
    }
    if (draft.trim() === "") {
      setProblem(t("comments.blank"));
      setPreview(false);
      setRefocus((count) => count + 1);
      return;
    }
    setProblem(null);
    out.current = true;
    setPending(true);
    const sent = await onSend(draft);
    out.current = false;
    setPending(false);
    if (sent === null) {
      setRefusal(null);
      setDraft("");
      return;
    }
    // A new alert for each refusal, so a second one is spoken too.
    setRefusal((last) => ({ text: sent.refusal, stale: sent.stale, count: (last?.count ?? 0) + 1 }));
  }

  return (
    <form
      className="comment-composer"
      noValidate
      aria-busy={pending}
      onSubmit={(event) => {
        void submit(event);
      }}
    >
      <div className="comment-composer-head">
        <label className="field-label" htmlFor={id}>
          {words.label}
        </label>
        <div className="text-mode" role="group" aria-label={t("comments.view")}>
          <button
            type="button"
            className="text-mode-option"
            aria-pressed={!preview}
            onClick={() => {
              setPreview(false);
            }}
          >
            {t("comments.write")}
          </button>
          <button
            type="button"
            className="text-mode-option"
            aria-pressed={preview}
            onClick={() => {
              setPreview(true);
            }}
          >
            {t("comments.preview")}
          </button>
        </div>
      </div>
      {preview ? (
        <div className="comment-preview" dir="auto">
          {draft.trim() === "" ? <p className="muted">{t("comments.previewEmpty")}</p> : <Prose source={draft} links={null} project={project} baseLevel={4} frontMatter={false} />}
        </div>
      ) : (
        <textarea
          ref={field}
          id={id}
          className="field comment-field"
          rows={3}
          dir="auto"
          value={draft}
          readOnly={pending}
          aria-required
          aria-invalid={problem !== null}
          onChange={(event) => {
            setDraft(event.target.value);
          }}
        />
      )}
      <p className="comment-as muted">{fill(t("comments.as", { name: slot("name") }), { name: <bdi>{shownSlot(as)}</bdi> })}</p>
      {problem !== null && (
        <p className="field-error" role="alert">
          {problem}
        </p>
      )}
      {refusal !== null && (
        <div key={refusal.count} className="refusal" role="alert">
          <p className="refusal-title">
            <Icon name="alert" />
            <span>{refusal.stale ? t("comments.stale") : t("board.view.refused")}</span>
          </p>
          <p className="verbatim" lang="en">
            {refusal.text}
          </p>
        </div>
      )}
      <div className="dialog-actions">
        <Button type="submit" aria-disabled={pending}>
          {pending ? words.sending : words.send}
        </Button>
        {onCancel !== undefined && (
          <Button
            aria-disabled={pending}
            onClick={() => {
              if (!out.current) {
                onCancel();
              }
            }}
          >
            {t("board.dialog.cancel")}
          </Button>
        )}
      </div>
    </form>
  );
}
