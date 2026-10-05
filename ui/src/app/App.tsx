import type { SpecEngineClient } from "../api/client";
import { ApiProvider } from "../api/provider";
import { DataSource } from "./DataSource";
import { ErrorBoundary } from "./ErrorBoundary";
import { Shell } from "./Shell";

function RootFailure({ error, client, scenario }: { error: Error; client: SpecEngineClient; scenario: string | null }) {
  return (
    <>
      <header className="app-header">
        <p className="brand">SpecEngine</p>
        <DataSource dataSource={client.dataSource} scenario={scenario} />
      </header>
      <main className="app-main">
        <h1>SpecEngine stopped</h1>
        <div className="error-panel">
          <p className="error-message" role="alert">
            {error.message}
          </p>
          <button
            type="button"
            className="button"
            onClick={() => {
              window.location.reload();
            }}
          >
            Reload
          </button>
        </div>
      </main>
    </>
  );
}

/**
 * The whole UI over one SpecEngineClient. `scenario` names a non-default mock scenario for the
 * "Mock data" indicator, null otherwise.
 */
export function App({ client, scenario }: { client: SpecEngineClient; scenario: string | null }) {
  return (
    <ErrorBoundary fallback={(error) => <RootFailure error={error} client={client} scenario={scenario} />}>
      <ApiProvider client={client}>
        <Shell scenario={scenario} />
      </ApiProvider>
    </ErrorBoundary>
  );
}
