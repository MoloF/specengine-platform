import type { SpecEngineClient } from "../api/client";
import { Icon } from "../ui/Icon";

/** "Mock data", always shown while the mock serves, naming a non-default scenario; not dismissible. */
export function DataSource({
  dataSource,
  scenario,
}: {
  dataSource: SpecEngineClient["dataSource"];
  scenario: string | null;
}) {
  if (dataSource !== "mock") {
    return null;
  }
  return (
    <p className="data-source">
      <Icon name="mock" />
      <span>Mock data</span>
      {scenario !== null && <span className="data-source-scenario">scenario: {scenario}</span>}
    </p>
  );
}
