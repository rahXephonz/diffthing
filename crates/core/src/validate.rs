//! The validator gate. The LLM proposes; this verifies. A walkthrough that
//! fails here is never shown — the daemon regenerates with the violation as
//! feedback, then falls back to the deterministic walkthrough after N tries.

use crate::hunk::HunkId;
use crate::schema::{Impact, ImpactScore, Walkthrough};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum Violation {
    #[error("hunk {0:?} is not covered by any step")]
    Uncovered(HunkId),
    #[error("hunk {0:?} is assigned to more than one step")]
    DuplicateAssignment(HunkId),
    #[error("step references unknown hunk {0:?} (hallucinated)")]
    UnknownHunk(HunkId),
    #[error("last scope ('{scope}') holds {pct}% of changed lines (max {max}%)")]
    DumpingGround { scope: String, pct: u32, max: u32 },
    #[error("highest-impact hunk {0:?} is buried in the final scope")]
    BuriedHighest(HunkId),
    #[error("walkthrough has no scopes")]
    Empty,
}

pub struct ValidatorConfig {
    /// Max share of total changed lines allowed in the final scope.
    pub max_last_scope_pct: u32,
    /// Below this many total hunks the dumping-ground rule is skipped
    /// (tiny diffs legitimately fit in one scope).
    pub dumping_ground_min_hunks: usize,
}

impl Default for ValidatorConfig {
    fn default() -> Self {
        Self { max_last_scope_pct: 40, dumping_ground_min_hunks: 8 }
    }
}

pub fn validate(
    w: &Walkthrough,
    known_hunks: &BTreeMap<HunkId, u32>, // id -> changed line count
    scores: &BTreeMap<HunkId, ImpactScore>,
    cfg: &ValidatorConfig,
) -> Vec<Violation> {
    let mut violations = Vec::new();

    if w.scopes.is_empty() {
        violations.push(Violation::Empty);
        return violations;
    }

    // Coverage + exclusivity + hallucination.
    let mut seen: BTreeSet<&HunkId> = BTreeSet::new();
    for id in w.all_hunk_ids() {
        if !known_hunks.contains_key(id) {
            violations.push(Violation::UnknownHunk(id.clone()));
        }
        if !seen.insert(id) {
            violations.push(Violation::DuplicateAssignment(id.clone()));
        }
    }
    for id in known_hunks.keys() {
        if !seen.contains(id) {
            violations.push(Violation::Uncovered(id.clone()));
        }
    }

    // Dumping ground: the final scope must not swallow the diff.
    if known_hunks.len() >= cfg.dumping_ground_min_hunks {
        let total: u64 = known_hunks.values().map(|&n| n as u64).sum();
        if let Some(last) = w.scopes.last() {
            let last_lines: u64 = last
                .steps
                .iter()
                .flat_map(|s| s.hunks.iter())
                .filter_map(|id| known_hunks.get(id))
                .map(|&n| n as u64)
                .sum();
            if total > 0 {
                let pct = ((last_lines * 100) / total) as u32;
                if pct > cfg.max_last_scope_pct {
                    violations.push(Violation::DumpingGround {
                        scope: last.title.clone(),
                        pct,
                        max: cfg.max_last_scope_pct,
                    });
                }
            }
        }
    }

    // A `highest` hunk must never sit in the final scope (when >1 scope).
    if w.scopes.len() > 1 {
        if let Some(last) = w.scopes.last() {
            for id in last.steps.iter().flat_map(|s| s.hunks.iter()) {
                if scores.get(id).map(|s| s.impact) == Some(Impact::Highest) {
                    violations.push(Violation::BuriedHighest(id.clone()));
                }
            }
        }
    }

    violations
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{Scope, Step, WALKTHROUGH_SCHEMA_VERSION};

    fn id(s: &str) -> HunkId {
        HunkId(s.into())
    }

    fn wt(scopes: Vec<Scope>) -> Walkthrough {
        Walkthrough {
            schema_version: WALKTHROUGH_SCHEMA_VERSION,
            revision: 1,
            tree_state: "test".into(),
            scopes,
            degraded: false,
        }
    }

    fn step(sid: &str, hunks: Vec<HunkId>) -> Step {
        Step { id: sid.into(), title: sid.into(), framing: String::new(), hunks }
    }

    fn score(imp: Impact) -> ImpactScore {
        ImpactScore { impact: imp, points: 0, reasons: vec![] }
    }

    #[test]
    fn full_coverage_passes() {
        let known: BTreeMap<HunkId, u32> =
            [(id("a"), 10), (id("b"), 5)].into_iter().collect();
        let w = wt(vec![Scope {
            id: "s1".into(),
            title: "s1".into(),
            steps: vec![step("1", vec![id("a"), id("b")])],
        }]);
        let v = validate(&w, &known, &BTreeMap::new(), &ValidatorConfig::default());
        assert!(v.is_empty(), "{v:?}");
    }

    #[test]
    fn uncovered_and_hallucinated_detected() {
        let known: BTreeMap<HunkId, u32> = [(id("a"), 10)].into_iter().collect();
        let w = wt(vec![Scope {
            id: "s1".into(),
            title: "s1".into(),
            steps: vec![step("1", vec![id("ghost")])],
        }]);
        let v = validate(&w, &known, &BTreeMap::new(), &ValidatorConfig::default());
        assert!(v.contains(&Violation::UnknownHunk(id("ghost"))));
        assert!(v.contains(&Violation::Uncovered(id("a"))));
    }

    #[test]
    fn duplicate_assignment_detected() {
        let known: BTreeMap<HunkId, u32> = [(id("a"), 10)].into_iter().collect();
        let w = wt(vec![Scope {
            id: "s1".into(),
            title: "s1".into(),
            steps: vec![step("1", vec![id("a")]), step("2", vec![id("a")])],
        }]);
        let v = validate(&w, &known, &BTreeMap::new(), &ValidatorConfig::default());
        assert!(v.contains(&Violation::DuplicateAssignment(id("a"))));
    }

    #[test]
    fn dumping_ground_detected() {
        // 10 hunks, last scope holds 90% of lines.
        let mut known = BTreeMap::new();
        let mut first = Vec::new();
        let mut last = Vec::new();
        for i in 0..10 {
            let h = id(&format!("h{i}"));
            if i == 0 {
                known.insert(h.clone(), 10);
                first.push(h);
            } else {
                known.insert(h.clone(), 100);
                last.push(h);
            }
        }
        let w = wt(vec![
            Scope { id: "s1".into(), title: "core".into(), steps: vec![step("1", first)] },
            Scope { id: "s2".into(), title: "other changes".into(), steps: vec![step("2", last)] },
        ]);
        let v = validate(&w, &known, &BTreeMap::new(), &ValidatorConfig::default());
        assert!(matches!(v[0], Violation::DumpingGround { .. }), "{v:?}");
    }

    #[test]
    fn buried_highest_detected() {
        let known: BTreeMap<HunkId, u32> =
            [(id("a"), 10), (id("hot"), 2)].into_iter().collect();
        let scores: BTreeMap<HunkId, ImpactScore> =
            [(id("hot"), score(Impact::Highest))].into_iter().collect();
        let w = wt(vec![
            Scope { id: "s1".into(), title: "main".into(), steps: vec![step("1", vec![id("a")])] },
            Scope { id: "s2".into(), title: "support".into(), steps: vec![step("2", vec![id("hot")])] },
        ]);
        let v = validate(&w, &known, &scores, &ValidatorConfig::default());
        assert!(v.contains(&Violation::BuriedHighest(id("hot"))));
    }
}
