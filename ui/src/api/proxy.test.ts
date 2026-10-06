import type { MinimalPluginContextWithoutEnvironment, Plugin, PreviewServer, ProxyOptions, ViteDevServer } from "vite";
import { describe, expect, it } from "vitest";
import config from "../../vite.config";

// The dev and preview servers' `/api` proxy (vite.config.ts; `docs/features/daemon-read.md` "Data"),
// driven through the config itself: the proxy's `proxyReq` hook, the plugin's guard, CORS off. The
// daemon's fence takes its own origin only; the proxy must hand it the UI's requests and nothing
// else: an Origin is dropped only when it is the dev server's own, any other is sent on for the
// daemon to refuse; another machine is refused before the proxy.

/** A proxied request's headers as the hook sees them. */
class OutgoingStub {
  readonly headers = new Map<string, string | string[]>();

  constructor(origin: string | string[] | undefined) {
    if (origin !== undefined) {
      this.headers.set("origin", origin);
    }
  }

  getHeader(name: string): unknown {
    return this.headers.get(name);
  }

  removeHeader(name: string): void {
    this.headers.delete(name);
  }
}

type ProxyHook = (proxyReq: OutgoingStub, req: { socket: { localPort?: number } }) => void;

/** The `/api` proxy's options, as the config declares them. */
function apiProxy(): ProxyOptions {
  const options = config.server?.proxy?.["/api"];
  if (options === undefined || typeof options === "string") {
    throw new Error("vite.config.ts proxies no /api");
  }
  return options;
}

/** The listeners `configure` registers, by event name. */
function proxyListeners(): Map<string, ProxyHook[]> {
  const listeners = new Map<string, ProxyHook[]>();
  const fakeProxy = {
    on(event: string, listener: ProxyHook) {
      listeners.set(event, [...(listeners.get(event) ?? []), listener]);
      return fakeProxy;
    },
  };
  const options = apiProxy();
  type ProxyServer = Parameters<NonNullable<ProxyOptions["configure"]>>[0];
  options.configure?.(fakeProxy as unknown as ProxyServer, options);
  return listeners;
}

/** The Origin the daemon receives for a request that came in on `localPort` carrying `origin`. */
function forwarded(origin: string | string[] | undefined, localPort: number | undefined): unknown {
  const proxyReq = new OutgoingStub(origin);
  for (const listener of proxyListeners().get("proxyReq") ?? []) {
    listener(proxyReq, { socket: { localPort } });
  }
  return proxyReq.getHeader("origin");
}

function isPlugin(value: unknown): value is Plugin {
  return typeof value === "object" && value !== null && !Array.isArray(value) && "name" in value;
}

/** Every plugin of the config, nested arrays flattened. */
function plugins(): Plugin[] {
  const out: Plugin[] = [];
  const walk = (value: unknown) => {
    if (Array.isArray(value)) {
      for (const item of value) {
        walk(item);
      }
    } else if (isPlugin(value)) {
      out.push(value);
    }
  };
  walk(config.plugins);
  return out;
}

type Guard = (req: unknown, res: unknown, next: () => void) => void;

/** What the guard plugin mounts on a server's middlewares: [route, handler] pairs. */
function mounted(which: "configureServer" | "configurePreviewServer"): [string, Guard][] {
  const uses: [string, Guard][] = [];
  const server = {
    middlewares: {
      use(route: string, handler: Guard) {
        uses.push([route, handler]);
      },
    },
  };
  const context = {} as MinimalPluginContextWithoutEnvironment;
  for (const plugin of plugins()) {
    if (which === "configureServer") {
      const hook = plugin.configureServer;
      const run = typeof hook === "function" ? hook : hook?.handler;
      void run?.call(context, server as unknown as ViteDevServer);
    } else {
      const hook = plugin.configurePreviewServer;
      const run = typeof hook === "function" ? hook : hook?.handler;
      void run?.call(context, server as unknown as PreviewServer);
    }
  }
  return uses.filter(([route]) => route === "/api");
}

