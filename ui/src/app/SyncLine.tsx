import { useSync } from "../api/queries";
import { useHubLogin } from "../board/acting";
import { useTranslate, type Translate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import { Icon } from "../ui/Icon";
import { shownConfirmText } from "../ui/text";

/** The count of `ahead`, the hub's commits the remote branch lacks, in the page's words. */
export function notPushedLine(ahead: number, t: Translate): string {
  return t("app.sync.notPushed", { count: ahead });
}

/**
 * A `--push` hub's push status, beside the project's view
 * (`docs/features/hub-git.md` "Status and the screen"): the commits not pushed when `ahead` > 0, and
 * the last sync's `error` beneath, the daemon's words as text, escaped as a Confirm dialog escapes
 * them (shownConfirmText), never markup.
 * Nothing otherwise: a local page, which has no login and never asks; a 404, a hub without `--push`;
 * a read that failed, a last answer no longer holding; `ahead` 0 or null with no error. No control:
 * the sync runs on the hub, never from here, and nothing here holds back a confirm or any other work.
 */
export function SyncLine({ project }: { project: string }) {
  const t = useTranslate();
  const login = useHubLogin();
  const sync = useSync(project, login !== null);
  if (login === null || sync.error !== null || sync.data === undefined) {
    return null;
  }
  const { ahead, error } = sync.data;
  const notPushed = ahead !== null && ahead > 0 ? ahead : null;
  const failed = error !== null && error !== "" ? error : null;
  if (notPushed === null && failed === null) {
    return null;
  }
  return (
    <div className="sync-line">
      {notPushed !== null && (
        <p className="sync-part">
          <Icon name="info" />
          <span>{notPushedLine(notPushed, t)}</span>
        </p>
      )}
      {failed !== null && (
        <p className="sync-part sync-error">
          <Icon name="alert" />
          <span className="agent-text" dir="auto">
            {fill(t("app.sync.failed", { error: slot("error") }), { error: <span lang="en">{shownConfirmText(failed)}</span> })}
          </span>
        </p>
      )}
    </div>
  );
}
