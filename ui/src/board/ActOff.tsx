import type { KnownTeamAct } from "../api/types";
import { Icon } from "../ui/Icon";
import { ACT_OFF } from "./labels";

/** Why a control beside it stands idle: `GET .../team` omits its act (`docs/features/hub-team.md` "Description and interactions"). */
export function ActOff({ act }: { act: KnownTeamAct }) {
  return (
    <p className="note">
      <Icon name="info" />
      <span>{ACT_OFF[act]}</span>
    </p>
  );
}