/** The guard's answer to a request from `peer`: passed on, or the status and body written. */
function guarded(which: "configureServer" | "configurePreviewServer", peer: string | undefined) {
  const guards = mounted(which);
  expect(guards).toHaveLength(1);
  const [, guard] = guards[0] ?? ["", () => undefined];
  let passed = false;
  const written = { statusCode: 200, headers: new Map<string, string>(), body: null as string | null };
  const res = {
    set statusCode(code: number) {
      written.statusCode = code;
    },
    get statusCode() {
      return written.statusCode;
    },
    setHeader(name: string, value: string) {
      written.headers.set(name.toLowerCase(), value);
    },
    end(body: string) {
      written.body = body;
    },
  };
  guard({ socket: { remoteAddress: peer } }, res, () => {
    passed = true;
  });
  return { passed, ...written };
}

describe("the /api proxy's Origin (R-m1)", () => {
  it("proxies /api to the daemon's default port as 127.0.0.1:7777", () => {
    expect(apiProxy().target).toBe("http://127.0.0.1:7777");
    expect(apiProxy().changeOrigin).toBe(true);
  });

  it.each([
    ["the dev server's own, by IP", "http://127.0.0.1:5173", 5173],
    ["the dev server's own, by localhost", "http://localhost:5173", 5173],
    ["the preview server's own", "http://127.0.0.1:4173", 4173],
    ["the port Vite took when 5173 was busy", "http://localhost:5174", 5174],
  ])("drops %s, so the daemon answers the UI", (_name, origin, port) => {
    expect(forwarded(origin, port)).toBeUndefined();
  });

  it.each([
    ["another page on this machine", "http://localhost:3000"],
    ["another port of the same host", "http://127.0.0.1:5174"],
    ["the daemon's own origin", "http://127.0.0.1:7777"],
    ["https on the dev server's port", "https://127.0.0.1:5173"],
    ["a LAN address on the dev server's port", "http://192.168.1.7:5173"],
    ["a rebinding name on the dev server's port", "http://evil.example:5173"],
    ["an opaque origin", "null"],
    ["a trailing slash", "http://127.0.0.1:5173/"],
    ["two origins joined", "http://127.0.0.1:5173, http://evil.example"],
  ])("sends %s on unchanged, for the daemon to refuse", (_name, origin) => {
    expect(forwarded(origin, 5173)).toBe(origin);
  });

  it("sends an Origin on when the port it came in on is unknown, and repeated ones as they are", () => {
    expect(forwarded("http://127.0.0.1:5173", undefined)).toBe("http://127.0.0.1:5173");
    expect(forwarded(["http://127.0.0.1:5173", "http://127.0.0.1:5173"], 5173)).toEqual(["http://127.0.0.1:5173", "http://127.0.0.1:5173"]);
  });

  it("adds no Origin to a request without one (a same-origin GET)", () => {
    expect(forwarded(undefined, 5173)).toBeUndefined();
  });
});

describe("the dev and preview servers (R-m1)", () => {
  it("send no CORS headers: the UI is same-origin", () => {
    expect(config.server?.cors).toBe(false);
    expect(config.preview?.cors).toBe(false);
  });

  it("bind 127.0.0.1", () => {
    expect(config.server?.host).toBe("127.0.0.1");
    expect(config.preview?.host).toBe("127.0.0.1");
  });

  it.each(["configureServer", "configurePreviewServer"] as const)("%s: pass /api on from this machine", (which) => {
    for (const peer of ["127.0.0.1", "127.0.0.2", "::1", "::ffff:127.0.0.1"]) {
      expect(guarded(which, peer)).toMatchObject({ passed: true, statusCode: 200, body: null });
    }
  });

  it.each(["configureServer", "configurePreviewServer"] as const)("%s: refuse /api from another machine, 403, before the proxy", (which) => {
    for (const peer of ["192.168.1.7", "::ffff:192.168.1.7", "10.0.0.127", "fe80::1", undefined]) {
      const answer = guarded(which, peer);
      expect(answer.passed).toBe(false);
      expect(answer.statusCode).toBe(403);
      expect(answer.headers.get("content-type")).toBe("text/plain; charset=utf-8");
      expect(answer.body).toBe(
        `refused by the UI's dev server: /api reaches the daemon from this machine only (127.0.0.1); this request came from ${peer ?? "an unknown address"}`,
      );
    }
  });
});
