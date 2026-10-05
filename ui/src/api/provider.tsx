import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createContext, use, useState, type ReactNode } from "react";
import type { SpecEngineClient } from "./client";

const ClientContext = createContext<SpecEngineClient | null>(null);

/** A query cache with the slice's defaults: no retry, no refetch on focus. */
export function createQueryClient(): QueryClient {
  return new QueryClient({
    defaultOptions: {
      queries: { retry: 0, refetchOnWindowFocus: false },
      mutations: { retry: 0 },
    },
  });
}

/** Gives the tree the client the bootstrap picked and a query cache of its own. */
export function ApiProvider({ client, children }: { client: SpecEngineClient; children: ReactNode }) {
  const [queryClient] = useState(createQueryClient);
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
