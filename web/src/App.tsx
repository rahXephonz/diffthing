import { useEffect, useMemo, useRef } from "react";
import { connect, parseFragment } from "./libs/connection";
import type { ClientMsg } from "./libs/protocol";
import { useStore } from "./libs/store";

const IMPACT_CLASS: Record<string, string> = {
  highest: "border-highest text-highest",
  high: "border-high text-high",
  medium: "border-medium text-medium",
  low: "text-low",
};

const badge = "text-[11px] px-2 py-0.5 rounded-full border border-border text-muted";
const chromeButton =
  "bg-transparent border border-border text-text rounded-md px-2.5 py-1 cursor-pointer hover:border-accent disabled:opacity-40 disabled:cursor-default disabled:hover:border-border";

export default function App() {
  const sendRef = useRef<(m: ClientMsg) => void>(() => null);
  const { conn, walkthrough, files, scores, review, pending, selectedStep } = useStore();
  const { setConn, onServerMsg, selectStep } = useStore();

  useEffect(() => {
    const { send, close } = connect(parseFragment(location.hash), setConn, onServerMsg);
    sendRef.current = send;
    return close;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const hunksByStep = useMemo(() => {
    const all = new Map(files.flatMap((f) => f.hunks.map((h) => [h.id, h] as const)));
    return { all };
  }, [files]);

  if (conn.kind === "connecting" || conn.kind === "probing") {
    return (
      <Centered>
        <h1>diffthing</h1>
        <p className="text-muted">
          {conn.kind === "connecting"
            ? "Connecting to your local daemon…"
            : "Connection failed — diagnosing…"}
        </p>
      </Centered>
    );
  }

  if (conn.kind === "diagnosed") {
    return (
      <Centered>
        <h1>Can’t reach the daemon</h1>
        <p>{conn.detail}</p>
        <p className="text-muted">
          Escape hatch: <code>npx diffthing --offline</code> serves this UI directly from 127.0.0.1
          — no hosted page, no browser gymnastics.
        </p>
      </Centered>
    );
  }

  if (conn.kind === "session_ended") {
    return (
      <Centered>
        <h1>Session ended</h1>
        <p>
          The daemon restarted, so this tab’s token is stale. Rerun <code>npx diffthing</code> and
          open the new URL it prints.
        </p>
      </Centered>
    );
  }

  // connected
  const step =
    walkthrough?.scopes.flatMap((s) => s.steps).find((s) => s.id === selectedStep) ?? null;

  return (
    <div className="grid grid-cols-[320px_1fr] min-h-screen">
      <aside className="bg-panel border-r border-border p-4 flex flex-col gap-3 sticky top-0 h-screen overflow-y-auto">
        <header className="flex items-center gap-2 flex-wrap">
          <strong>diffthing</strong>
          {walkthrough?.degraded && (
            <span
              className={`${badge} border-warn text-warn`}
              title="LLM unavailable or failed validation — showing deterministic file-order walkthrough"
            >
              structure unavailable
            </span>
          )}
        </header>

        {pending && (
          <button
            className="bg-[#1d2735] border border-accent text-text rounded-md p-2.5 cursor-pointer text-left"
            onClick={() =>
              sendRef.current({
                type: "apply_update",
                to_revision: pending.revision,
              })
            }
          >
            Changes detected — {pending.report.changed.length} modified,{" "}
            {pending.report.added.length} new, {pending.report.removed.length} removed. Apply
          </button>
        )}

        {walkthrough?.scopes.map((scope) => (
          <section key={scope.id}>
            <h2 className="text-xs uppercase tracking-wider text-muted mt-3 mb-1">{scope.title}</h2>
            {scope.steps.map((s) => (
              <button
                key={s.id}
                className={`flex flex-col w-full text-left bg-transparent border rounded-md text-text px-2.5 py-2 cursor-pointer hover:border-border ${
                  s.id === selectedStep ? "border-accent" : "border-transparent"
                }`}
                onClick={() => selectStep(s.id)}
              >
                <span>{s.title}</span>
                <span className="text-muted">{s.framing}</span>
              </button>
            ))}
          </section>
        ))}

        <footer>
          <button
            className={chromeButton}
            onClick={() => sendRef.current({ type: "export_review" })}
            disabled={!review || review.flags.filter((f) => f.open).length === 0}
          >
            Export review ({review?.flags.filter((f) => f.open).length ?? 0} open flags)
          </button>
        </footer>
      </aside>

      <main className="p-6 flex flex-col gap-4">
        {!step && <p className="text-muted">Select a step to start reading.</p>}
        {step?.hunks.map((id) => {
          const h = hunksByStep.all.get(id);
          if (!h) return null;
          const score = scores[id];
          const status = review?.status[id] ?? "unviewed";
          return (
            <article
              key={id}
              className={`border rounded-lg overflow-hidden ${
                status === "changed_since_viewed" ? "border-warn" : "border-border"
              }`}
            >
              <header className="flex items-center gap-2 flex-wrap px-3 py-2 bg-panel border-b border-border">
                <code>{h.path}</code>
                {score && (
                  <span
                    className={`${badge} ${IMPACT_CLASS[score.impact] ?? ""}`}
                    title={score.reasons.join(", ")}
                  >
                    {score.impact} — {score.reasons[0]}
                  </span>
                )}
                {status === "changed_since_viewed" && (
                  <span className={`${badge} border-warn text-warn`}>changed since viewed</span>
                )}
                <button
                  className={chromeButton}
                  onClick={() => sendRef.current({ type: "mark_viewed", hunk: id })}
                >
                  {status === "viewed" ? "viewed ✓" : "mark viewed"}
                </button>
                <button
                  className={chromeButton}
                  onClick={() => {
                    const comment = prompt("Flag comment:");
                    if (comment) sendRef.current({ type: "add_flag", hunk: id, comment });
                  }}
                >
                  flag
                </button>
              </header>
              <pre className="m-0 py-2 overflow-x-auto text-[13px]">
                {h.lines.map((l, i) => (
                  <div
                    key={i}
                    className={`px-3 whitespace-pre ${
                      l.startsWith("+") ? "bg-add" : l.startsWith("-") ? "bg-del" : ""
                    }`}
                  >
                    {l}
                  </div>
                ))}
              </pre>
            </article>
          );
        })}
      </main>
    </div>
  );
}

function Centered({ children }: { children: React.ReactNode }) {
  return (
    <div className="min-h-screen grid place-content-center text-center gap-2 p-6">{children}</div>
  );
}
