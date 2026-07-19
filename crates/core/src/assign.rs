//! Incremental assignment (CLAUDE.md M1): when the tree changes, the
//! existing walkthrough structure PERSISTS. Carried hunks keep their step,
//! changed hunks follow their lineage into the same step, new hunks in a
//! file some step already claims inherit that step, and only true orphans
//! (new files, unclaimed paths) land in an appended "New changes" scope.
//! Existing scope/step ORDER never reshuffles — stability beats optimality.
//!
//! Pure code, not LLM output — coverage and exclusivity hold by
//! construction, so this bypasses the validator gate by design.

use crate::hunk::{Hunk, HunkId};
use crate::reconcile::ReconcileReport;
use crate::schema::{Scope, Step, Walkthrough};
use std::collections::{BTreeMap, BTreeSet};

/// Patch `old` against `report`, producing a walkthrough that covers
/// exactly `new_hunks` while preserving the old structure and order.
pub fn carry_walkthrough(
    old: &Walkthrough,
    report: &ReconcileReport,
    new_hunks: &[Hunk],
    tree_state: &str,
    revision: u64,
) -> Walkthrough {
    let carried: BTreeSet<&HunkId> = report.carried.iter().collect();
    let lineage: BTreeMap<&HunkId, &HunkId> =
        report.changed.iter().map(|l| (&l.from, &l.to)).collect();
    let path_of: BTreeMap<&HunkId, &str> =
        new_hunks.iter().map(|h| (&h.id, h.path.as_str())).collect();

    // 1. Patch existing steps: keep carried, follow lineage, drop removed.
    let mut scopes: Vec<Scope> = old
        .scopes
        .iter()
        .map(|scope| Scope {
            id: scope.id.clone(),
            title: scope.title.clone(),
            steps: scope
                .steps
                .iter()
                .map(|step| Step {
                    id: step.id.clone(),
                    title: step.title.clone(),
                    framing: step.framing.clone(),
                    hunks: step
                        .hunks
                        .iter()
                        .filter_map(|id| {
                            if carried.contains(id) {
                                Some(id.clone())
                            } else {
                                lineage.get(id).map(|to| (*to).clone())
                            }
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect();

    // 2. New hunks whose file a step already claims inherit that step
    //    (first claiming step in walk order). The rest are orphans.
    let mut orphans: Vec<&Hunk> = Vec::new();
    for id in &report.added {
        let Some(hunk) = new_hunks.iter().find(|h| &h.id == id) else { continue };
        let claiming = scopes.iter_mut().flat_map(|s| s.steps.iter_mut()).find(|step| {
            step.hunks.iter().any(|hid| path_of.get(hid) == Some(&hunk.path.as_str()))
        });
        match claiming {
            Some(step) => step.hunks.push(id.clone()),
            None => orphans.push(hunk),
        }
    }

    // 3. Orphans: appended scope, one step per file, fallback-style framing.
    if !orphans.is_empty() {
        let mut by_file: BTreeMap<&str, Vec<&Hunk>> = BTreeMap::new();
        for h in orphans {
            by_file.entry(h.path.as_str()).or_default().push(h);
        }
        let steps = by_file
            .into_iter()
            .map(|(path, hs)| {
                let added: u32 = hs.iter().map(|h| h.added).sum();
                let removed: u32 = hs.iter().map(|h| h.removed).sum();
                Step {
                    id: format!("step:new:{path}"),
                    title: path.to_string(),
                    framing: format!("+{added} -{removed} in {path}"),
                    hunks: hs.iter().map(|h| h.id.clone()).collect(),
                }
            })
            .collect();
        scopes.push(Scope { id: "scope:new-changes".into(), title: "New changes".into(), steps });
    }

    // 4. Prune steps and scopes emptied by removals.
    for scope in &mut scopes {
        scope.steps.retain(|s| !s.hunks.is_empty());
    }
    scopes.retain(|s| !s.steps.is_empty());

    Walkthrough {
        schema_version: old.schema_version,
        revision,
        tree_state: tree_state.to_string(),
        // Focus text describes the preserved majority structure — keep it.
        focus: old.focus.clone(),
        scopes,
        degraded: old.degraded,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hunk::hunk_id;
    use crate::reconcile::Lineage;
    use crate::schema::WALKTHROUGH_SCHEMA_VERSION;

    fn mk(path: &str, body: &str) -> Hunk {
        let lines: Vec<String> = body.lines().map(|s| s.to_string()).collect();
        Hunk {
            id: hunk_id(path, &lines),
            path: path.into(),
            new_start: 1,
            old_start: 1,
            added: lines.len() as u32,
            removed: 0,
            lines,
        }
    }

    fn wt(scopes: Vec<Scope>) -> Walkthrough {
        Walkthrough {
            schema_version: WALKTHROUGH_SCHEMA_VERSION,
            revision: 1,
            tree_state: "t1".into(),
            focus: Some("Order tracks the auth shift.".into()),
            scopes,
            degraded: false,
        }
    }

    fn scope(id: &str, steps: Vec<Step>) -> Scope {
        Scope { id: id.into(), title: id.into(), steps }
    }

    fn step(id: &str, hunks: Vec<HunkId>) -> Step {
        Step { id: id.into(), title: id.into(), framing: "f".into(), hunks }
    }

    #[test]
    fn carried_hunks_keep_their_step_and_order() {
        let a = mk("src/a.rs", "+a");
        let b = mk("src/b.rs", "+b");
        let old = wt(vec![
            scope("s1", vec![step("st1", vec![a.id.clone()])]),
            scope("s2", vec![step("st2", vec![b.id.clone()])]),
        ]);
        let report = ReconcileReport {
            carried: vec![a.id.clone(), b.id.clone()],
            changed: vec![],
            added: vec![],
            removed: vec![],
        };
        let w = carry_walkthrough(&old, &report, &[a.clone(), b.clone()], "t2", 2);
        assert_eq!(w.scopes.len(), 2);
        assert_eq!(w.scopes[0].id, "s1");
        assert_eq!(w.scopes[0].steps[0].hunks, vec![a.id]);
        assert_eq!(w.focus.as_deref(), Some("Order tracks the auth shift."));
        assert_eq!(w.revision, 2);
    }

    #[test]
    fn changed_hunk_follows_lineage_into_same_step() {
        let old_h = mk("src/a.rs", "+old");
        let new_h = mk("src/a.rs", "+new");
        let old = wt(vec![scope("s1", vec![step("st1", vec![old_h.id.clone()])])]);
        let report = ReconcileReport {
            carried: vec![],
            changed: vec![Lineage { from: old_h.id.clone(), to: new_h.id.clone() }],
            added: vec![],
            removed: vec![],
        };
        let w = carry_walkthrough(&old, &report, std::slice::from_ref(&new_h), "t2", 2);
        assert_eq!(w.scopes[0].steps[0].hunks, vec![new_h.id]);
    }

    #[test]
    fn new_hunk_in_claimed_file_inherits_step() {
        let a1 = mk("src/a.rs", "+a1");
        let a2 = mk("src/a.rs", "+a2 fresh");
        let old = wt(vec![scope("s1", vec![step("st1", vec![a1.id.clone()])])]);
        let report = ReconcileReport {
            carried: vec![a1.id.clone()],
            changed: vec![],
            added: vec![a2.id.clone()],
            removed: vec![],
        };
        let w = carry_walkthrough(&old, &report, &[a1.clone(), a2.clone()], "t2", 2);
        assert_eq!(w.scopes.len(), 1, "no orphan scope");
        assert_eq!(w.scopes[0].steps[0].hunks, vec![a1.id, a2.id]);
    }

    #[test]
    fn orphan_new_file_lands_in_appended_scope() {
        let a = mk("src/a.rs", "+a");
        let n = mk("src/brand_new.rs", "+n");
        let old = wt(vec![scope("s1", vec![step("st1", vec![a.id.clone()])])]);
        let report = ReconcileReport {
            carried: vec![a.id.clone()],
            changed: vec![],
            added: vec![n.id.clone()],
            removed: vec![],
        };
        let w = carry_walkthrough(&old, &report, &[a.clone(), n.clone()], "t2", 2);
        assert_eq!(w.scopes.len(), 2);
        assert_eq!(w.scopes[1].title, "New changes");
        assert_eq!(w.scopes[1].steps[0].hunks, vec![n.id]);
        // Existing structure untouched, order preserved.
        assert_eq!(w.scopes[0].id, "s1");
    }

    #[test]
    fn removed_hunks_prune_empty_steps_and_scopes() {
        let a = mk("src/a.rs", "+a");
        let b = mk("src/b.rs", "+b");
        let old = wt(vec![
            scope("s1", vec![step("st1", vec![a.id.clone()])]),
            scope("s2", vec![step("st2", vec![b.id.clone()])]),
        ]);
        let report = ReconcileReport {
            carried: vec![b.id.clone()],
            changed: vec![],
            added: vec![],
            removed: vec![a.id.clone()],
        };
        let w = carry_walkthrough(&old, &report, std::slice::from_ref(&b), "t2", 2);
        assert_eq!(w.scopes.len(), 1);
        assert_eq!(w.scopes[0].id, "s2");
    }

    #[test]
    fn result_covers_exactly_new_hunks() {
        use crate::score::{score_hunk, ExternalSignals};
        use crate::validate::{validate, ValidatorConfig};

        let a1 = mk("src/a.rs", "+a1");
        let a1b = mk("src/a.rs", "+a1 changed");
        let a2 = mk("src/a.rs", "+a2");
        let gone = mk("src/gone.rs", "+gone");
        let fresh = mk("docs/new.md", "+fresh");
        let old = wt(vec![
            scope("s1", vec![step("st1", vec![a1.id.clone()])]),
            scope("s2", vec![step("st2", vec![gone.id.clone()])]),
        ]);
        let report = ReconcileReport {
            carried: vec![],
            changed: vec![Lineage { from: a1.id.clone(), to: a1b.id.clone() }],
            added: vec![a2.id.clone(), fresh.id.clone()],
            removed: vec![gone.id.clone()],
        };
        let new_hunks = vec![a1b.clone(), a2.clone(), fresh.clone()];
        let w = carry_walkthrough(&old, &report, &new_hunks, "t2", 2);

        let known: BTreeMap<HunkId, u32> =
            new_hunks.iter().map(|h| (h.id.clone(), h.added + h.removed)).collect();
        let scores: BTreeMap<_, _> = new_hunks
            .iter()
            .map(|h| (h.id.clone(), score_hunk(h, &ExternalSignals::default())))
            .collect();
        let cfg = ValidatorConfig { max_last_scope_pct: 100, dumping_ground_min_hunks: usize::MAX };
        let coverage = validate(&w, &known, &scores, &cfg);
        assert!(coverage.is_empty(), "{coverage:?}");
    }
}
