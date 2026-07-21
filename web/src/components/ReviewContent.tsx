import clsx from "clsx";
import { Columns2, Rows3 } from "lucide-react";
import { useStore, type DispatchState } from "../libs/store";
import type { ClientMsg, Hunk, ImpactScore, ReviewState, Scope, Step } from "../libs/protocol";
import DiffPane, { type ViewMode } from "./DiffPane";

export default function ReviewContent({
  scope,
  step,
  stepNumber,
  hunks,
  scores,
  review,
  dispatch,
  viewMode,
  onViewModeChange,
  send,
}: {
  scope: Scope | null;
  step: Step | null;
  stepNumber: number | undefined;
  hunks: Hunk[];
  scores: Record<string, ImpactScore>;
  review: ReviewState | null;
  dispatch: DispatchState | null;
  viewMode: ViewMode;
  onViewModeChange: (mode: ViewMode) => void;
  send: (message: ClientMsg) => void;
}) {
  const { clearDispatch } = useStore();

  return (
    <main className="h-screen flex flex-col overflow-hidden">
      {!step && (
        <div className="p-6">
          <p className="text-muted">Select a step to start reading.</p>
        </div>
      )}
      {step && (
        <>
          <div className="px-4 py-3 border-b border-border">
            {scope && (
              <div className="text-[11px] uppercase tracking-wider text-muted mb-0.5">
                {scope.title}
              </div>
            )}
            <h1 className="text-base font-semibold m-0">
              {stepNumber} {step.title}
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
                onClick={() => onViewModeChange(mode)}
              >
                {mode === "unified" ? <Rows3 size={12} /> : <Columns2 size={12} />}
                {mode}
              </button>
            ))}
          </div>
          <div className="flex-1 min-h-0">
            <DiffPane
              hunks={hunks}
              scores={scores}
              statusOf={(id) => review?.status[id] ?? "unviewed"}
              onMarkViewed={(id) => send({ type: "mark_viewed", hunk: id })}
              onFlag={(id, line, comment) => send({ type: "add_flag", hunk: id, line, comment })}
              onResolve={(id, line) => send({ type: "close_flag", hunk: id, line })}
              onDispatch={(id, line, instruction) => {
                clearDispatch();
                send({
                  type: "request_change",
                  hunks: [id],
                  line,
                  instruction,
                  runner: "auto",
                });
              }}
              flags={review?.flags ?? []}
              dispatch={dispatch}
              viewMode={viewMode}
            />
          </div>
        </>
      )}
    </main>
  );
}
