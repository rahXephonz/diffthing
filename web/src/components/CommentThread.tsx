import clsx from "clsx";
import type { DispatchState } from "../libs/store";
import type { Flag, FlagEntry } from "../libs/protocol";

interface Props {
  flags: Flag[];
  dispatch: DispatchState | null;
  draft: string;
  onDraftChange: (v: string) => void;
  onSubmit: () => void; // add_flag → append reply / open thread
  onResolve: () => void; // close_flag (human only)
  onDispatch: (instruction: string) => void; // request_change
  onCancel: () => void; // close the bare composer
  composerOnly: boolean; // opened via "comment" with no thread yet
}

type Author = { label: string; initials: string; cls: string };

const AUTHOR: Record<FlagEntry["kind"], Author> = {
  human_comment: { label: "You", initials: "Y", cls: "bg-accent/20 text-accent" },
  agent_claim: { label: "Agent", initials: "AI", cls: "bg-green/20 text-green" },
  dispatch_note: { label: "diffthing", initials: "!", cls: "bg-warn/20 text-warn" },
};

function Avatar({ kind }: { kind: FlagEntry["kind"] }) {
  const a = AUTHOR[kind];
  return (
    <span
      className={clsx(
        "w-5 h-5 shrink-0 rounded-full grid place-content-center text-[9px] font-bold",
        a.cls,
      )}
    >
      {a.initials}
    </span>
  );
}

function Comment({ entry }: { entry: FlagEntry }) {
  const a = AUTHOR[entry.kind];
  return (
    <div className="border-border first:border-t-0">
      <div className="flex items-center gap-2 px-3 py-1.5">
        <Avatar kind={entry.kind} />
        <span className="text-xs font-semibold">{a.label}</span>
        {entry.kind === "agent_claim" && (
          <span className="text-[10px] px-1.5 py-0.5 rounded-full border border-green/50 text-green">
            claim · unverified
          </span>
        )}
        {entry.kind === "dispatch_note" && (
          <span className="text-[10px] text-warn">dispatch note</span>
        )}
      </div>
      <p className="px-3 pb-2 text-sm leading-snug whitespace-pre-wrap m-0">{entry.body}</p>
      {entry.kind === "agent_claim" && (
        <p className="px-3 pb-2 -mt-1 text-[10px] text-muted italic m-0">
          Reconciliation confirms the code actually changed — you decide if it's right.
        </p>
      )}
    </div>
  );
}

function Composer({
  draft,
  onDraftChange,
  onSubmit,
  onCancel,
  placeholder,
  showCancel,
}: {
  draft: string;
  onDraftChange: (v: string) => void;
  onSubmit: () => void;
  onCancel: () => void;
  placeholder: string;
  showCancel: boolean;
}) {
  return (
    <div className="flex flex-col gap-2 px-3 py-2">
      <textarea
        value={draft}
        onChange={(e) => onDraftChange(e.target.value)}
        placeholder={placeholder}
        rows={2}
        onKeyDown={(e) => {
          if ((e.metaKey || e.ctrlKey) && e.key === "Enter") onSubmit();
        }}
        className="w-full resize-y bg-bg border border-border rounded-md px-2 py-1.5 text-sm placeholder:text-muted outline-none"
      />
      <div className="flex gap-1.5 justify-end">
        {showCancel && (
          <button
            className="text-xs bg-transparent border border-border rounded-md px-2.5 py-1 cursor-pointer text-muted hover:border-border"
            onClick={onCancel}
          >
            Cancel
          </button>
        )}
        <button
          className="text-xs bg-green/15 border border-green/50 rounded-md px-2.5 py-1 cursor-pointer text-green hover:bg-green/25 disabled:opacity-40"
          disabled={draft.trim() === ""}
          onClick={onSubmit}
        >
          Comment
        </button>
      </div>
    </div>
  );
}

export default function CommentThread({
  flags,
  dispatch,
  draft,
  onDraftChange,
  onSubmit,
  onResolve,
  onDispatch,
  onCancel,
  composerOnly,
}: Props) {
  const running = dispatch?.status === "running";

  if (flags.length === 0) {
    if (!composerOnly) return null;
    return (
      <div className="px-4 py-3 bg-panel/40 border-b border-border">
        <div className="rounded-md border border-border bg-panel">
          <Composer
            draft={draft}
            onDraftChange={onDraftChange}
            onSubmit={onSubmit}
            onCancel={onCancel}
            placeholder="Leave a comment on this hunk…"
            showCancel
          />
        </div>
      </div>
    );
  }

  return (
    <div className="px-4 py-3 bg-panel/40 border-b border-border flex flex-col gap-3">
      {flags.map((flag, fi) => {
        const resolved = !flag.open;
        const instruction = [...flag.thread]
          .reverse()
          .find((entry) => entry.kind === "human_comment")?.body;
        return (
          <div
            key={fi}
            className={clsx(
              "rounded-md border bg-panel overflow-hidden",
              resolved ? "border-border/60 opacity-70" : "border-border",
            )}
          >
            {resolved && (
              <div className="flex items-center gap-2 px-3 py-1.5 bg-green/5 border-b border-border text-xs text-green">
                ✓ Resolved
              </div>
            )}
            {flag.thread.map((e, i) => (
              <Comment key={i} entry={e} />
            ))}

            {!resolved && (
              <div className="border-t border-border bg-panel/60">
                {flag.addressed_claim && (
                  <div className="px-3 py-1.5 text-[11px] text-accent border-b border-border">
                    Agent claims this is addressed — review the change, then resolve.
                  </div>
                )}
                <Composer
                  draft={draft}
                  onDraftChange={onDraftChange}
                  onSubmit={onSubmit}
                  onCancel={onCancel}
                  placeholder="Reply…"
                  showCancel={false}
                />
                <div className="flex items-center gap-1.5 px-3 py-2 border-t border-border">
                  <button
                    className="text-xs bg-transparent border border-border rounded-md px-2.5 py-1 cursor-pointer text-text hover:border-accent disabled:opacity-40"
                    disabled={running}
                    onClick={() => onDispatch(instruction ?? "")}
                    title="Dispatch to your agent — it edits the code, then reports what it changed"
                  >
                    {running ? "agent busy…" : "Fix with agent"}
                  </button>
                  <button
                    className="text-xs bg-transparent border border-border rounded-md px-2.5 py-1 cursor-pointer text-muted hover:border-green hover:text-green"
                    onClick={onResolve}
                    title="Resolving is always your call"
                  >
                    Resolve
                  </button>
                  {dispatch && dispatch.status !== "running" && (
                    <span
                      className={clsx(
                        "text-[11px] ml-auto truncate",
                        dispatch.status === "done" && "text-green",
                        dispatch.status === "scope_violation" && "text-warn",
                        (dispatch.status === "failed" ||
                          dispatch.status === "timed_out_reverted") &&
                          "text-highest",
                      )}
                      title={dispatch.detail ?? undefined}
                    >
                      {dispatch.status.replace(/_/g, " ")}
                    </span>
                  )}
                </div>
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
