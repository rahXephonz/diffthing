import { useEffect, useMemo, useRef, useState } from "react";
import Landing from "./components/Landing";
import ReviewContent from "./components/ReviewContent";
import { Kbd } from "./components/ReviewChrome";
import ReviewSidebar from "./components/ReviewSidebar";
import type { ViewMode } from "./components/DiffPane";
import { connect, parseFragment } from "./libs/connection";
import { preloadIconForPath } from "./libs/fileIcon";
import type { ClientMsg, Step } from "./libs/protocol";
import { useStore } from "./libs/store";

export default function App() {
  const sendRef = useRef<(message: ClientMsg) => void>(() => null);
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
    if (pending) sendRef.current({ type: "apply_update", to_revision: pending.revision });
  }, [pending]);

  const hunksById = useMemo(
    () => new Map(files.flatMap((file) => file.hunks.map((hunk) => [hunk.id, hunk] as const))),
    [files],
  );
  const fileByPath = useMemo(
    () => new Map(files.map((file) => [file.path, file] as const)),
    [files],
  );
  const query = filter.trim().toLowerCase();

  const flagTextByHunk = useMemo(() => {
    const textByHunk = new Map<string, string>();
    for (const flag of review?.flags ?? []) {
      const text = flag.thread
        .map((entry) => entry.body)
        .join("\n")
        .toLowerCase();
      textByHunk.set(flag.hunk, `${textByHunk.get(flag.hunk) ?? ""}\n${text}`);
    }
    return textByHunk;
  }, [review]);

  const stepMatches = (step: Step) =>
    query === "" ||
    step.title.toLowerCase().includes(query) ||
    step.hunks.some((id) => {
      const hunk = hunksById.get(id);
      if (!hunk) return false;
      return (
        hunk.path.toLowerCase().includes(query) ||
        hunk.lines.some((line) => line.toLowerCase().includes(query)) ||
        (flagTextByHunk.get(id)?.includes(query) ?? false)
      );
    });

  const stepDone = (step: Step) =>
    step.hunks.length > 0 && step.hunks.every((id) => review?.status[id] === "viewed");

  const stepForFile = useMemo(() => {
    const firstStepByPath = new Map<string, string>();
    for (const scope of walkthrough?.scopes ?? []) {
      for (const step of scope.steps) {
        for (const id of step.hunks) {
          const hunk = hunksById.get(id);
          if (hunk && !firstStepByPath.has(hunk.path)) firstStepByPath.set(hunk.path, step.id);
        }
      }
    }
    return firstStepByPath;
  }, [walkthrough, hunksById]);

  const filterRef = useRef<HTMLInputElement>(null);
  const orderedSteps =
    walkthrough?.scopes.flatMap((scope) => scope.steps.filter(stepMatches)) ?? [];
  const navRef = useRef<{ steps: Step[]; selected: string | null }>({ steps: [], selected: null });
  useEffect(() => {
    navRef.current = { steps: orderedSteps, selected: selectedStep };
  });
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement;
      const typing =
        target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable;
      if (typing) {
        if (event.key === "Escape") target.blur();
        return;
      }
      if (event.metaKey || event.ctrlKey || event.altKey) return;
      if (event.key === "/") {
        event.preventDefault();
        filterRef.current?.focus();
        return;
      }
      if (event.key !== "j" && event.key !== "k") return;
      const { steps, selected } = navRef.current;
      if (steps.length === 0) return;
      const index = steps.findIndex((step) => step.id === selected);
      const next =
        index === -1
          ? steps[0]
          : steps[Math.min(steps.length - 1, Math.max(0, index + (event.key === "j" ? 1 : -1)))];
      selectStep(next.id);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const [iconsVersion, setIconsVersion] = useState(0);
  useEffect(() => {
    const paths = new Set(files.map((file) => file.path));
    if (paths.size === 0) return;
    Promise.all([...paths].map((path) => preloadIconForPath(path))).then(() => {
      setIconsVersion((version) => version + 1);
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

  const scope =
    walkthrough?.scopes.find((item) => item.steps.some((s) => s.id === selectedStep)) ?? null;
  const step = scope?.steps.find((item) => item.id === selectedStep) ?? null;
  const stepHunks = (step?.hunks ?? [])
    .map((id) => hunksById.get(id))
    .filter((hunk) => hunk != null);
  const stepNumber = new Map<string, number>();
  walkthrough?.scopes
    .flatMap((item) => item.steps)
    .forEach((item, i) => stepNumber.set(item.id, i + 1));

  return (
    <div className="grid grid-cols-[320px_1fr] h-screen">
      <ReviewSidebar
        daemonVersion={conn.daemonVersion}
        llm={conn.llm}
        walkthrough={walkthrough}
        files={files}
        review={review}
        dispatch={dispatch}
        progress={progress}
        filter={filter}
        filterRef={filterRef}
        iconsVersion={iconsVersion}
        selectedStep={selectedStep}
        selectedPaths={new Set(stepHunks.map((hunk) => hunk.path))}
        hunksById={hunksById}
        fileByPath={fileByPath}
        stepForFile={stepForFile}
        stepNumber={stepNumber}
        stepMatches={stepMatches}
        stepDone={stepDone}
        onFilterChange={setFilter}
        onSelectStep={selectStep}
        send={(message) => sendRef.current(message)}
      />
      <ReviewContent
        scope={scope}
        step={step}
        stepNumber={step ? stepNumber.get(step.id) : undefined}
        hunks={stepHunks}
        scores={scores}
        review={review}
        dispatch={dispatch}
        viewMode={viewMode}
        onViewModeChange={setViewMode}
        send={(message) => sendRef.current(message)}
      />
    </div>
  );
}
