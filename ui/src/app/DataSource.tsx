import type { SpecEngineClient } from "../api/client";
import { useTranslate } from "../i18n";
import { Icon } from "../ui/Icon";

/** "Mock data", always shown while the mock serves, naming a non-default scenario; not dismissible. */
export function DataSource({
  dataSource,
  scenario,
}: {
  dataSource: SpecEngineClient["dataSource"];
  scenario: string | null;
}) {
  const t = useTranslate();
  if (dataSource !== "mock") {
    return null;
  }
  return (
    <p className="data-source">
      <Icon name="mock" />
      <span>{t("app.mockData")}</span>
      {scenario !== null && <span className="data-source-scenario">{t("app.scenario", { scenario })}</span>}
    </p>
  );
}
