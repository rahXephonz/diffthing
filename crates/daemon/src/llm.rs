//! LLM boundary. The single narrow job: propose walkthrough structure from
//! hunk DIGESTS (never full bodies for grouping — shape, not content).
//! Output is serde-deserialized and validator-gated; failure regenerates
//! with violations as feedback, then falls back deterministically.
//!
//! v1 scaffold ships `NoopLlm` so the whole loop runs end-to-end with zero
//! keys — the fallback walkthrough IS the product in degraded mode.
//! Real providers (Anthropic/OpenAI/local) land behind this same trait via
//! reqwest + structured output. See CLAUDE.md M1.

use diffthing_core::hunk::{Hunk, HunkId};
use diffthing_core::schema::{ImpactScore, Walkthrough};
use serde::Serialize;
use std::collections::BTreeMap;

/// What the model sees per hunk. Deliberately compact: path, symbols-ish
/// first line, counts, score. ~10x cheaper than bodies and makes huge
/// diffs possible rather than lossy.
#[derive(Debug, Serialize)]
pub struct HunkDigest<'a> {
    pub id: &'a HunkId,
    pub path: &'a str,
    pub added: u32,
    pub removed: u32,
    pub impact: &'a ImpactScore,
    pub head: &'a str,
}

pub fn digest<'a>(h: &'a Hunk, score: &'a ImpactScore) -> HunkDigest<'a> {
    HunkDigest {
        id: &h.id,
        path: &h.path,
        added: h.added,
        removed: h.removed,
        impact: score,
        head: h.lines.first().map(|s| s.as_str()).unwrap_or(""),
    }
}

#[allow(async_fn_in_trait)]
pub trait LlmClient: Send + Sync {
    /// Returns None when unavailable — caller falls back deterministically.
    async fn propose_walkthrough(
        &self,
        digests: &[HunkDigest<'_>],
        prior_violations: &[String],
    ) -> Option<Walkthrough>;
}

/// Zero-key scaffold client: always defers to the fallback.
pub struct NoopLlm;

impl LlmClient for NoopLlm {
    async fn propose_walkthrough(
        &self,
        _digests: &[HunkDigest<'_>],
        _prior_violations: &[String],
    ) -> Option<Walkthrough> {
        None
    }
}

/// Shared generation pipeline: LLM proposal -> validator gate -> retry with
/// feedback -> deterministic fallback. This function is the philosophy in
/// code: the LLM proposes, code verifies, the user never sees an
/// unvalidated walkthrough.
pub async fn generate<L: LlmClient>(
    llm: &L,
    hunks: &[Hunk],
    scores: &BTreeMap<HunkId, ImpactScore>,
    tree_state: &str,
    revision: u64,
    max_retries: usize,
) -> Walkthrough {
    use diffthing_core::validate::{validate, ValidatorConfig};

    let known: BTreeMap<HunkId, u32> = hunks
        .iter()
        .map(|h| (h.id.clone(), h.added + h.removed))
        .collect();
    let digests: Vec<HunkDigest> = hunks
        .iter()
        .filter_map(|h| scores.get(&h.id).map(|s| digest(h, s)))
        .collect();

    let cfg = ValidatorConfig::default();
    let mut violations_feedback: Vec<String> = Vec::new();

    for _ in 0..=max_retries {
        match llm.propose_walkthrough(&digests, &violations_feedback).await {
            Some(mut w) => {
                let violations = validate(&w, &known, scores, &cfg);
                if violations.is_empty() {
                    w.revision = revision;
                    w.tree_state = tree_state.to_string();
                    w.degraded = false;
                    return w;
                }
                violations_feedback = violations.iter().map(|v| v.to_string()).collect();
            }
            None => break,
        }
    }

    diffthing_core::fallback::build_fallback(hunks, scores, tree_state, revision)
}
