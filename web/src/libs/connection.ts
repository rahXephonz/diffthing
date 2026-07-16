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
  | { kind: "connected"; daemonVersion: string }
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
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
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
      const res = await fetch(`http://127.0.0.1:${port}/health`, {
        signal: AbortSignal.timeout(2000),
      });
      if (res.ok) {
        onState({
          kind: "diagnosed",
          diagnosis: "browser_blocked",
          detail: browserFix(),
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
      onState({ kind: "connected", daemonVersion: msg.daemon_version });
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

function browserFix(): string {
  const ua = navigator.userAgent;
  // Brave hides itself in the UA; feature-detect via navigator.brave.
  const isBrave = "brave" in navigator;
  if (isBrave)
    return "Brave is blocking localhost. Click the Brave shield icon and turn Shields off for this site — or run `npx diffthing --offline`.";
  if (ua.includes("Safari") && !ua.includes("Chrome"))
    return "Safari blocks localhost from HTTPS pages. Run `npx diffthing --offline` and open the printed 127.0.0.1 URL instead.";
  return "Your browser blocked local network access. Open Site information in the URL bar and enable Local network access — or run `npx diffthing --offline`.";
}
