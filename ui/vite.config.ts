// Vite and Vitest in one file (ui/README.md "Gates", "Laptop rules"): tests run once and exit,
// at most two workers, on jsdom, with a setup that fails a test writing to console.error or warn.
import react from "@vitejs/plugin-react";
import type { Plugin } from "vite";
import { defineConfig } from "vitest/config";

// The dev and preview servers' `/api` proxy to the daemon, specengine-http, on its default port
// (`docs/features/daemon-read.md` "Data"). The page and its `/api` are one origin; the proxy sends
// each request on to 127.0.0.1:7777 with that Host, so the daemon's fence (Host, Origin,
// Sec-Fetch-Site: its own origin only) sees what the browser sent, less an Origin that is this
// server's own. Node's request objects are named here as the hooks touch them: the UI has no
// @types/node (ui/README.md "Dependencies"). src/api/proxy.test.ts drives these hooks.

/** The dev or preview server's request: the connection it came in on. */
interface IncomingRequest {
  socket: { localPort?: number; remoteAddress?: string };
}

/** The proxied request to the daemon, before it is sent. */
interface OutgoingRequest {
  getHeader(name: string): unknown;
  removeHeader(name: string): void;
}

/** The proxy server's `proxyReq` event (Node's EventEmitter, unresolved without Node's types). */
interface ProxyRequests {
  on(event: "proxyReq", listener: (proxyReq: OutgoingRequest, req: IncomingRequest) => void): unknown;
}

/** A Connect response as the guard writes one. */
interface PlainResponse {
  statusCode: number;
  setHeader(name: string, value: string): unknown;
  end(body: string): unknown;
}

/**
 * The origins of a page this server served, read off the request's own connection: the port it
 * came in on (dev 5173, preview 4173, or the next free one Vite took) on `127.0.0.1` and
 * `localhost`, plain http. No port known: none, and every Origin is sent on.
 */
function ownOrigins(localPort: number | undefined): string[] {
  return localPort === undefined ? [] : [`http://127.0.0.1:${localPort}`, `http://localhost:${localPort}`];
}

/**
 * A proxied request's Origin is removed only when it is this server's own: the UI itself (a
 * same-origin POST, the decision, carries one), which the daemon then answers (its 403 for a
 * decision). Any other Origin (another page or port, a LAN address, a list of two) is sent on
 * unchanged, and the daemon refuses it, 403 in its own words.
 */
function dropOwnOrigin(proxyReq: OutgoingRequest, req: IncomingRequest): void {
  const origin = proxyReq.getHeader("origin");
  if (typeof origin === "string" && ownOrigins(req.socket.localPort).includes(origin)) {
    proxyReq.removeHeader("origin");
  }
}

/** A loopback peer: 127.0.0.0/8, ::1, or 127.x mapped into IPv6. */
function isLoopback(address: string | undefined): boolean {
  return address !== undefined && /^(?:127\.|::1$|::ffff:127\.)/i.test(address);
}

/**
 * `/api` for this machine only. The server binds 127.0.0.1; started with `--host` it would hand the
 * daemon's reads to the network, and the daemon cannot tell, since every proxied request reaches it
 * from 127.0.0.1. A request from another machine is refused here, 403, before the proxy.
 */
function thisMachineOnly(req: unknown, res: unknown, next: () => void): void {
  const peer = (req as IncomingRequest).socket.remoteAddress;
  if (isLoopback(peer)) {
    next();
    return;
  }
  const refused = res as PlainResponse;
  refused.statusCode = 403;
  refused.setHeader("Content-Type", "text/plain; charset=utf-8");
  refused.end(
    `refused by the UI's dev server: /api reaches the daemon from this machine only (127.0.0.1); this request came from ${peer ?? "an unknown address"}`,
  );
}

/** The guard on the dev and the preview server: a plugin's middlewares run before Vite's own, its proxy among them. */
function apiThisMachineOnly(): Plugin {
  return {
    name: "specengine-api-this-machine-only",
    configureServer(server) {
      server.middlewares.use("/api", thisMachineOnly);
    },
    configurePreviewServer(server) {
      server.middlewares.use("/api", thisMachineOnly);
    },
  };
}

export default defineConfig({
  plugins: [react(), apiThisMachineOnly()],
  server: {
    host: "127.0.0.1",
    // Same-origin only: no CORS headers for another page, `/api` included.
    cors: false,
    proxy: {
      "/api": {
        target: "http://127.0.0.1:7777",
        changeOrigin: true,
        configure: (proxy) => {
          (proxy as unknown as ProxyRequests).on("proxyReq", dropOwnOrigin);
        },
      },
    },
  },
  // `preview` takes the proxy and the guard as they are; CORS off there too.
  preview: { host: "127.0.0.1", cors: false },
  test: {
    environment: "jsdom",
    watch: false,
    maxWorkers: 2,
    include: ["src/**/*.test.{ts,tsx}"],
    setupFiles: ["./src/test/setup.ts"],
    // CSS reaches tests only as ?raw text (the token and focus checks); without this it is empty.
    css: true,
  },
});
