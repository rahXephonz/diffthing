//! Deterministic fallback walkthrough: used when the LLM is unavailable or
//! fails validation N times. Also serves as the honest baseline the LLM
//! version must beat. Groups by top-level directory, orders scopes by the
//! max impact inside them, steps by file.

use crate::hunk::Hunk;
use crate::schema::{Impact, ImpactScore, Scope, Step, Walkthrough, WALKTHROUGH_SCHEMA_VERSION};
use std::collections::BTreeMap;

fn top_dir(path: &str) -> String {
    match path.split('/').next() {
        Some(d) if d != path => d.to_string(),
        _ => "root".to_string(),
    }
}

pub fn build_fallback(
    hunks: &[Hunk],
    scores: &BTreeMap<crate::hunk::HunkId, ImpactScore>,
    tree_state: &str,
    revision: u64,
) -> Walkthrough {
    // group: dir -> file -> hunks
    let mut groups: BTreeMap<String, BTreeMap<String, Vec<&Hunk>>> = BTreeMap::new();
    for h in hunks {
        groups.entry(top_dir(&h.path)).or_default().entry(h.path.clone()).or_default().push(h);
    }

    let mut scopes: Vec<(Impact, Scope)> = Vec::new();
    for (dir, files) in groups {
        let mut steps = Vec::new();
        let mut max_impact = Impact::Low;
        for (file, hs) in files {
            let added: u32 = hs.iter().map(|h| h.added).sum();
            let removed: u32 = hs.iter().map(|h| h.removed).sum();
            for h in &hs {
                if let Some(s) = scores.get(&h.id) {
                    if s.impact > max_impact {
                        max_impact = s.impact;
                    }
                }
            }
            steps.push(Step {
                id: format!("step:{file}"),
                title: file.clone(),
                framing: format!("+{added} -{removed} in {file}"),
                hunks: hs.iter().map(|h| h.id.clone()).collect(),
            });
        }
        scopes.push((max_impact, Scope { id: format!("scope:{dir}"), title: dir, steps }));
    }
    // Highest-impact scopes first — even degraded mode respects reading order.
    scopes.sort_by_key(|(impact, _)| std::cmp::Reverse(*impact));

    Walkthrough {
        schema_version: WALKTHROUGH_SCHEMA_VERSION,
        revision,
        tree_state: tree_state.to_string(),
        scopes: scopes.into_iter().map(|(_, s)| s).collect(),
        degraded: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hunk::hunk_id;
    use crate::score::{score_hunk, ExternalSignals};
    use crate::validate::{validate, ValidatorConfig};

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

    #[test]
    fn fallback_always_passes_its_own_validator() {
        let hunks = vec![
            mk("src/auth/login.ts", "+if (!ok) return"),
            mk("src/ui/Button.tsx", "+<button/>"),
            mk("docs/readme.md", "+hello"),
        ];
        let scores: BTreeMap<_, _> = hunks
            .iter()
            .map(|h| (h.id.clone(), score_hunk(h, &ExternalSignals::default())))
            .collect();
        let known: BTreeMap<_, _> =
            hunks.iter().map(|h| (h.id.clone(), h.added + h.removed)).collect();
        let w = build_fallback(&hunks, &scores, "t", 1);
        let v = validate(&w, &known, &scores, &ValidatorConfig::default());
        assert!(v.is_empty(), "{v:?}");
        assert!(w.degraded);
    }

    #[test]
    fn higher_impact_scope_comes_first() {
        let hunks =
            vec![mk("docs/notes.md", "+meh"), mk("src/payment/charge.ts", "+if (x) throw e")];
        let scores: BTreeMap<_, _> = hunks
            .iter()
            .map(|h| (h.id.clone(), score_hunk(h, &ExternalSignals::default())))
            .collect();
        let w = build_fallback(&hunks, &scores, "t", 1);
        assert_eq!(w.scopes[0].title, "src");
    }
}
