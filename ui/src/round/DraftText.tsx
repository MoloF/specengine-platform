import { useId, type Ref } from "react";
import { useTranslate } from "../i18n";
import { Icon } from "../ui/Icon";
import { ANSWER_MAX_BYTES, utf8Bytes } from "./text";

/**
 * The text a pick sends, typed in full, never cut: its UTF-8 bytes against the cap beside it (a
 * text over the cap is sent all the same; the daemon's refusal decides). `fieldRef` reaches the field,
 * `readOnly` holds it still while a submit is out; `required` marks it so, and `stop`, a submit stopped
 * for it, is said as an alert under it, the field invalid (a dialog's, as its other stops are).
 */
export function DraftText({
  label,
  value,
  cap,
  sent,
  missing,
  onChange,
  fieldRef,
  readOnly = false,
  required = false,
  stop = null,
}: {
  label: string;
  value: string;
  cap: number;
  /** The text as it is sent, which the counter counts. */
  sent: string;
  /** What to write when nothing is written yet; null when something is. */
  missing: string | null;
  onChange: (value: string) => void;
  fieldRef?: Ref<HTMLTextAreaElement>;
  readOnly?: boolean;
  required?: boolean;
  stop?: string | null;
}) {
  const t = useTranslate();
  const fieldId = useId();
  const counterId = useId();
  const missingId = useId();
  const stopId = useId();
  const bytes = utf8Bytes(sent);
  const over = bytes > cap;
  return (
    <div className="round-field">
      <label className="field-label" htmlFor={fieldId}>
        {label}
      </label>
      <textarea
        id={fieldId}
        ref={fieldRef}
        className="field"
        rows={3}
        dir="auto"
        spellCheck
        value={value}
        readOnly={readOnly}
        aria-required={required || undefined}
        aria-invalid={over || stop !== null}
        aria-describedby={[counterId, missing === null ? null : missingId, stop === null ? null : stopId].filter((id) => id !== null).join(" ")}
        onChange={(event) => {
          onChange(event.target.value);
        }}
      />
      <p id={counterId} className={over ? "byte-count byte-count-over" : "byte-count"}>
        {over && <Icon name="alert" />}
        <span className="byte-count-value">{t("round.bytes", { bytes, cap })}</span>
        {over && <span>{t("round.overCap")}</span>}
      </p>
      {missing !== null && (
        <p id={missingId} className="note">
          <Icon name="info" />
          <span>{missing}</span>
        </p>
      )}
      {stop !== null && (
        <p id={stopId} className="field-error" role="alert">
          {stop}
        </p>
      )}
    </div>
  );
}

/**
 * An own answer as the Inbox's Accept takes it
 * (docs/features/question-accept-note.md "Description and interactions"): "Your answer", counted as
 * sent, trimmed, against the answer's cap, as the Questions screen counts it.
 */
export function OwnAnswerText(props: { value: string; stop: string | null; onChange: (value: string) => void; fieldRef?: Ref<HTMLTextAreaElement>; readOnly?: boolean }) {
  const t = useTranslate();
  return <DraftText {...props} label={t("round.yourAnswer")} cap={ANSWER_MAX_BYTES} sent={props.value.trim()} missing={null} required />;
}
