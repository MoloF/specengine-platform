import { useEffect, useId, useRef, useState } from "react";
import { apiErrorOf } from "../api/client";
import { useClient } from "../api/provider";
import { useReferenceWrite } from "../api/referenceQueries";
import { actingName, useActing } from "../board/acting";
import { ActingNote, TextArea, TextField, WriteDialog } from "../board/CardDialogs";
import { useTranslate } from "../i18n";
import { useAnnounce } from "../ui/announcer";
import { Button } from "../ui/Button";
import { Icon } from "../ui/Icon";
import { shownSlot } from "../ui/text";
import { agentCopyOf, type CopyOutcome } from "./agentCopy";
import { blobGone, copyRefused, FILE_MAX, guessedClass, needsMatters, PICKER_ACCEPT, sizeWords } from "./labels";
import { attachBody, uploadAll, type Uploaded, type UploadStep } from "./uploads";

// Add a file… (docs/features/work-references.md "UI", "The agent copy"): a picker and a drop zone take
// one file. Past 256 MiB it is refused before a byte is sent. A raster image past 1 568 px or 1 MiB
// first gets a smaller copy this browser makes for agents; the copy is uploaded first, then the file,
// each one `putBlob` with its progress and a Cancel that aborts it. Meanwhile a title and "what matters
// here" are typed, the latter marked required by the class the browser's type suggests, then by the
// daemon's own answer (`needs_matters`). Attach posts the target, the file's hash, its copy's (only
// where the daemon does not find the file agent-ready), the name, the title and "what matters" under
// the acting name. A 400 keeps the dialog and the uploaded file, so the next Attach sends nothing again;
// a 404 saying a named file is gone uploads it once more and attaches again; a 400 refusing the copy
// (`agent_b3: …`) attaches once more without it. Leaving the dialog aborts an upload in flight. Each
// step's start and each quarter of an upload are said to a screen reader, its end once. A file
// dropped anywhere on the dialog but its zone is taken by nothing, so the page never navigates to it.
// A raster image's size is read from its first bytes before any decode (./agentCopy.ts).

/** An upload as the dialog shows it. */
type Upload =
  | { phase: "idle" }
  | { phase: "tooBig"; size: number }
  | { phase: "preparing" }
  | { phase: "uploading"; step: UploadStep; sent: number; total: number }
  | { phase: "done"; uploaded: Uploaded }
  | { phase: "refused"; message: string };

/** The whole percent of a progress, 0 while nothing is known. */
function percentOf(sent: number, total: number): number {
  return total > 0 ? Math.min(100, Math.floor((sent * 100) / total)) : 0;
}

/** Whether an upload ended by Cancel, which says nothing more. */
function isCancel(error: unknown): boolean {
  return error instanceof DOMException && error.name === "AbortError";
}

