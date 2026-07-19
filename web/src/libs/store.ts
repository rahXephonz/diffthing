// Client store. Deliberately thin: the daemon is the source of truth,
// the browser renders snapshots + events. No durable state lives here.

import { create } from "zustand";
import type { ConnState } from "./connection";
import type {
  FileDiff,
  ImpactScore,
  ReconcileReport,
  ReviewState,
  ServerMsg,
  Walkthrough,
} from "./protocol";

interface PendingUpdate {
  revision: number;
  report: ReconcileReport;
}

export interface DispatchState {
  jobId: string;
  status: "running" | "done" | "failed" | "timed_out_reverted" | "scope_violation";
  detail: string | null;
}

interface Store {
  conn: ConnState;
  walkthrough: Walkthrough | null;
  files: FileDiff[];
  scores: Record<string, ImpactScore>;
  review: ReviewState | null;
  pending: PendingUpdate | null;
  selectedStep: string | null;
  exportMarkdown: string | null;
  /** Live line from the daemon while background organization runs. */
  progress: string | null;
  /** Latest agent-dispatch lifecycle status, or null when idle. */
  dispatch: DispatchState | null;

  setConn: (c: ConnState) => void;
  onServerMsg: (m: ServerMsg) => void;
  selectStep: (id: string) => void;
  clearDispatch: () => void;
}

export const useStore = create<Store>((set) => ({
  conn: { kind: "connecting" },
  walkthrough: null,
  files: [],
  scores: {},
  review: null,
  pending: null,
  selectedStep: null,
  exportMarkdown: null,
  progress: null,
  dispatch: null,

  setConn: (conn) => set({ conn }),

  onServerMsg: (m) => {
    switch (m.type) {
      case "snapshot":
        // Snapshot replaces everything — including clearing the banner.
        set({
          walkthrough: m.walkthrough,
          files: m.files,
          scores: m.scores,
          review: m.review,
          pending: null,
          progress: null,
        });
        break;
      case "dispatch_status":
        // Terminal states linger so the reader sees the outcome; Running
        // supersedes any stale prior result.
        set({ dispatch: { jobId: m.job_id, status: m.status, detail: m.detail } });
        break;
      case "generation_progress":
        set({ progress: m.message });
        break;
      case "review_updated":
        // Review-only delta (comment, viewed-mark, flag close). Never
        // touches the diff — keep everything else, swap review in place.
        set({ review: m.review });
        break;
      case "update_available":
        // Banner only. The screen NEVER reflows on its own.
        set({ pending: { revision: m.revision, report: m.report } });
        break;
      case "review_export":
        set({ exportMarkdown: m.markdown });
        break;
      default:
        break;
    }
  },

  selectStep: (id) => set({ selectedStep: id }),
  clearDispatch: () => set({ dispatch: null }),
}));
