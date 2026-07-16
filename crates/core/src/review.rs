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
#[cfg_attr(feature = "ts-export", derive(TS), ts(export))]
pub enum HunkStatus {
    Unviewed,
    Viewed,
    /// Was viewed, then the agent touched it. Re-enters the queue.
    ChangedSinceViewed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(TS), ts(export))]
pub struct Flag {
    pub hunk: HunkId,
    pub comment: String,
    pub open: bool,
    /// Set by reconciliation when the flagged hunk changed after flagging.
    /// Claim only — closing a flag remains a human click.
    pub addressed_claim: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(TS), ts(export))]
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
