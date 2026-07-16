import { useEffect, useMemo, useRef } from "react";
import { connect, parseFragment } from "./connection";
import type { ClientMsg } from "./protocol";
import { useStore } from "./store";

export default function App() {
  const sendRef = useRef<(m: ClientMsg) => void>(() => {});
  const { conn, walkthrough, files, scores, review, pending, selectedStep } =
    useStore();
  const { setConn, onServerMsg, selectStep } = useStore();

  useEffect(() => {
    const { send, close } = connect(
      parseFragment(location.hash),
      setConn,
      onServerMsg,
    );
    sendRef.current = send;
    return close;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const hunksByStep = useMemo(() => {
    const all = new Map(
      files.flatMap((f) => f.hunks.map((h) => [h.id, h] as const)),
    );
    return { all };
  }, [files]);

  if (conn.kind === "connecting" || conn.kind === "probing") {
    return (
      <Centered>
        <h1>diffthing</h1>
        <p className="muted">
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
        <p className="muted">
          Escape hatch: <code>npx diffthing --offline</code> serves this UI
          directly from 127.0.0.1 — no hosted page, no browser gymnastics.
        </p>
      </Centered>
    );
  }

  if (conn.kind === "session_ended") {
    return (
      <Centered>
        <h1>Session ended</h1>
        <p>
          The daemon restarted, so this tab’s token is stale. Rerun{" "}
          <code>npx diffthing</code> and open the new URL it prints.
        </p>
      </Centered>
    );
  }

  // connected
  const step =
    walkthrough?.scopes
      .flatMap((s) => s.steps)
      .find((s) => s.id === selectedStep) ?? null;

  return (
    <div className="layout">
      <aside className="sidebar">
        <header>
          <strong>diffthing</strong>
          {walkthrough?.degraded && (
            <span className="badge warn" title="LLM unavailable or failed validation — showing deterministic file-order walkthrough">
              structure unavailable
            </span>
          )}
        </header>

        {pending && (
          <button
            className="banner"
            onClick={() =>
              sendRef.current({
                type: "apply_update",
                to_revision: pending.revision,
              })
            }
          >
            Changes detected — {pending.report.changed.length} modified,{" "}
            {pending.report.added.length} new,{" "}
            {pending.report.removed.length} removed. Apply
          </button>
        )}

        {walkthrough?.scopes.map((scope) => (
          <section key={scope.id}>
            <h2>{scope.title}</h2>
            {scope.steps.map((s) => (
              <button
                key={s.id}
                className={`step ${s.id === selectedStep ? "active" : ""}`}
                onClick={() => selectStep(s.id)}
              >
                <span>{s.title}</span>
                <span className="muted">{s.framing}</span>
              </button>
            ))}
          </section>
        ))}

        <footer>
          <button
            onClick={() => sendRef.current({ type: "export_review" })}
            disabled={!review || review.flags.filter((f) => f.open).length === 0}
          >
            Export review ({review?.flags.filter((f) => f.open).length ?? 0}{" "}
            open flags)
          </button>
        </footer>
      </aside>

      <main className="diff">
        {!step && <p className="muted">Select a step to start reading.</p>}
        {step?.hunks.map((id) => {
          const h = hunksByStep.all.get(id);
          if (!h) return null;
          const score = scores[id];
          const status = review?.status[id] ?? "unviewed";
          return (
            <article key={id} className={`hunk ${status}`}>
              <header>
                <code>{h.path}</code>
                {score && (
                  <span
                    className={`badge impact-${score.impact}`}
                    title={score.reasons.join(", ")}
                  >
                    {score.impact} — {score.reasons[0]}
                  </span>
                )}
                {status === "changed_since_viewed" && (
                  <span className="badge warn">changed since viewed</span>
                )}
                <button
                  onClick={() => sendRef.current({ type: "mark_viewed", hunk: id })}
                >
                  {status === "viewed" ? "viewed ✓" : "mark viewed"}
                </button>
                <button
                  onClick={() => {
                    const comment = prompt("Flag comment:");
                    if (comment)
                      sendRef.current({ type: "add_flag", hunk: id, comment });
                  }}
                >
                  flag
                </button>
              </header>
              <pre>
                {h.lines.map((l, i) => (
                  <div
                    key={i}
                    className={
                      l.startsWith("+") ? "add" : l.startsWith("-") ? "del" : ""
                    }
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
  return <div className="centered">{children}</div>;
}
