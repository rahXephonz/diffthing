import clsx from "clsx";
import { useEffect, useMemo, useRef, useState } from "react";
import DiffPane, { type ViewMode } from "./components/DiffPane";
import { connect, parseFragment } from "./libs/connection";
import { iconUrlForPath, preloadIconForPath, STATUS_CLASS, STATUS_LETTER } from "./libs/fileIcon";
import type { ClientMsg, Step } from "./libs/protocol";
import { useStore } from "./libs/store";
import { basename, fileTotals } from "./libs/utils";

const badge = "text-[11px] px-2 py-0.5 rounded-full border border-border text-muted";
const chromeButton =
  "bg-transparent border border-border text-text rounded-md px-2.5 py-1 cursor-pointer hover:border-accent disabled:opacity-40 disabled:cursor-default disabled:hover:border-border";

function Counts({ added, removed }: { added: number; removed: number }) {
  return (
    <span className="tabular-nums whitespace-nowrap">
      <span className="text-green-400">+{added}</span>{" "}
      <span className="text-highest">-{removed}</span>
    </span>
  );
}

function StepDot({ done }: { done: boolean }) {
  return done ? (
    <span
      className="w-4 h-4 shrink-0 rounded-full bg-green text-panel text-[10px] font-bold grid place-content-center"
      title="all hunks viewed"
    >
      ✓
    </span>
  ) : (
    <span className="w-4 h-4 shrink-0 rounded-full border border-border" title="unread" />
  );
}

