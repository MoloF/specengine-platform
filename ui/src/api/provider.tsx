import { focusManager, QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createContext, use, useState, type ReactNode } from "react";
import type { SpecEngineClient } from "./client";

const ClientContext = createContext<SpecEngineClient | null>(null);

/**
 * Focus as a person meets it: query-core listens to `visibilitychange` alone, which a browser beside
 * an editor never raises, so a hand edit and a click back on the page would read nothing. The
 * window's `focus` is that click. A read still asks for it, by `refetchOnWindowFocus`.
 */
function listenForFocus(onFocus: () => void): () => void {
  const listener = () => {
    onFocus();
  };
  window.addEventListener("visibilitychange", listener);
  window.addEventListener("focus", listener);
  return () => {
    window.removeEventListener("visibilitychange", listener);
    window.removeEventListener("focus", listener);
  };
}

/** A query cache with the slice's defaults: no retry, no refetch on focus but where a read asks it. */
export function createQueryClient(): QueryClient {
  focusManager.setEventListener(listenForFocus);
  return new QueryClient({
    defaultOptions: {
      queries: { retry: 0, refetchOnWindowFocus: false },
      mutations: { retry: 0 },
    },
  });
}

/** Gives the tree the client the bootstrap picked and a query cache of its own (a test may pass one). */
export function ApiProvider({
  client,
  queryClient: given,
  children,
}: {
  client: SpecEngineClient;
  queryClient?: QueryClient;
  children: ReactNode;
}) {
  const [queryClient] = useState(() => given ?? createQueryClient());
  return (
    <ClientContext value={client}>
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    </ClientContext>
  );
}

/** The client of the nearest ApiProvider. */
export function useClient(): SpecEngineClient {
  const client = use(ClientContext);
  if (client === null) {
    throw new Error("useClient needs an ApiProvider above it");
  }
  return client;
}

/** Where the data comes from: the mock until the daemon exists. */
export function useDataSource(): SpecEngineClient["dataSource"] {
  return useClient().dataSource;
}
