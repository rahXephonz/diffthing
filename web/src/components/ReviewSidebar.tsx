import clsx from "clsx";
import { Check, CheckCheck, Tag } from "lucide-react";
import type { RefObject } from "react";
import { iconUrlForPath, STATUS_CLASS, STATUS_LETTER } from "../libs/fileIcon";
import type { ClientMsg, FileDiff, Hunk, ReviewState, Step, Walkthrough } from "../libs/protocol";
import type { DispatchState } from "../libs/store";
import { basename, fileTotals } from "../libs/utils";
import FileTreeSection from "./FileTreeSection";
import { badge, chromeButton, Counts, Highlight, Kbd } from "./ReviewChrome";

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

function VersionBadge({ version }: { version: string }) {
  return (
    <span
      className="inline-flex items-center gap-1 mt-1 rounded-full border border-border px-2 py-0.5 text-[11px] font-medium tracking-normal text-muted"
      title={`daemon v${version}`}
    >
      <Tag size={11} aria-hidden="true" />v{version}
    </span>
  );
}

interface ReviewSidebarProps {
  daemonVersion: string;
  llm: string;
  walkthrough: Walkthrough | null;
  files: FileDiff[];
  review: ReviewState | null;
  dispatch: DispatchState | null;
  progress: string | null;
  filter: string;
  filterRef: RefObject<HTMLInputElement>;
  iconsVersion: number;
  selectedStep: string | null;
  selectedPaths: Set<string>;
  hunksById: Map<string, Hunk>;
  fileByPath: Map<string, FileDiff>;
  stepForFile: Map<string, string>;
  stepNumber: Map<string, number>;
  stepMatches: (step: Step) => boolean;
  stepDone: (step: Step) => boolean;
  unviewedCount: number;
  onFilterChange: (value: string) => void;
  onSelectStep: (id: string) => void;
  send: (message: ClientMsg) => void;
}

export default function ReviewSidebar(props: ReviewSidebarProps) {
  const {
    daemonVersion,
    llm,
    walkthrough,
    files,
    review,
    dispatch,
    progress,
    filter,
    filterRef,
    iconsVersion,
    selectedStep,
    selectedPaths,
    hunksById,
    fileByPath,
    stepForFile,
    stepNumber,
    stepMatches,
    stepDone,
    unviewedCount,
    onFilterChange,
    onSelectStep,
    send,
  } = props;

  const primaryFileFor = (step: Step) => {
    const hunk = hunksById.get(step.hunks[0]);
    return hunk ? (fileByPath.get(hunk.path) ?? null) : null;
  };

  const query = filter.trim().toLowerCase();

  return (
    // Fixed chrome (logo, search, file tree) with ONLY the scope list
    // scrolling: the aside never scrolls as a whole, the middle pane does.
    <aside className="bg-panel border-r border-border p-4 flex flex-col gap-3 sticky top-0 h-screen overflow-hidden">
      <header className="flex items-center gap-2 flex-wrap">
        <div className="flex items-center gap-2 text-xl font-semibold tracking-tight text-text">
          <img
            src="/images/diffthing-logo.png"
            alt="diffthing-logo"
            aria-hidden="true"
            className="h-7 w-7 shrink-0 object-contain"
          />
          diffthing
          <VersionBadge version={daemonVersion} />
        </div>
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
        onChange={(event) => onFilterChange(event.target.value)}
        className="bg-transparent border border-border rounded-md px-2.5 py-1.5 text-sm placeholder:text-muted outline-none"
      />
      <div className="text-xs text-muted truncate" title="walkthrough organizer">
        {llm}
      </div>
      {progress && <div className="text-xs text-accent animate-pulse">{progress}</div>}

      <FileTreeSection
        files={files}
        query={query}
        iconsVersion={iconsVersion}
        selectedPaths={selectedPaths}
        onSelectFile={(path) => {
          const id = stepForFile.get(path);
          if (id) onSelectStep(id);
        }}
      />

      {/* Everything below (focus + scopes + agent status) is the only
          scrolling region; chrome above and footer below stay put. */}
      <div className="flex-1 min-h-0 overflow-y-auto flex flex-col gap-3">
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
            onClick={() => send({ type: "regenerate" })}
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
              {visible.map((step) => {
                const file = primaryFileFor(step);
                void iconsVersion;
                const iconUrl = file ? iconUrlForPath(file.path) : undefined;
                const { files: stepFiles, total } = fileTotals(step, hunksById);
                return (
                  <button
                    key={step.id}
                    className={clsx(
                      "flex flex-col w-full text-left bg-transparent border rounded-md text-text px-2.5 py-2 cursor-pointer hover:border-border gap-1",
                      step.id === selectedStep ? "border-green" : "border-transparent",
                    )}
                    onClick={() => onSelectStep(step.id)}
                  >
                    <span className="flex w-full min-w-0 items-start gap-1.5">
                      <StepDot done={stepDone(step)} />
                      <span className="min-w-0 flex-1">
                        <span className="block whitespace-normal break-all font-medium leading-snug">
                          {stepNumber.get(step.id)} <Highlight text={step.title} query={query} />
                        </span>
                      </span>
                      <span className="text-[11px] text-muted shrink-0 text-right leading-tight">
                        {stepFiles.length}
                        <br />
                        {stepFiles.length === 1 ? "file" : "files"}
                      </span>
                    </span>
                    {step.framing && (
                      <span className="text-xs text-muted leading-snug">
                        <Highlight text={step.framing} query={query} />
                      </span>
                    )}
                    <span className="flex flex-col gap-0.5 text-xs">
                      {stepFiles.map((stepFile) => (
                        <span key={stepFile.path} className="flex items-center gap-1.5">
                          {iconUrl && stepFile.path === file?.path && (
                            <img src={iconUrl} alt="" className="w-3.5 h-3.5 shrink-0" />
                          )}
                          {file && stepFile.path === file.path && (
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
                            title={stepFile.path}
                          >
                            <Highlight text={basename(stepFile.path)} query={query} />
                          </span>
                          <Counts added={stepFile.added} removed={stepFile.removed} />
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
      </div>

      <footer className="flex flex-col gap-2">
        <button
          className={clsx(
            chromeButton,
            "flex items-center justify-center gap-1.5",
            unviewedCount > 0 && "border-green/60 text-green hover:border-green",
          )}
          onClick={() => {
            // Marks every hunk viewed and stages each newly-approved file —
            // a working-tree side effect, so confirm before the sweep.
            if (window.confirm(`Mark all ${unviewedCount} unviewed hunk(s) as reviewed?`)) {
              send({ type: "mark_all_viewed" });
            }
          }}
          disabled={unviewedCount === 0}
          title="Mark every hunk viewed — use when you've reviewed everything and found no issues"
        >
          <CheckCheck size={14} aria-hidden="true" />
          {unviewedCount === 0 ? "All reviewed" : `Mark all reviewed (${unviewedCount})`}
        </button>
        <button
          className={chromeButton}
          onClick={() => send({ type: "export_review" })}
          disabled={!review || review.flags.filter((flag) => flag.open).length === 0}
        >
          Export review ({review?.flags.filter((flag) => flag.open).length ?? 0} open flags)
        </button>
        <div className="text-[10px] text-muted/70">
          <Kbd>j</Kbd>/<Kbd>k</Kbd> steps · <Kbd>n</Kbd>/<Kbd>p</Kbd> hunks · <Kbd>v</Kbd> mark
          viewed · <Kbd>/</Kbd> search
        </div>
      </footer>
    </aside>
  );
}
