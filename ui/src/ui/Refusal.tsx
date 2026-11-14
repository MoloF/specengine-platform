import { doorRefusalOf } from "../api/door";
import type { ApiError } from "../api/types";
import { useTranslate } from "../i18n";
import { Icon } from "./Icon";

/**
 * A refusal on a step-up surface, the confirm and the card writes of its class
 * (docs/features/door-step-up.md "Description and interactions"), as one `role="alert"` block. The
 * door's 401: the step-up notice and, when the body gave a usable address, the door's login page in a
 * new tab, built from `location.href` as the refusal shows; no byte of the body is shown. Status 0:
 * `heading` and the words as before, then the line that a second factor may be what is missing, no
 * link. Anything else: `heading` and the daemon's words verbatim.
 */
export function Refusal({ error, heading }: { error: ApiError; heading: string }) {
  const t = useTranslate();
  const door = doorRefusalOf(error, window.location.href);
  // The door's own variant is the one with a link field, set or null.
  const atDoor = door !== null && "link" in door;
  return (
    <div className="refusal" role="alert">
      <p className="refusal-title">
        <Icon name="alert" />
        <span>{atDoor ? t("ui.stepUp.title") : heading}</span>
      </p>
      {atDoor ? (
        <p>{t("ui.stepUp.text")}</p>
      ) : (
        <p className="verbatim" lang="en">
          {error.message}
        </p>
      )}
      {atDoor && door.link !== null && (
        <p>
          <a href={door.link} target="_blank" rel="noopener noreferrer">
            {t("ui.stepUp.open")}
          </a>
        </p>
      )}
      {door !== null && !atDoor && <p>{t("ui.stepUp.noAnswer")}</p>}
    </div>
  );
}
