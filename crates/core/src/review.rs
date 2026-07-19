//! Review state. Keys off HunkId (content hash) so it survives any reflow.
//! The honesty rule lives here: a hunk that changed after being viewed is
//! NEVER silently kept as viewed.

use crate::hunk::HunkId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[cfg(feature = "ts-export")]
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts-export", derive(TS))]
pub enum HunkStatus {
    Unviewed,
    Viewed,
    /// Was viewed, then the agent touched it. Re-enters the queue.
    ChangedSinceViewed,
}

/// Who authored a line in a flag's thread. There is deliberately NO
/// "verdict" kind: the machine reports what it did, never whether the code
/// is good. Judgment is the human's, always (CLAUDE.md, the one rule).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts-export", derive(TS))]
pub enum FlagEntryKind {
    /// A person typed this.
    HumanComment,
    /// Agent answered or explained without changing the working tree.
    AgentResponse,
    /// An agent's summary of what it changed — a CLAIM, not a verdict and
    /// not trusted on its word. Reconciliation independently confirms the
    /// hunk actually moved; the human still closes the flag.
    AgentClaim,
    /// Runner lifecycle note (timed out + reverted, out-of-scope files, …).
    DispatchNote,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(TS))]
pub struct FlagEntry {
    pub kind: FlagEntryKind,
    pub body: String,
    /// Walkthrough revision this entry was recorded against.
    #[cfg_attr(feature = "ts-export", ts(type = "number"))]
    pub revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(TS))]
pub struct Flag {
    pub hunk: HunkId,
    /// Line this thread anchors to — an index into the hunk's raw lines
    /// (GitHub per-line comments). None = hunk-level. A render offset only;
    /// the flag's identity remains the hunk hash (invariant 2).
    #[serde(default)]
    pub line: Option<u32>,
    /// The conversation on this flag, oldest first: the human's comment,
    /// agent change-claims, dispatch notes. Migrates as a unit when the
    /// hunk's id changes (reconcile), so the history is never orphaned.
    pub thread: Vec<FlagEntry>,
    pub open: bool,
    /// Set by reconciliation when the flagged hunk changed after flagging.
    /// Claim only — closing a flag remains a human click.
    pub addressed_claim: bool,
}

impl Flag {
    /// Open a flag, seeded with the human's comment as the first entry.
    pub fn new(hunk: HunkId, line: Option<u32>, comment: String) -> Self {
        Self {
            hunk,
            line,
            thread: vec![FlagEntry {
                kind: FlagEntryKind::HumanComment,
                body: comment,
                revision: 0,
            }],
            open: true,
            addressed_claim: false,
        }
    }

    /// The flag's headline — its first human comment. Used in exports.
    pub fn headline(&self) -> &str {
        self.thread
            .iter()
            .find(|e| e.kind == FlagEntryKind::HumanComment)
            .map(|e| e.body.as_str())
            .unwrap_or("")
    }

    /// Append a thread entry (agent claim, dispatch note, follow-up comment).
    pub fn push(&mut self, kind: FlagEntryKind, body: String, revision: u64) {
        self.thread.push(FlagEntry { kind, body, revision });
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(TS))]
pub struct ReviewState {
    pub status: BTreeMap<HunkId, HunkStatus>,
    pub flags: Vec<Flag>,
    /// Comments on hunks that were later deleted — preserved, not dropped:
    /// "I flagged this and the agent deleted it" is itself information.
    pub tombstones: Vec<Flag>,
}

impl ReviewState {
    pub fn status_of(&self, id: &HunkId) -> HunkStatus {
        *self.status.get(id).unwrap_or(&HunkStatus::Unviewed)
    }

    pub fn mark_viewed(&mut self, id: HunkId) {
        self.status.insert(id, HunkStatus::Viewed);
    }

    pub fn open_flags(&self) -> impl Iterator<Item = &Flag> {
        self.flags.iter().filter(|f| f.open)
    }

    pub fn all_viewed_and_clean(&self, all: &[&HunkId]) -> bool {
        all.iter().all(|id| self.status_of(id) == HunkStatus::Viewed)
            && self.open_flags().count() == 0
    }
}
