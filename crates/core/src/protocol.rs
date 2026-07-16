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

pub const PROTOCOL_VERSION: u16 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "ts-export", derive(TS), ts(export))]
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
        comment: String,
    },
    CloseFlag {
        hunk: HunkId,
    },
    /// Anchored prompt dispatch — always attached to hunks, never free-form.
    RequestChange {
        hunks: Vec<HunkId>,
        instruction: String,
        runner: String,
    },
    ApplyUpdate {
        to_revision: u64,
    },
    Regenerate,
    ExportReview,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "ts-export", derive(TS), ts(export))]
pub enum ServerMsg {
    HelloAck {
        protocol: u16,
        daemon_version: String,
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
        revision: u64,
        report: ReconcileReport,
    },
    GenerationProgress {
        message: String,
    },
    DispatchStatus {
        job_id: String,
        status: JobStatus,
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
#[cfg_attr(feature = "ts-export", derive(TS), ts(export))]
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
#[cfg_attr(feature = "ts-export", derive(TS), ts(export))]
pub enum ErrorCode {
    BadToken,
    ProtocolMismatch,
    BusyWriterLock,
    Internal,
}
