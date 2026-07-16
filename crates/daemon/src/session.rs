//! Session: the daemon-side source of truth. The browser is stateless —
//! it renders snapshots and events; a crashed tab loses nothing.
//!
//! Update rule (load-bearing UX): reconciliation runs continuously in the
//! background, but the served snapshot only advances when the client sends
//! ApplyUpdate. The screen never moves under the reader's cursor.

use crate::{gitio, llm};
use diffthing_analyzers::Registry;
use diffthing_core::hunk::{FileDiff, Hunk, HunkId};
use diffthing_core::protocol::ServerMsg;
use diffthing_core::reconcile::{apply_to_review, reconcile, ReconcileReport};
use diffthing_core::review::ReviewState;
use diffthing_core::schema::{ImpactScore, Walkthrough};
use diffthing_core::score::score_hunk;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tokio::sync::{broadcast, Mutex};

pub struct Snapshot {
    pub walkthrough: Walkthrough,
    pub files: Vec<FileDiff>,
    pub scores: BTreeMap<HunkId, ImpactScore>,
    pub review: ReviewState,
}

pub struct Pending {
    pub revision: u64,
    pub files: Vec<FileDiff>,
    pub report: ReconcileReport,
}

pub struct Session {
    pub repo: PathBuf,
    pub base: String,
    pub token: String,
    pub state: Mutex<Snapshot>,
    pub pending: Mutex<Option<Pending>>,
    pub events: broadcast::Sender<ServerMsg>,
    registry: Registry,
}

fn all_hunks(files: &[FileDiff]) -> Vec<Hunk> {
    files.iter().flat_map(|f| f.hunks.iter().cloned()).collect()
}

impl Session {
    pub async fn boot(
        repo: &Path,
        base: &str,
        token: String,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let registry = Registry::with_defaults(repo);
        let files = gitio::diff_against(repo, base).await?;
        let hunks = all_hunks(&files);
        let scores: BTreeMap<HunkId, ImpactScore> = hunks
            .iter()
            .map(|h| (h.id.clone(), score_hunk(h, &registry.signals_for(h))))
            .collect();
        let tree_state = gitio::tree_state(repo, base).await?;
        let walkthrough =
            llm::generate(&llm::NoopLlm, &hunks, &scores, &tree_state, 1, 2).await;

        let (events, _) = broadcast::channel(64);
        Ok(Self {
            repo: repo.to_path_buf(),
            base: base.to_string(),
            token,
            state: Mutex::new(Snapshot {
                walkthrough,
                files,
                scores,
                review: ReviewState::default(),
            }),
            pending: Mutex::new(None),
            events,
            registry,
        })
    }

    /// Called by the watcher after quiescence. Computes the update in the
    /// background and announces it — never mutates the served snapshot.
    pub async fn on_fs_quiescence(&self) {
        let Ok(new_files) = gitio::diff_against(&self.repo, &self.base).await else {
            return;
        };
        let new_hunks = all_hunks(&new_files);
        let report = {
            let state = self.state.lock().await;
            let old_hunks = all_hunks(&state.files);
            reconcile(&old_hunks, &new_hunks)
        };
        if report.is_noop() {
            return;
        }
        let revision = {
            let state = self.state.lock().await;
            state.walkthrough.revision + 1
        };
        *self.pending.lock().await = Some(Pending {
            revision,
            files: new_files,
            report: report.clone(),
        });
        let _ = self
            .events
            .send(ServerMsg::UpdateAvailable { revision, report });
    }

    /// User clicked Apply. NOW the snapshot advances: review-state honesty
    /// rules run, incremental assignment keeps existing step order stable
    /// (deterministic pass; LLM-for-orphans is an M1 refinement).
    pub async fn apply_update(&self, to_revision: u64) -> bool {
        let Some(pending) = self.pending.lock().await.take() else {
            return false;
        };
        if pending.revision != to_revision {
            return false;
        }
        let mut state = self.state.lock().await;
        apply_to_review(&mut state.review, &pending.report);

        let hunks = all_hunks(&pending.files);
        let scores: BTreeMap<HunkId, ImpactScore> = hunks
            .iter()
            .map(|h| (h.id.clone(), score_hunk(h, &self.registry.signals_for(h))))
            .collect();
        let tree_state = gitio::tree_state(&self.repo, &self.base)
            .await
            .unwrap_or_else(|_| "unknown".into());
        // Scaffold: regenerate via the gated pipeline (fallback today).
        // M1: replace with incremental assignment that preserves step order.
        let walkthrough = llm::generate(
            &llm::NoopLlm,
            &hunks,
            &scores,
            &tree_state,
            pending.revision,
            2,
        )
        .await;

        state.files = pending.files;
        state.scores = scores;
        state.walkthrough = walkthrough;
        true
    }

    pub async fn export_markdown(&self) -> String {
        let state = self.state.lock().await;
        let mut out = String::from("## diffthing review export\n\n");
        for f in state.review.open_flags() {
            if let Some(h) = state
                .files
                .iter()
                .flat_map(|fd| fd.hunks.iter())
                .find(|h| h.id == f.hunk)
            {
                out.push_str(&format!(
                    "### {} (line {})\nhunk: `{}`\n\n> {}\n\n```diff\n{}\n```\n\n",
                    h.path,
                    h.new_start,
                    h.id.0,
                    f.comment,
                    h.lines.join("\n")
                ));
            }
        }
        out.push_str("Address only the flags above. Do not refactor, reformat, or touch unrelated files.\n");
        out
    }
}
