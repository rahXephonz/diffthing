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

interface Store {
  conn: ConnState;
  walkthrough: Walkthrough | null;
  files: FileDiff[];
  scores: Record<string, ImpactScore>;
  review: ReviewState | null;
  pending: PendingUpdate | null;
  selectedStep: string | null;
  exportMarkdown: string | null;

  setConn: (c: ConnState) => void;
  onServerMsg: (m: ServerMsg) => void;
  selectStep: (id: string) => void;
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
        });
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
}));