export default function App() {
  const sendRef = useRef<(m: ClientMsg) => void>(() => null);
  const { conn, walkthrough, files, scores, review, pending, selectedStep, progress, dispatch } =
    useStore();
  const { setConn, onServerMsg, selectStep } = useStore();
  const [viewMode, setViewMode] = useState<ViewMode>("unified");
  const [filter, setFilter] = useState("");

  useEffect(() => {
    const { send, close } = connect(parseFragment(location.hash), setConn, onServerMsg);
    sendRef.current = send;
    return close;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

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
      <Landing
        tone="wait"
        status={conn.kind === "connecting" ? "Connecting…" : "Diagnosing…"}
      >
        <p>
          Reviewing changes on your machine. If this hangs, make sure the daemon is
          still running in your project.
        </p>
      </Landing>
    );
  }

  if (conn.kind === "diagnosed") {
    return (
      <Landing tone="error" status="Not connected">
        <p>{conn.detail}</p>
        <p className="text-subtle">
          Or run <Kbd>npx diffthing --offline</Kbd> to serve this UI directly from
          127.0.0.1.
        </p>
      </Landing>
    );
  }

  if (conn.kind === "session_ended") {
    return (
      <Landing tone="ended" status="Session ended">
        <p>
          The daemon restarted, so this tab’s token is stale. Rerun the command below
          and open the fresh URL it prints.
        </p>
      </Landing>
    );
  }

  const scopeOfStep =
    walkthrough?.scopes.find((sc) => sc.steps.some((s) => s.id === selectedStep)) ?? null;
  const step = scopeOfStep?.steps.find((s) => s.id === selectedStep) ?? null;
  const stepHunks = (step?.hunks ?? []).map((id) => hunksById.get(id)).filter((h) => h != null);

  const stepNumber = new Map<string, number>();
  walkthrough?.scopes.flatMap((s) => s.steps).forEach((s, i) => stepNumber.set(s.id, i + 1));

  const q = filter.trim().toLowerCase();
  const stepMatches = (s: Step) =>
    q === "" ||
    s.title.toLowerCase().includes(q) ||
    s.hunks.some((id) => hunksById.get(id)?.path.toLowerCase().includes(q));

  const stepDone = (s: Step) =>
    s.hunks.length > 0 && s.hunks.every((id) => review?.status[id] === "viewed");

  return (
    <div className="grid grid-cols-[320px_1fr] h-screen">
      <aside className="bg-panel border-r border-border p-4 flex flex-col gap-3 sticky top-0 h-screen overflow-y-auto">
        <header className="flex items-center gap-2 flex-wrap">
          <strong>diffthing</strong>
          {walkthrough?.degraded && (
            <span
              className={clsx(badge, "border-warn/60 text-warn bg-warn/10")}
              title="LLM unavailable or failed validation — showing deterministic file-order walkthrough"
            >
              structure unavailable
            </span>
          )}
        </header>

        <input
          type="search"
          placeholder="Filter files"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          className="bg-transparent border border-border rounded-md px-2.5 py-1.5 text-sm placeholder:text-muted outline-none"
        />

        {conn.kind === "connected" && (
          <div className="text-xs text-muted truncate" title="walkthrough organizer">
            {conn.llm}
          </div>
        )}

        {progress && <div className="text-xs text-accent animate-pulse">{progress}</div>}

        {walkthrough?.focus && (
          <section>
            <h2 className="text-xs uppercase tracking-wider text-muted mb-1">Review focus</h2>
            <p className="text-sm text-muted leading-snug">{walkthrough.focus}</p>
          </section>
        )}

        <div className="flex items-center justify-between mt-1">
          <h2 className="text-xs uppercase tracking-wider text-muted">Scope</h2>
          <button
            className="text-xs bg-transparent border-none text-muted cursor-pointer hover:text-accent"
            onClick={() => sendRef.current({ type: "regenerate" })}
            title="Re-run walkthrough organization"
          >
            Regenerate
          </button>
        </div>

        {walkthrough?.scopes.map((scope) => {
          const visible = scope.steps.filter(stepMatches);
          if (visible.length === 0) return null;
          return (
            <section key={scope.id}>
              <h2 className="text-xs uppercase tracking-wider text-muted mt-2 mb-1">
                {scope.title}
              </h2>
              {visible.map((s) => {
                const file = primaryFileFor(s);
                void iconsVersion; // re-run once preloaded icon URLs resolve
                const iconUrl = file ? iconUrlForPath(file.path) : undefined;
                const { files: stepFiles, total } = fileTotals(s, hunksById);

                return (
                  <button
                    key={s.id}
                    className={clsx(
                      "flex flex-col w-full text-left bg-transparent border rounded-md text-text px-2.5 py-2 cursor-pointer hover:border-border gap-1",
                      s.id === selectedStep ? "border-green" : "border-transparent",
                    )}
                    onClick={() => selectStep(s.id)}
                  >
                    <span className="flex w-full min-w-0 items-start gap-1.5">
                      <StepDot done={stepDone(s)} />
                      <span className="min-w-0 flex-1">
                        <span className="block whitespace-normal break-all font-medium leading-snug">
                          {stepNumber.get(s.id)} {s.title}
                        </span>
                      </span>
                      <span className="text-[11px] text-muted shrink-0 text-right leading-tight">
                        {stepFiles.length}
                        <br />
                        {stepFiles.length === 1 ? "file" : "files"}
                      </span>
                    </span>
                    {s.framing && (
                      <span className="text-xs text-muted leading-snug">{s.framing}</span>
                    )}
                    <span className="flex flex-col gap-0.5 text-xs">
                      {stepFiles.map((f) => (
                        <span key={f.path} className="flex items-center gap-1.5">
                          {iconUrl && f.path === file?.path && (
                            <img src={iconUrl} alt="" className="w-3.5 h-3.5 shrink-0" />
                          )}
                          {file && f.path === file.path && (
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
                          <span
                            className="min-w-0 flex-1 whitespace-normal break-all text-muted leading-snug"
                            title={f.path}
                          >
                            {basename(f.path)}
                          </span>
                          <Counts added={f.added} removed={f.removed} />
                        </span>
                      ))}
                      {stepFiles.length > 1 && (
                        <span className="flex justify-end border-t border-border pt-0.5 mt-0.5">
                          <Counts added={total.added} removed={total.removed} />
                        </span>
                      )}
                    </span>
                  </button>
                );
              })}
            </section>
          );
        })}

        {dispatch && (
          <div
            className={clsx(
              "text-[11px] rounded-md border px-2 py-1 leading-snug",
              dispatch.status === "running" &&
                "border-accent/60 text-accent bg-accent/10 animate-pulse",
              dispatch.status === "done" && "border-green/60 text-green bg-green/10",
              dispatch.status === "scope_violation" && "border-warn/60 text-warn bg-warn/10",
              (dispatch.status === "failed" || dispatch.status === "timed_out_reverted") &&
                "border-highest/60 text-highest bg-highest/10",
            )}
            title={dispatch.detail ?? undefined}
          >
            <strong className="capitalize">agent: {dispatch.status.replace(/_/g, " ")}</strong>
            {dispatch.detail && <> — {dispatch.detail}</>}
          </div>
        )}

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
            <div className="px-4 py-3 border-b border-border">
              {scopeOfStep && (
                <div className="text-[11px] uppercase tracking-wider text-muted mb-0.5">
                  {scopeOfStep.title}
                </div>
              )}
              <h1 className="text-base font-semibold m-0">
                {stepNumber.get(step.id)} {step.title}
              </h1>
              {step.framing && (
                <p className="text-sm text-muted leading-snug mt-1 mb-0">{step.framing}</p>
              )}
            </div>
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
                onFlag={(id, line, comment) =>
                  sendRef.current({ type: "add_flag", hunk: id, line, comment })
                }
                onResolve={(id, line) => sendRef.current({ type: "close_flag", hunk: id, line })}
                onDispatch={(id, line, instruction) =>
                  sendRef.current({
                    type: "request_change",
                    hunks: [id],
                    line,
                    instruction,
                    runner: "auto",
                  })
                }
                flags={review?.flags ?? []}
                dispatch={dispatch}
                viewMode={viewMode}
              />
            </div>
          </>
        )}
      </main>
    </div>
  );
}