export function AddFileDialog({ project, on, onClose, onAttached }: { project: string; on: string; onClose: () => void; onAttached: (id: string) => void }) {
  const t = useTranslate();
  const client = useClient();
  const announce = useAnnounce();
  const [acting] = useActing();
  const write = useReferenceWrite(project);
  const inputId = useId();
  const hintId = useId();
  const progressId = useId();
  const picker = useRef<HTMLInputElement>(null);
  const zone = useRef<HTMLDivElement>(null);
  // What an upload last said to a screen reader: its step and the quarter it reached.
  const spoken = useRef<{ step: UploadStep; quarter: number } | null>(null);
  const controller = useRef<AbortController | null>(null);
  // The bytes sent, kept for an upload once more (a 404 saying the file is gone).
  const blobs = useRef<{ file: File; copy: Blob | null } | null>(null);
  const [picked, setPicked] = useState<File | null>(null);
  const [upload, setUpload] = useState<Upload>({ phase: "idle" });
  const [name, setName] = useState("");
  const [title, setTitle] = useState("");
  const [matters, setMatters] = useState("");
  const [checked, setChecked] = useState(false);
  const [over, setOver] = useState(false);
  // The copy made for agents, or why none, for the file picked last.
  const [copied, setCopied] = useState<CopyOutcome | null>(null);
  // While an attach is out the file stays as it is: no pick, no drop.
  const [attaching, setAttaching] = useState(false);

  // Leaving the dialog aborts what is out.
  useEffect(
    () => () => {
      controller.current?.abort();
    },
    [],
  );

  // A file dragged over the dialog but outside its zone is refused there, and dropped there taken by
  // nothing: the browser never opens it in place of the page. The zone takes its own drop.
  useEffect(() => {
    const host = zone.current?.closest<HTMLElement>("[data-dialog-host]");
    if (host === null || host === undefined) {
      return undefined;
    }
    const hold = (event: DragEvent) => {
      if (!(event.target instanceof Node && zone.current?.contains(event.target) === true)) {
        event.preventDefault();
        if (event.dataTransfer !== null) {
          event.dataTransfer.dropEffect = "none";
        }
      }
    };
    host.addEventListener("dragover", hold);
    host.addEventListener("drop", hold);
    return () => {
      host.removeEventListener("dragover", hold);
      host.removeEventListener("drop", hold);
    };
  }, []);

  /** What an upload step is called as it is said: the copy's, or the file's by its name. */
  function stepWords(step: UploadStep, fileName: string): string {
    return step === "copy" ? t("references.file.uploadingCopy") : t("references.file.uploading", { name: shownSlot(fileName) });
  }

  /** Says a step as it starts, and each quarter it passes, never more often. */
  function speak(fileName: string, step: UploadStep, sent: number, total: number) {
    const quarter = Math.min(3, Math.floor(percentOf(sent, total) / 25));
    const last = spoken.current;
    if (last === null || last.step !== step) {
      spoken.current = { step, quarter: 0 };
      announce(stepWords(step, fileName));
      return;
    }
    if (quarter > last.quarter) {
      spoken.current = { step, quarter };
      announce(t("references.file.progressSaid", { what: stepWords(step, fileName), percent: t("references.file.percent", { percent: String(quarter * 25) }) }));
    }
  }

  /** Uploads the copy and the file, the progress shown and said step by step. */
  function send(file: File, copy: Blob | null, signal: AbortSignal): Promise<Uploaded> {
    const first: UploadStep = copy === null ? "file" : "copy";
    setUpload({ phase: "uploading", step: first, sent: 0, total: (copy ?? file).size });
    spoken.current = null;
    speak(file.name, first, 0, (copy ?? file).size);
    return uploadAll(
      client,
      project,
      file,
      copy,
      (step, sent, total) => {
        if (!signal.aborted) {
          setUpload({ phase: "uploading", step, sent, total });
          speak(file.name, step, sent, total);
        }
      },
      signal,
    );
  }

  /** A new controller for the next upload, the one before aborted. */
  function fresh(): AbortController {
    controller.current?.abort();
    const made = new AbortController();
    controller.current = made;
    return made;
  }

  async function start(file: File) {
    if (attaching) {
      return;
    }
    const mine = fresh();
    setPicked(file);
    setName(file.name);
    setCopied(null);
    blobs.current = null;
    if (file.size > FILE_MAX) {
      setUpload({ phase: "tooBig", size: file.size });
      return;
    }
    // Whether a copy is made is the first bytes' to say, never the browser's type for the file.
    const outcome = await agentCopyOf(file, mine.signal, () => {
      setUpload({ phase: "preparing" });
      announce(t("references.file.preparing"));
    });
    if (mine.signal.aborted) {
      return;
    }
    setCopied(outcome);
    const copy = "copy" in outcome ? outcome.copy.blob : null;
    blobs.current = { file, copy };
    try {
      const uploaded = await send(file, copy, mine.signal);
      setUpload({ phase: "done", uploaded });
      announce(t("references.said.uploaded", { name: shownSlot(file.name) }));
    } catch (error) {
      // Cancel and a newer pick abort it; they say nothing more.
      if (!isCancel(error)) {
        setUpload({ phase: "refused", message: apiErrorOf(error).message });
      }
    }
  }

  function cancelUpload() {
    controller.current?.abort();
    controller.current = null;
    blobs.current = null;
    setPicked(null);
    setCopied(null);
    setUpload({ phase: "idle" });
    picker.current?.focus();
  }

  const required = upload.phase === "done" ? upload.uploaded.file.needs_matters : needsMatters(guessedClass(picked?.type ?? ""));
  const running = upload.phase === "preparing" || upload.phase === "uploading";

  /**
   * One attach of `uploaded`, its reference's ID: a 400 refusing the copy (`agent_b3: …`) posts once
   * more without it, so the file lands with no copy rather than not at all.
   */
  async function post(uploaded: Uploaded): Promise<string> {
    const body = attachBody(on, uploaded, { name, title, matters, as: actingName(acting) });
    try {
      return (await write.mutateAsync({ write: "add", body })).id;
    } catch (error) {
      if (body.agent_b3 === null || !copyRefused(apiErrorOf(error))) {
        throw error;
      }
      return (await write.mutateAsync({ write: "add", body: { ...body, agent_b3: null } })).id;
    }
  }

  /** The attach of what was uploaded; a 404 naming a file gone uploads both once more and attaches again. */
  async function attachUploaded(uploaded: Uploaded): Promise<string | null | false> {
    try {
      onAttached(await post(uploaded));
      return null;
    } catch (error) {
      const answer = apiErrorOf(error);
      const kept = blobs.current;
      if (kept === null || !blobGone(answer, [uploaded.file.b3, uploaded.copy?.b3 ?? ""])) {
        return answer.message;
      }
      const mine = fresh();
      try {
        const again = await send(kept.file, kept.copy, mine.signal);
        setUpload({ phase: "done", uploaded: again });
        onAttached(await post(again));
        return null;
      } catch (failed) {
        if (isCancel(failed)) {
          return false;
        }
        setUpload({ phase: "done", uploaded });
        return apiErrorOf(failed).message;
      }
    }
  }

  async function attach(): Promise<string | null | false> {
    if (upload.phase !== "done") {
      return t("references.file.notReady");
    }
    setAttaching(true);
    try {
      return await attachUploaded(upload.uploaded);
    } finally {
      setAttaching(false);
    }
  }

  return (
    <WriteDialog
      title={t("references.file.title")}
      action={t("references.file.attach")}
      sending={t("references.file.attaching")}
      onClose={onClose}
      check={() => {
        setChecked(true);
        if (upload.phase !== "done") {
          return t("references.file.notReady");
        }
        return required && matters.trim() === "" ? t("references.matter.missing") : null;
      }}
      onSubmit={attach}
    >
      <div
        ref={zone}
        className={over ? "reference-drop reference-drop-over" : "reference-drop"}
        onDragOver={(event) => {
          event.preventDefault();
          event.dataTransfer.dropEffect = attaching ? "none" : "copy";
          setOver(!attaching);
        }}
        onDragLeave={() => {
          setOver(false);
        }}
        onDrop={(event) => {
          event.preventDefault();
          setOver(false);
          const file = event.dataTransfer.files[0];
          if (file !== undefined && !attaching) {
            void start(file);
          }
        }}
      >
        <label className="field-label" htmlFor={inputId}>
          {t("references.file.choose")}
        </label>
        <input
          ref={picker}
          id={inputId}
          className="reference-picker"
          type="file"
          accept={PICKER_ACCEPT}
          aria-describedby={hintId}
          aria-invalid={checked && upload.phase !== "done"}
          disabled={attaching}
          onChange={(event) => {
            const file = event.target.files?.[0];
            // Read, then let the same file be picked again.
            event.target.value = "";
            if (file !== undefined) {
              void start(file);
            }
          }}
        />
        <p className="reference-drop-text muted">{t("references.file.drop")}</p>
        <p id={hintId} className="card-field-hint">
          {t("references.file.types")}
        </p>
      </div>
      {upload.phase === "tooBig" && picked !== null && (
        <p className="field-error" role="alert">
          {t("references.file.tooBig", { name: shownSlot(picked.name), size: sizeWords(upload.size, t) })}
        </p>
      )}
      {running && (
        <div className="reference-progress">
          {upload.phase === "preparing" ? (
            <p id={progressId} className="reference-progress-text">
              {t("references.file.preparing")}
            </p>
          ) : (
            <>
              <p id={progressId} className="reference-progress-text">
                {upload.step === "copy" ? t("references.file.uploadingCopy") : t("references.file.uploading", { name: shownSlot(name) })}
              </p>
              <progress
                className="reference-progress-bar"
                aria-labelledby={progressId}
                max={Math.max(1, upload.total)}
                value={upload.sent}
                aria-valuetext={t("references.file.percent", { percent: String(percentOf(upload.sent, upload.total)) })}
              />
              <span className="reference-progress-percent muted" aria-hidden="true">
                {t("references.file.percent", { percent: String(percentOf(upload.sent, upload.total)) })}
              </span>
            </>
          )}
          <Button spacing="compact" onClick={cancelUpload}>
            {t("references.file.cancelUpload")}
          </Button>
        </div>
      )}
      {copied !== null && <CopyNote outcome={copied} />}
      {upload.phase === "done" && (
        // Said once through the announcer as it lands: no live region of its own.
        <div className="reference-uploaded">
          <p>{t("references.file.done", { name: shownSlot(name), size: sizeWords(upload.uploaded.file.size, t) })}</p>
          {upload.uploaded.file.warning !== null && (
            <p className="note">
              <Icon name="alert" />
              <span lang="en">{shownSlot(upload.uploaded.file.warning)}</span>
            </p>
          )}
        </div>
      )}
      {upload.phase === "refused" && (
        <div className="refusal" role="alert">
          <p className="refusal-title">
            <Icon name="alert" />
            <span>{t("references.file.refused")}</span>
          </p>
          <p className="verbatim" lang="en">
            {upload.message}
          </p>
          {picked !== null && (
            <Button
              spacing="compact"
              onClick={() => {
                void start(picked);
              }}
            >
              {t("references.file.again")}
            </Button>
          )}
        </div>
      )}
      {picked !== null && <TextField label={t("references.file.name")} value={name} onChange={setName} required autoFocus />}
      <TextField label={t("references.titleField")} value={title} onChange={setTitle} hint={t("references.titleHint")} />
      <TextArea
        label={required ? t("references.matter.required") : t("references.matter.label")}
        value={matters}
        onChange={setMatters}
        hint={t("references.matter.hint")}
        required={required}
        invalid={checked && required && matters.trim() === ""}
      />
      <ActingNote />
    </WriteDialog>
  );
}

/** What became of the copy for agents: made, its size said; past the pixel budget or not made, said so; none needed, nothing. */
function CopyNote({ outcome }: { outcome: CopyOutcome }) {
  const t = useTranslate();
  if ("copy" in outcome) {
    const { copy } = outcome;
    return <p className="muted reference-copy-note">{t("references.file.copy", { width: String(copy.width), height: String(copy.height), size: sizeWords(copy.blob.size, t) })}</p>;
  }
  switch (outcome.why) {
    case "notImage":
    case "ready":
    case "cancelled":
      return null;
    case "budget":
      return (
        <p className="note reference-copy-note">
          <Icon name="info" />
          <span>{t("references.file.noCopyBudget", { width: String(outcome.width), height: String(outcome.height) })}</span>
        </p>
      );
    case "unknown":
    case "shape":
    case "failed":
      return (
        <p className="note reference-copy-note">
          <Icon name="info" />
          <span>{t("references.file.noCopy")}</span>
        </p>
      );
  }
}
