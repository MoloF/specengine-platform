import type { KnownTeamAct } from "../api/types";
import { useTranslate } from "../i18n";
import { Icon } from "../ui/Icon";
import { actOff } from "./labels";

/** Why a control beside it stands idle: `GET .../team` omits its act (`docs/features/hub-team.md` "Description and interactions"). */
export function ActOff({ act }: { act: KnownTeamAct }) {
  const t = useTranslate();
  return (
    <p className="note">
      <Icon name="info" />
      <span>{actOff(act, t)}</span>
    </p>
  );
}
