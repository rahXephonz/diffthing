//! Walkthrough schema. This is the product's real API — the validator,
//! the UI, the export format, and reconciliation all hang off these types.
//! Version it like a wire protocol, because it is one.

use crate::hunk::HunkId;
use serde::{Deserialize, Serialize};

#[cfg(feature = "ts-export")]
use ts_rs::TS;

pub const WALKTHROUGH_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts-export", derive(TS))]
pub enum Impact {
    Low,
    Medium,
    High,
    Highest,
}

/// Deterministic score attached to every hunk. `reasons` is user-facing:
/// "highest — exported signature changed, 23 importers, payment path".
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(TS))]
pub struct ImpactScore {
    pub impact: Impact,
    pub points: u32,
    pub reasons: Vec<String>,
}

/// A step: the atomic reading unit. References hunks by id — never contains
/// diff content itself (single source of truth is the hunk store).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(TS))]
pub struct Step {
    pub id: String,
    pub title: String,
    /// One-line *description* of what changed. Never an evaluation.
    pub framing: String,
    pub hunks: Vec<HunkId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(TS))]
pub struct Scope {
    pub id: String,
    pub title: String,
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(TS))]
pub struct Walkthrough {
    pub schema_version: u16,
    /// Monotonic version within a session; bumped by every applied update.
    #[cfg_attr(feature = "ts-export", ts(type = "number"))]
    pub revision: u64,
    /// Working-tree state this was generated against (git rev + dirty hash).
    pub tree_state: String,
    /// "Review focus": 1-2 sentences describing the reading order's logic.
    /// LLM-authored, validator-gated with the rest; None in fallback mode.
    /// A *description* of the walk, never an evaluation of the code.
    #[serde(default)]
    pub focus: Option<String>,
    pub scopes: Vec<Scope>,
    /// True when the LLM was unavailable/failed validation and we fell back
    /// to the deterministic file-order walkthrough. Shown honestly in UI.
    pub degraded: bool,
}

impl Walkthrough {
    pub fn all_hunk_ids(&self) -> Vec<&HunkId> {
        self.scopes.iter().flat_map(|s| s.steps.iter()).flat_map(|st| st.hunks.iter()).collect()
    }
}
