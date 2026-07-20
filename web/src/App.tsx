import clsx from "clsx";
import { Check, ChevronDown, ChevronRight, Columns2, Rows3 } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import DiffPane, { type ViewMode } from "./components/DiffPane";
import { connect, parseFragment } from "./libs/connection";
import { iconUrlForPath, preloadIconForPath, STATUS_CLASS, STATUS_LETTER } from "./libs/fileIcon";
import type { ClientMsg, FileDiff, Step } from "./libs/protocol";
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
      className="w-4 h-4 shrink-0 rounded-full bg-green text-panel grid place-content-center"
      title="all hunks viewed"
    >
      <Check size={10} strokeWidth={3} />
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

  const q = filter.trim().toLowerCase();

  // In-review search: a step matches on its title, any of its file paths,
  // any changed line of code, or any comment in a thread anchored to it.
  const flagTextByHunk = useMemo(() => {
    const m = new Map<string, string>();
    for (const f of review?.flags ?? []) {
      const text = f.thread
        .map((e) => e.body)
        .join("\n")
        .toLowerCase();
      m.set(f.hunk, `${m.get(f.hunk) ?? ""}\n${text}`);
    }
    return m;
  }, [review]);

  const stepMatches = (s: Step) =>
    q === "" ||
    s.title.toLowerCase().includes(q) ||
    s.hunks.some((id) => {
      const h = hunksById.get(id);
      if (!h) return false;
      return (
        h.path.toLowerCase().includes(q) ||
        h.lines.some((line) => line.toLowerCase().includes(q)) ||
        (flagTextByHunk.get(id)?.includes(q) ?? false)
      );
    });

  const stepDone = (s: Step) =>
    s.hunks.length > 0 && s.hunks.every((id) => review?.status[id] === "viewed");

  // First step that touches each file — the file tree's click target.
  const stepForFile = useMemo(() => {
    const m = new Map<string, string>();
    for (const scope of walkthrough?.scopes ?? []) {
      for (const s of scope.steps) {
        for (const id of s.hunks) {
          const h = hunksById.get(id);
          if (h && !m.has(h.path)) m.set(h.path, s.id);
        }
      }
    }
    return m;
  }, [walkthrough, hunksById]);

  // Keyboard navigation. j/k walk the (filtered) step order; / focuses
  // search. n/p/v live in DiffPane, which owns the hunk cursor. Handler
  // reads through a ref so the listener binds once.
  const filterRef = useRef<HTMLInputElement>(null);
  const orderedSteps = walkthrough?.scopes.flatMap((sc) => sc.steps.filter(stepMatches)) ?? [];
  const navRef = useRef<{ steps: Step[]; selected: string | null }>({
    steps: [],
    selected: null,
  });
  useEffect(() => {
    navRef.current = { steps: orderedSteps, selected: selectedStep };
  });
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement;
      const typing =
        target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable;
      if (typing) {
        if (e.key === "Escape") target.blur();
        return;
      }
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      if (e.key === "/") {
        e.preventDefault();
        filterRef.current?.focus();
        return;
      }
      if (e.key !== "j" && e.key !== "k") return;
      const { steps, selected } = navRef.current;
      if (steps.length === 0) return;
      const index = steps.findIndex((s) => s.id === selected);
      const next =
        index === -1
          ? steps[0]
          : steps[Math.min(steps.length - 1, Math.max(0, index + (e.key === "j" ? 1 : -1)))];
      selectStep(next.id);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

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
    // Every changed file shows an icon somewhere (file tree + step rows).
    const paths = new Set(files.map((f) => f.path));
    if (paths.size === 0) return;
    Promise.all([...paths].map((p) => preloadIconForPath(p))).then(() => {
      setIconsVersion((v) => v + 1);
    });
  }, [files]);

  if (conn.kind === "connecting" || conn.kind === "probing") {
    return (
      <Landing tone="wait" status={conn.kind === "connecting" ? "Connecting…" : "Diagnosing…"}>
        <p>
          Reviewing changes on your machine. If this hangs, make sure the daemon is still running in
          your project.
        </p>
      </Landing>
    );
  }

  if (conn.kind === "diagnosed") {
    return (
      <Landing tone="error" status="Not connected">
        <p>{conn.detail}</p>
        <p className="text-subtle">
          Or run <Kbd>npx diffthing --offline</Kbd> to serve this UI directly from 127.0.0.1.
        </p>
      </Landing>
    );
  }

  if (conn.kind === "session_ended") {
    return (
      <Landing tone="ended" status="Session ended">
        <p>
          The daemon restarted, so this tab’s token is stale. Rerun the command below and open the
          fresh URL it prints.
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
          ref={filterRef}
          type="search"
          placeholder="Search files, code, comments (/)"
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

        <FileTreeSection
          files={files}
          query={q}
          iconsVersion={iconsVersion}
          selectedPaths={new Set(stepHunks.map((h) => h.path))}
          onSelectFile={(path) => {
            const id = stepForFile.get(path);
            if (id) selectStep(id);
          }}
        />

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

        <footer className="flex flex-col gap-2">
          <button
            className={chromeButton}
            onClick={() => sendRef.current({ type: "export_review" })}
            disabled={!review || review.flags.filter((f) => f.open).length === 0}
          >
            Export review ({review?.flags.filter((f) => f.open).length ?? 0} open flags)
          </button>
          <div className="text-[10px] text-muted/70">
            <Kbd>j</Kbd>/<Kbd>k</Kbd> steps · <Kbd>n</Kbd>/<Kbd>p</Kbd> hunks · <Kbd>v</Kbd> mark
            viewed · <Kbd>/</Kbd> search
          </div>
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
                    "inline-flex items-center gap-1 text-xs px-2 py-1 rounded-md border cursor-pointer",
                    viewMode === mode
                      ? "border-accent text-accent"
                      : "border-border text-muted hover:border-accent",
                  )}
                  onClick={() => setViewMode(mode)}
                >
                  {mode === "unified" ? <Rows3 size={12} /> : <Columns2 size={12} />}
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

