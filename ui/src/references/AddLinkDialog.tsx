import { useState } from "react";
import { apiErrorOf } from "../api/client";
import { useReferenceWrite } from "../api/referenceQueries";
import { actingName, useActing } from "../board/acting";
import { ActingNote, TextArea, TextField, WriteDialog } from "../board/CardDialogs";
import { useTranslate } from "../i18n";

// Add a link… (docs/features/work-references.md "UI"): its address, required, a title and "what
// matters", both optional for a link; one POST under the acting name, the daemon judging the address
// (http or https, a host, no whitespace or control). A refusal keeps the dialog and the typed text with
// the daemon's words; nothing fetches the link, here or on the daemon.

export function AddLinkDialog({ project, on, onClose, onAdded }: { project: string; on: string; onClose: () => void; onAdded: (id: string) => void }) {
  const t = useTranslate();
  const [acting] = useActing();
  const write = useReferenceWrite(project);
  const [url, setUrl] = useState("");
  const [title, setTitle] = useState("");
  const [matters, setMatters] = useState("");
  const [checked, setChecked] = useState(false);
  return (
    <WriteDialog
      title={t("references.url.title")}
      action={t("references.url.add")}
      sending={t("references.url.adding")}
      onClose={onClose}
      check={() => {
        setChecked(true);
        return url.trim() === "" ? t("references.url.missing") : null;
      }}
      onSubmit={async () => {
        try {
          const made = await write.mutateAsync({
            write: "add",
            body: { on, url: url.trim(), title: title.trim() === "" ? null : title.trim(), matters: matters.trim() === "" ? null : matters.trim(), as: actingName(acting) },
          });
          onAdded(made.id);
          return null;
        } catch (error) {
          return apiErrorOf(error).message;
        }
      }}
    >
      <TextField label={t("references.url.field")} value={url} onChange={setUrl} hint={t("references.url.hint")} required autoFocus invalid={checked && url.trim() === ""} />
      <TextField label={t("references.titleField")} value={title} onChange={setTitle} />
      <TextArea label={t("references.matter.label")} value={matters} onChange={setMatters} />
      <ActingNote />
    </WriteDialog>
  );
}
