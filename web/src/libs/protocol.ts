// Wire protocol — mirrors crates/core/src/protocol.rs (PROTOCOL_VERSION 1).
// TODO(M1): replace this hand-written mirror with ts-rs codegen from the
// Rust structs (`cargo test --features ts-export` emits bindings). Until
// then, this file is the single place to keep in sync.

export const PROTOCOL_VERSION = 4;

export type Impact = "low" | "medium" | "high" | "highest";

export interface ImpactScore {
  impact: Impact;
  points: number;
  reasons: string[];
}

export interface Hunk {
  id: string;
  path: string;
  new_start: number;
  old_start: number;
  added: number;
  removed: number;
  lines: string[];
}

export interface FileDiff {
  path: string;
  old_path: string | null;
  status: "added" | "modified" | "deleted" | "renamed";
  hunks: Hunk[];
}

export interface Step {
  id: string;
  title: string;
  framing: string;
  hunks: string[];
}

export interface Scope {
  id: string;
  title: string;
  steps: Step[];
}

export interface Walkthrough {
  schema_version: number;
  revision: number;
  tree_state: string;
  /** Reading-order description ("review focus"). Null in fallback mode. */
  focus: string | null;
  scopes: Scope[];
  degraded: boolean;
}

export type HunkStatus = "unviewed" | "viewed" | "changed_since_viewed";

/** No "verdict" kind exists on purpose — the machine never judges the code. */
export type FlagEntryKind = "human_comment" | "agent_claim" | "dispatch_note";

export interface FlagEntry {
  kind: FlagEntryKind;
  body: string;
  revision: number;
}

export interface Flag {
  hunk: string;
  /** Index into hunk.lines this thread anchors to (per-line comment), or
   *  null for a hunk-level comment. Render offset only — identity is the hunk. */
  line: number | null;
  /** Oldest-first: the human's comment, agent change-claims, dispatch notes. */
  thread: FlagEntry[];
  open: boolean;
  addressed_claim: boolean;
}

export interface ReviewState {
  status: Record<string, HunkStatus>;
  flags: Flag[];
  tombstones: Flag[];
}

export interface Lineage {
  from: string;
  to: string;
}

export interface ReconcileReport {
  carried: string[];
  changed: Lineage[];
  added: string[];
  removed: string[];
}

export type ClientMsg =
  | { type: "hello"; protocol: number; token: string }
  | { type: "mark_viewed"; hunk: string }
  | { type: "add_flag"; hunk: string; line: number | null; comment: string }
  | { type: "close_flag"; hunk: string; line: number | null }
  | {
      type: "request_change";
      hunks: string[];
      line: number | null;
      instruction: string;
      runner: string;
    }
  | { type: "apply_update"; to_revision: number }
  | { type: "regenerate" }
  | { type: "export_review" };

export type ServerMsg =
  | { type: "hello_ack"; protocol: number; daemon_version: string; llm: string }
  | {
      type: "snapshot";
      walkthrough: Walkthrough;
      files: FileDiff[];
      scores: Record<string, ImpactScore>;
      review: ReviewState;
    }
  | { type: "update_available"; revision: number; report: ReconcileReport }
  | { type: "generation_progress"; message: string }
  | { type: "review_updated"; review: ReviewState }
  | {
      type: "dispatch_status";
      job_id: string;
      status: "running" | "done" | "failed" | "timed_out_reverted" | "scope_violation";
      detail: string | null;
    }
  | { type: "review_export"; markdown: string }
  | { type: "error"; code: string; message: string };
