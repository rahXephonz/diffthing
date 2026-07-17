import clsx from "clsx";
import { useEffect, useMemo, useRef, useState } from "react";
import DiffPane, { type ViewMode } from "./components/DiffPane";
import { connect, parseFragment } from "./libs/connection";
import { iconUrlForPath, preloadIconForPath, STATUS_CLASS, STATUS_LETTER } from "./libs/fileIcon";
import type { ClientMsg, Step } from "./libs/protocol";
import { useStore } from "./libs/store";

const badge = "text-[11px] px-2 py-0.5 rounded-full border border-border text-muted";
const chromeButton =
  "bg-transparent border border-border text-text rounded-md px-2.5 py-1 cursor-pointer hover:border-accent disabled:opacity-40 disabled:cursor-default disabled:hover:border-border";

const FRAMING_COUNTS = /^\+(\d+) -(\d+) (.*)$/;

/** Colorizes the fallback walkthrough's "+N -M in path" framing. LLM-
 *  authored framing is free-form narrative text, not guaranteed to match —
 *  falls back to plain text when it doesn't. */
function FramingText({ text }: { text: string }) {
  const match = FRAMING_COUNTS.exec(text);
  if (!match) return <span className="text-muted">{text}</span>;
  const [, added, removed, rest] = match;
  return (
    <span className="text-muted">
      <span className="text-green-400">+{added}</span>{" "}
      <span className="text-highest">-{removed}</span> {rest}
    </span>
  );
}

export default function App() {
  const sendRef = useRef<(m: ClientMsg) => void>(() => null);
  const { conn, walkthrough, files, scores, review, pending, selectedStep } = useStore();
  const { setConn, onServerMsg, selectStep } = useStore();
  const [viewMode, setViewMode] = useState<ViewMode>("unified");

  useEffect(() => {
    const { send, close } = connect(parseFragment(location.hash), setConn, onServerMsg);
    sendRef.current = send;
    return close;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Always auto-apply background updates — no manual Apply banner.
  useEffect(() => {
    if (pending) {
      sendRef.current({ type: "apply_update", to_revision: pending.revision });
    }
  }, [pending]);

  const hunksById = useMemo(() => {
    return new Map(files.flatMap((f) => f.hunks.map((h) => [h.id, h] as const)));
  }, [files]);
  const fileByPath = useMemo(() => new Map(files.map((f) => [f.path, f] as const)), [files]);

  // A step's "primary file" for the sidebar icon/badge — the file its
  // first hunk belongs to. Steps are usually single-file (narrative
  // grouping is by concern, which tends to be per-file); multi-file steps
  // just show the lead file, same convention as a commit's diffstat icon.
  const primaryFileFor = (s: Step) => {
    const hunk = hunksById.get(s.hunks[0]);
    return hunk ? (fileByPath.get(hunk.path) ?? null) : null;
  };

  // Icons load lazily from node_modules (see fileIcon.ts) — preload the
  // sidebar's icons once the walkthrough is known, then bump iconsVersion
  // to force the re-render that picks up the now-resolved URLs.
  const [iconsVersion, setIconsVersion] = useState(0);
  useEffect(() => {
    if (!walkthrough) return;
    const paths = new Set<string>();
    for (const scope of walkthrough.scopes) {
      for (const s of scope.steps) {
        const hunk = hunksById.get(s.hunks[0]);
        const file = hunk ? fileByPath.get(hunk.path) : undefined;
        if (file) paths.add(file.path);
      }
    }
    if (paths.size === 0) return;
    Promise.all([...paths].map((p) => preloadIconForPath(p))).then(() => {
      setIconsVersion((v) => v + 1);
    });
  }, [walkthrough, hunksById, fileByPath]);

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
  const stepHunks = (step?.hunks ?? []).map((id) => hunksById.get(id)).filter((h) => h != null);

  return (
    <div className="grid grid-cols-[320px_1fr] h-screen">
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

        {walkthrough?.scopes.map((scope) => (
          <section key={scope.id}>
            <h2 className="text-xs uppercase tracking-wider text-muted mt-3 mb-1">{scope.title}</h2>
            {scope.steps.map((s) => {
              const file = primaryFileFor(s);
              void iconsVersion; // re-run once preloaded icon URLs resolve
              const iconUrl = file ? iconUrlForPath(file.path) : undefined;
              return (
                <button
                  key={s.id}
                  className={clsx(
                    "flex flex-col w-full text-left bg-transparent border rounded-md text-text px-2.5 py-2 cursor-pointer hover:border-border",
                    s.id === selectedStep ? "border-accent" : "border-transparent",
                  )}
                  onClick={() => selectStep(s.id)}
                >
                  <span className="flex items-center gap-1.5">
                    {iconUrl && <img src={iconUrl} alt="" className="w-4 h-4 shrink-0" />}
                    {file && (
                      <span
                        className={clsx(
                          "text-[10px] font-bold w-3 shrink-0 text-center",
                          STATUS_CLASS[file.status],
                        )}
                        title={file.status}
                      >
                        {STATUS_LETTER[file.status]}
                      </span>
                    )}
                    <span className="truncate">{s.title}</span>
                  </span>
                  <FramingText text={s.framing} />
                </button>
              );
            })}
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

      <main className="h-screen flex flex-col overflow-hidden">
        {!step && (
          <div className="p-6">
            <p className="text-muted">Select a step to start reading.</p>
          </div>
        )}
        {step && (
          <>
            <div className="flex justify-end gap-1 px-4 py-2 border-b border-border">
              {(["unified", "split"] as const).map((mode) => (
                <button
                  key={mode}
                  className={clsx(
                    "text-xs px-2 py-1 rounded-md border cursor-pointer",
                    viewMode === mode
                      ? "border-accent text-accent"
                      : "border-border text-muted hover:border-accent",
                  )}
                  onClick={() => setViewMode(mode)}
                >
                  {mode}
                </button>
              ))}
            </div>
            <div className="flex-1 min-h-0">
              <DiffPane
                hunks={stepHunks}
                scores={scores}
                statusOf={(id) => review?.status[id] ?? "unviewed"}
                onMarkViewed={(id) => sendRef.current({ type: "mark_viewed", hunk: id })}
                onFlag={(id, comment) => sendRef.current({ type: "add_flag", hunk: id, comment })}
                viewMode={viewMode}
              />
            </div>
          </>
        )}
      </main>
    </div>
  );
}

function Centered({ children }: { children: React.ReactNode }) {
  return (
    <div className="min-h-screen grid place-content-center text-center gap-2 p-6">{children}</div>
  );
}
