//! Reconciliation: the heart of live-sync. Given the previous hunk set and
//! the freshly re-diffed one, classify every hunk and update review state
//! according to the honesty rules. Pure function — the watcher/daemon only
//! feeds it inputs.
//!
//! Matching model (v1):
//! - exact HunkId (path + content hash) match  -> Carried
//! - same path, no id match, overlapping line range -> Changed (lineage)
//! - new id, no lineage                         -> Added
//! - old id absent, not matched as lineage      -> Removed

use crate::hunk::{Hunk, HunkId};
use crate::review::{Flag, HunkStatus, ReviewState};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[cfg(feature = "ts-export")]
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(TS), ts(export))]
pub struct Lineage {
    pub from: HunkId,
    pub to: HunkId,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(TS), ts(export))]
pub struct ReconcileReport {
    pub carried: Vec<HunkId>,
    pub changed: Vec<Lineage>,
    pub added: Vec<HunkId>,
    pub removed: Vec<HunkId>,
}

impl ReconcileReport {
    pub fn is_noop(&self) -> bool {
        self.changed.is_empty() && self.added.is_empty() && self.removed.is_empty()
    }
}

fn ranges_overlap(a: &Hunk, b: &Hunk) -> bool {
    let a_end = a.new_start + a.added.max(1);
    let b_end = b.new_start + b.added.max(1);
    a.new_start <= b_end && b.new_start <= a_end
}

pub fn reconcile(old: &[Hunk], new: &[Hunk]) -> ReconcileReport {
    let new_ids: BTreeSet<&HunkId> = new.iter().map(|h| &h.id).collect();
    let old_ids: BTreeSet<&HunkId> = old.iter().map(|h| &h.id).collect();

    let mut report = ReconcileReport::default();
    let mut claimed_new: BTreeSet<HunkId> = BTreeSet::new();

    // Pass 1: exact carries.
    for h in old {
        if new_ids.contains(&h.id) {
            report.carried.push(h.id.clone());
            claimed_new.insert(h.id.clone());
        }
    }

    // Pass 2: lineage for unmatched old hunks — same path + overlapping range.
    let mut by_path: BTreeMap<&str, Vec<&Hunk>> = BTreeMap::new();
    for h in new {
        if !claimed_new.contains(&h.id) {
            by_path.entry(h.path.as_str()).or_default().push(h);
        }
    }
    for h in old {
        if new_ids.contains(&h.id) {
            continue;
        }
        let candidate = by_path
            .get(h.path.as_str())
            .and_then(|cands| {
                cands
                    .iter()
                    .filter(|c| !claimed_new.contains(&c.id) && ranges_overlap(h, c))
                    .min_by_key(|c| c.new_start.abs_diff(h.new_start))
            })
            .map(|c| c.id.clone());
        match candidate {
            Some(to) => {
                claimed_new.insert(to.clone());
                report.changed.push(Lineage { from: h.id.clone(), to });
            }
            None => report.removed.push(h.id.clone()),
        }
    }

    // Pass 3: everything unclaimed in new is added.
    for h in new {
        if !old_ids.contains(&h.id) && !claimed_new.contains(&h.id) {
            report.added.push(h.id.clone());
        }
    }

    report
}

/// Apply the honesty rules to review state. Returns hunks that re-entered
/// the queue (for UI highlighting).
pub fn apply_to_review(state: &mut ReviewState, report: &ReconcileReport) -> Vec<HunkId> {
    let mut requeued = Vec::new();

    for lin in &report.changed {
        // Viewed -> ChangedSinceViewed under the NEW id.
        if state.status_of(&lin.from) == HunkStatus::Viewed {
            state.status.remove(&lin.from);
            state.status.insert(lin.to.clone(), HunkStatus::ChangedSinceViewed);
            requeued.push(lin.to.clone());
        }
        // Flags migrate to the new id and gain an addressed CLAIM.
        for f in state.flags.iter_mut() {
            if f.hunk == lin.from && f.open {
                f.hunk = lin.to.clone();
                f.addressed_claim = true;
            }
        }
    }

    for removed in &report.removed {
        state.status.remove(removed);
        // Open flags on deleted hunks become tombstones — preserved.
        let (dead, alive): (Vec<Flag>, Vec<Flag>) =
            state.flags.drain(..).partition(|f| &f.hunk == removed && f.open);
        state.flags = alive;
        state.tombstones.extend(dead);
    }

    requeued
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hunk::hunk_id;

    fn mk(path: &str, start: u32, body: &str) -> Hunk {
        let lines: Vec<String> = body.lines().map(|s| s.to_string()).collect();
        Hunk {
            id: hunk_id(path, &lines),
            path: path.into(),
            new_start: start,
            old_start: start,
            added: lines.iter().filter(|l| l.starts_with('+')).count() as u32,
            removed: 0,
            lines,
        }
    }

    #[test]
    fn identical_sets_are_noop() {
        let a = vec![mk("a.ts", 1, "+x"), mk("b.ts", 5, "+y")];
        let r = reconcile(&a, &a);
        assert!(r.is_noop());
        assert_eq!(r.carried.len(), 2);
    }

    #[test]
    fn modified_hunk_gets_lineage_not_add_remove() {
        let old = vec![mk("a.ts", 10, "+old body")];
        let new = vec![mk("a.ts", 11, "+new body")];
        let r = reconcile(&old, &new);
        assert_eq!(r.changed.len(), 1);
        assert!(r.added.is_empty());
        assert!(r.removed.is_empty());
    }

    #[test]
    fn viewed_downgrades_on_change() {
        let old = vec![mk("a.ts", 10, "+old")];
        let new = vec![mk("a.ts", 10, "+new")];
        let r = reconcile(&old, &new);

        let mut state = ReviewState::default();
        state.mark_viewed(old[0].id.clone());
        let requeued = apply_to_review(&mut state, &r);

        assert_eq!(requeued.len(), 1);
        assert_eq!(state.status_of(&new[0].id), HunkStatus::ChangedSinceViewed);
    }

    #[test]
    fn flag_on_deleted_hunk_becomes_tombstone() {
        let old = vec![mk("a.ts", 10, "+doomed")];
        let new: Vec<Hunk> = vec![];
        let r = reconcile(&old, &new);

        let mut state = ReviewState::default();
        state.flags.push(Flag {
            hunk: old[0].id.clone(),
            comment: "why is this here".into(),
            open: true,
            addressed_claim: false,
        });
        apply_to_review(&mut state, &r);
        assert!(state.flags.is_empty());
        assert_eq!(state.tombstones.len(), 1);
    }

    #[test]
    fn flag_migrates_and_claims_addressed() {
        let old = vec![mk("a.ts", 10, "+bad fetch")];
        let new = vec![mk("a.ts", 10, "+good fetch")];
        let r = reconcile(&old, &new);

        let mut state = ReviewState::default();
        state.flags.push(Flag {
            hunk: old[0].id.clone(),
            comment: "make it server-side".into(),
            open: true,
            addressed_claim: false,
        });
        apply_to_review(&mut state, &r);
        assert_eq!(state.flags[0].hunk, new[0].id);
        assert!(state.flags[0].addressed_claim);
        assert!(state.flags[0].open, "closing remains a human click");
    }

    #[test]
    fn unrelated_paths_never_lineage() {
        let old = vec![mk("a.ts", 10, "+x")];
        let new = vec![mk("b.ts", 10, "+y")];
        let r = reconcile(&old, &new);
        assert_eq!(r.removed.len(), 1);
        assert_eq!(r.added.len(), 1);
        assert!(r.changed.is_empty());
    }
}
