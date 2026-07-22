// Connection state machine. Design rule from the Drizzle Studio critique:
// this page DIAGNOSES failures, it never shows an eternal spinner or a
// wall of maybe-causes. The SPA knows port+token from the URL fragment,
// so it can actively probe instead of passively waiting.
//
// States: connecting -> probing -> diagnosed(kind) | connected | session_ended

import { PROTOCOL_VERSION, type ClientMsg, type ServerMsg } from "./protocol";

export type Diagnosis =
  | "daemon_down" // /health also failed: daemon not running or wrong port
  | "browser_blocked" // /health ok but WS failed: shields / PNA / Safari
  | "bad_token" // daemon restarted; stale tab
  | "protocol_mismatch"; // installed daemon older than hosted SPA

export type ConnState =
  | { kind: "connecting" }
  | { kind: "probing" }
  | { kind: "diagnosed"; diagnosis: Diagnosis; detail: string }
  | { kind: "connected"; daemonVersion: string; llm: string }
  | { kind: "session_ended" };

export interface FragmentParams {
  port: number | null;
  token: string | null;
}

export function parseFragment(hash: string): FragmentParams {
  const params = new URLSearchParams(hash.replace(/^#/, ""));
  const port = params.get("port");
  return {
    port: port ? Number(port) : null,
    token: params.get("token"),
  };
}

const WS_TIMEOUT_MS = 3000;

export function connect(
  { port, token }: FragmentParams,
  onState: (s: ConnState) => void,
  onMessage: (m: ServerMsg) => void,
): { send: (m: ClientMsg) => void; close: () => void } {
  if (!port || !token) {
    onState({
      kind: "diagnosed",
      diagnosis: "daemon_down",
      detail: "Missing port/token — open the exact URL diffthing printed.",
    });
    return { send: () => {}, close: () => {} };
  }

  onState({ kind: "connecting" });
  // The daemon serves this page, so the WS and /health share its origin. Over
  // https (local.diffthing.dev) that's wss same-origin; over the plain-http
  // offline build it's ws on 127.0.0.1. Deriving from location keeps both
  // paths mixed-content-free without hardcoding a scheme.
  const secure = location.protocol === "https:";
  const wsOrigin = `${secure ? "wss" : "ws"}://${location.host}`;
  const httpOrigin = `${location.protocol}//${location.host}`;
  const ws = new WebSocket(`${wsOrigin}/ws`);
  let settled = false;

  const timeout = setTimeout(async () => {
    if (settled) return;
    settled = true;
    ws.close();
    onState({ kind: "probing" });
    // Layered probe: if /health answers but WS didn't, the browser is the
    // problem (Brave shields, Chrome PNA, Safari). If both fail, the
    // daemon is down or the port is wrong.
    try {
      const res = await fetch(`${httpOrigin}/health`, {
        signal: AbortSignal.timeout(2000),
      });
      if (res.ok) {
        onState({
          kind: "diagnosed",
          diagnosis: "browser_blocked",
          detail: "The daemon is up, but your browser blocked the local connection.",
        });
        return;
      }
    } catch {
      /* fall through */
    }
    onState({
      kind: "diagnosed",
      diagnosis: "daemon_down",
      detail: "Run `npx diffthing` in your repo, then open the printed URL.",
    });
  }, WS_TIMEOUT_MS);

  ws.onopen = () => {
    ws.send(
      JSON.stringify({
        type: "hello",
        protocol: PROTOCOL_VERSION,
        token,
      } satisfies ClientMsg),
    );
  };

  ws.onmessage = (ev) => {
    const msg = JSON.parse(ev.data) as ServerMsg;
    if (msg.type === "hello_ack") {
      settled = true;
      clearTimeout(timeout);
      onState({ kind: "connected", daemonVersion: msg.daemon_version, llm: msg.llm });
      return;
    }
    if (msg.type === "error" && msg.code === "bad_token") {
      settled = true;
      clearTimeout(timeout);
      onState({ kind: "session_ended" });
      return;
    }
    if (msg.type === "error" && msg.code === "protocol_mismatch") {
      settled = true;
      clearTimeout(timeout);
      onState({
        kind: "diagnosed",
        diagnosis: "protocol_mismatch",
        detail: msg.message,
      });
      return;
    }
    onMessage(msg);
  };

  ws.onclose = () => {
    if (!settled) return; // timeout path owns the diagnosis
  };

  return {
    send: (m: ClientMsg) => ws.send(JSON.stringify(m)),
    close: () => ws.close(),
  };
}

export type BrowserKind = "safari" | "brave" | "chrome" | "other";

export interface BrowserHelp {
  kind: BrowserKind;
  /** Card heading, e.g. "Using Safari?". */
  label: string;
  /** Ordered, plain-text remediation steps. Rendered as a list. */
  steps: string[];
}

/** Best-effort browser identification for the connection-help card. */
export function detectBrowser(): BrowserKind {
  // Brave hides itself in the UA; feature-detect via navigator.brave.
  if ("brave" in navigator) return "brave";
  const ua = navigator.userAgent;
  if (ua.includes("Safari") && !ua.includes("Chrome")) return "safari";
  if (ua.includes("Chrome") || ua.includes("Chromium")) return "chrome";
  return "other";
}

/**
 * Per-browser steps for the "your browser blocked localhost" case, mirroring
 * the Drizzle Studio onboarding. `--offline` is the universal escape hatch.
 */
export function browserHelp(): BrowserHelp {
  switch (detectBrowser()) {
    case "brave":
      return {
        kind: "brave",
        label: "Using Brave?",
        steps: [
          "Click the Brave shield icon in the address bar.",
          "Turn Shields down for this site, then reload.",
          "Still blocked? Run `npx diffthing --offline`.",
        ],
      };
    case "safari":
      return {
        kind: "safari",
        label: "Using Safari?",
        steps: [
          "Safari blocks self-signed localhost certs.",
          "Run `npx diffthing --offline` and open the printed 127.0.0.1 URL.",
          "Or trust the daemon cert (see certs/README) and reload.",
        ],
      };
    case "chrome":
      return {
        kind: "chrome",
        label: "Using Chrome?",
        steps: [
          "Open Site information in the address bar.",
          'Enable "Local network access", then reload.',
          "Still blocked? Run `npx diffthing --offline`.",
        ],
      };
    default:
      return {
        kind: "other",
        label: "Browser blocked the connection",
        steps: [
          "Allow local network access for this site, then reload.",
          "Or run `npx diffthing --offline` for the plain-HTTP path.",
        ],
      };
  }
}
