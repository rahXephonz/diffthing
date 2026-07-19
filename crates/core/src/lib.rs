//! diffthing-core: pure, deterministic logic. No IO, no async, no LLM.
//!
//! Invariant of the whole product: the LLM proposes, this crate verifies.
//! Everything here must stay unit-testable to death.

pub mod assign;
pub mod fallback;
pub mod hunk;
pub mod protocol;
pub mod reconcile;
pub mod review;
pub mod schema;
pub mod score;
pub mod validate;

pub use hunk::{FileDiff, Hunk, HunkId};
pub use schema::{Impact, Scope, Step, Walkthrough};