function Kbd({ children }: { children: React.ReactNode }) {
  return (
    <code className="font-mono text-[0.85em] text-text bg-panel border border-border rounded px-1.5 py-0.5">
      {children}
    </code>
  );
}

// The landing / not-connected screen. Shown whenever the SPA has no live
// daemon — mirrors Drizzle Studio's default state: branded, calm, and always
// showing the one command that gets you running, instead of a bare error.
function Landing({
  tone,
  status,
  children,
}: {
  tone: "wait" | "error" | "ended";
  status: string;
  children: React.ReactNode;
}) {
  const dot =
    tone === "error" ? "bg-highest" : tone === "ended" ? "bg-warn" : "bg-accent animate-pulse";
  return (
    <div className="min-h-screen grid place-content-center px-6">
      <div className="flex w-[min(30rem,90vw)] flex-col items-center gap-7 text-center">
        <div className="flex items-center gap-2.5 text-2xl font-semibold tracking-tight text-text">
          <span className="h-2.5 w-2.5 rounded-full bg-green shadow-[0_0_14px_2px] shadow-green/50" />
          diffthing
        </div>

        <div className="flex items-center gap-2 text-sm text-muted">
          <span className={clsx("h-2 w-2 rounded-full", dot)} />
          {status}
        </div>

        <div className="space-y-3 text-sm leading-relaxed text-muted">{children}</div>

        <div className="w-full text-left">
          <div className="mb-1.5 text-xs uppercase tracking-wide text-muted/70">
            Run in your project
          </div>
          <div className="flex items-center gap-2 rounded-lg border border-border bg-panel px-4 py-3 font-mono text-sm">
            <span className="select-none text-muted">$</span>
            <span className="text-text">npx diffthing</span>
          </div>
        </div>

        <div className="text-xs text-muted/70">
          AI organizes the diff. Only you review.
        </div>
      </div>
    </div>
  );
}
