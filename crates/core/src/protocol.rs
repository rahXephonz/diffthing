//! WebSocket wire protocol between daemon and SPA.
//! Message #1 in each direction is the version handshake — the hosted SPA
//! updates instantly, installed daemons don't, so skew is a permanent state
//! to handle, not an error to hide.

use crate::hunk::{FileDiff, HunkId};
use crate::reconcile::ReconcileReport;
use crate::review::ReviewState;
use crate::schema::{ImpactScore, Walkthrough};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[cfg(feature = "ts-export")]
use ts_rs::TS;

pub const PROTOCOL_VERSION: u16 = 5;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "ts-export", derive(TS))]
pub enum ClientMsg {
    /// Must be the first message. Token echoes the URL fragment value.
    Hello {
        protocol: u16,
        token: String,
    },
    MarkViewed {
        hunk: HunkId,
    },
    AddFlag {
        hunk: HunkId,
        /// Index into the hunk's raw lines this comment anchors to, GitHub
        /// per-line style. None = a hunk-level comment. Identity is still the
        /// hunk (invariant 2); this is only a position within it.
        #[serde(default)]
        line: Option<u32>,
        comment: String,
    },
    CloseFlag {
        hunk: HunkId,
        #[serde(default)]
        line: Option<u32>,
    },
    /// Anchored prompt dispatch — always attached to hunks, never free-form.
    RequestChange {
        hunks: Vec<HunkId>,
        /// Review thread anchor being dispatched. Claims return only to this
        /// thread, never every open flag sharing the hunk.
        #[serde(default)]
        line: Option<u32>,
        instruction: String,
        runner: String,
    },
    ApplyUpdate {
        #[cfg_attr(feature = "ts-export", ts(type = "number"))]
        to_revision: u64,
    },
    Regenerate,
    ExportReview,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "ts-export", derive(TS))]
pub enum ServerMsg {
    HelloAck {
        protocol: u16,
        daemon_version: String,
        /// Human label for the walkthrough organizer, e.g. "claude (your
        /// login)" or "none (deterministic fallback)".
        llm: String,
    },
    /// Full state on connect / after ApplyUpdate. Browser holds no durable state.
    Snapshot {
        walkthrough: Walkthrough,
        files: Vec<FileDiff>,
        scores: BTreeMap<HunkId, ImpactScore>,
        review: ReviewState,
    },
    /// Background reconciliation result — UI shows the apply banner,
    /// never reflows on its own.
    UpdateAvailable {
        #[cfg_attr(feature = "ts-export", ts(type = "number"))]
        revision: u64,
        report: ReconcileReport,
    },
    GenerationProgress {
        message: String,
    },
    /// Review state changed (comment added, flag closed, hunk viewed) without
    /// the diff moving. Pushed so every tab reflects it immediately — the
    /// browser holds no durable state, so a mutation the daemon doesn't echo
    /// is invisible until reconnect. Lighter than a full Snapshot.
    ReviewUpdated {
        review: ReviewState,
    },
    DispatchStatus {
        job_id: String,
        status: JobStatus,
        /// Human-readable note: the agent's change summary on Done, the
        /// revert reason on failure, the out-of-scope files on
        /// ScopeViolation. None while merely Running.
        detail: Option<String>,
    },
    ReviewExport {
        markdown: String,
    },
    Error {
        code: ErrorCode,
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts-export", derive(TS))]
pub enum JobStatus {
    Running,
    Done,
    Failed,
    TimedOutReverted,
    /// Agent touched files outside the anchored set — surfaced, not hidden.
    ScopeViolation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts-export", derive(TS))]
pub enum ErrorCode {
    BadToken,
    ProtocolMismatch,
    BusyWriterLock,
    Internal,
}