type DirNode = { name: string; path: string; dirs: DirNode[]; files: FileDiff[] };

function buildTree(files: FileDiff[]): DirNode {
  const root: DirNode = { name: "", path: "", dirs: [], files: [] };
  for (const f of [...files].sort((a, b) => a.path.localeCompare(b.path))) {
    const parts = f.path.split("/");
    let node = root;
    let prefix = "";
    for (const part of parts.slice(0, -1)) {
      prefix = prefix ? `${prefix}/${part}` : part;
      let child = node.dirs.find((d) => d.name === part);
      if (!child) {
        child = { name: part, path: prefix, dirs: [], files: [] };
        node.dirs.push(child);
      }
      node = child;
    }
    node.files.push(f);
  }
  return root;
}

// Changed-files tree. Clicking a file jumps to the first step that touches
// it; the search query prunes the tree alongside the step list.
function FileTreeSection({
  files,
  query,
  iconsVersion,
  selectedPaths,
  onSelectFile,
}: {
  files: FileDiff[];
  query: string;
  iconsVersion: number;
  selectedPaths: Set<string>;
  onSelectFile: (path: string) => void;
}) {
  const [closed, setClosed] = useState<Set<string>>(new Set());
  const tree = useMemo(() => {
    const visible = query ? files.filter((f) => f.path.toLowerCase().includes(query)) : files;
    return buildTree(visible);
  }, [files, query]);
  void iconsVersion; // re-render once preloaded icon URLs resolve

  if (files.length === 0) return null;

  const toggle = (path: string) =>
    setClosed((current) => {
      const next = new Set(current);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });

  const renderDir = (node: DirNode, depth: number): React.ReactNode => (
    <>
      {node.dirs.map((dir) => {
        // A live search shows every match — collapsed state only applies
        // when browsing.
        const isClosed = !query && closed.has(dir.path);
        return (
          <div key={dir.path}>
            <button
              className="flex w-full items-center gap-1 bg-transparent border-none text-muted cursor-pointer py-0.5 text-xs hover:text-text"
              style={{ paddingLeft: depth * 12 }}
              onClick={() => toggle(dir.path)}
            >
              {isClosed ? <ChevronRight size={12} /> : <ChevronDown size={12} />}
              <span className="truncate">{dir.name}</span>
            </button>
            {!isClosed && renderDir(dir, depth + 1)}
          </div>
        );
      })}
      {node.files.map((file) => {
        const totals = file.hunks.reduce(
          (acc, h) => ({ added: acc.added + h.added, removed: acc.removed + h.removed }),
          { added: 0, removed: 0 },
        );
        const iconUrl = iconUrlForPath(file.path);
        return (
          <button
            key={file.path}
            className={clsx(
              "flex w-full items-center gap-1.5 rounded bg-transparent border-none cursor-pointer py-0.5 pr-1 text-xs",
              selectedPaths.has(file.path) ? "text-text bg-panel" : "text-muted hover:text-text",
            )}
            style={{ paddingLeft: depth * 12 + 14 }}
            title={file.path}
            onClick={() => onSelectFile(file.path)}
          >
            {iconUrl && <img src={iconUrl} alt="" className="w-3.5 h-3.5 shrink-0" />}
            <span
              className={clsx(
                "text-[10px] font-bold w-3 shrink-0 text-center",
                STATUS_CLASS[file.status],
              )}
              title={file.status}
            >
              {STATUS_LETTER[file.status]}
            </span>
            <span className="min-w-0 flex-1 truncate text-left">{basename(file.path)}</span>
            <Counts added={totals.added} removed={totals.removed} />
          </button>
        );
      })}
    </>
  );

  return (
    <section>
      <h2 className="text-xs uppercase tracking-wider text-muted mb-1">Files</h2>
      {renderDir(tree, 0)}
    </section>
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

        <div className="text-xs text-muted/70">AI organizes the diff. Only you review.</div>
      </div>
    </div>
  );
}
