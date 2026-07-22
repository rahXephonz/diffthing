//! Session: the daemon-side source of truth. The browser is stateless —
//! it renders snapshots and events; a crashed tab loses nothing.
//!
//! Update rule (load-bearing UX): reconciliation runs continuously in the
//! background, but the served snapshot only advances when the client sends
//! ApplyUpdate. The screen never moves under the reader's cursor.

use crate::llm::AnyLlm;
use crate::store::Store;
use crate::{gitio, llm};
use diffthing_analyzers::Registry;
use diffthing_core::hunk::{FileDiff, Hunk, HunkId};
use diffthing_core::protocol::ServerMsg;
use diffthing_core::reconcile::{apply_to_review, reconcile, ReconcileReport};
use diffthing_core::review::ReviewState;
use diffthing_core::schema::{ImpactScore, Walkthrough};
use diffthing_core::score::score_hunk;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
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
    /// Single-writer lock for agent dispatch: only one runner may edit the
    /// working tree at a time. `try_lock` fails fast with BusyWriterLock
    /// rather than queueing — concurrent agents on one tree is not a thing.
    pub writer: Arc<Mutex<()>>,
    /// One background LLM organization run at a time. A Regenerate flood
    /// must not spawn one agent CLI process per click — extra requests are
    /// dropped while a run is in flight (DoS hardening).
    organizing: std::sync::atomic::AtomicBool,
    registry: Registry,
    llm: AnyLlm,
    /// Review-state persistence. `None` when the `.diffthing/` store could
    /// not be opened — persistence is best-effort and never blocks review.
    store: Option<Store>,
}

fn all_hunks(files: &[FileDiff]) -> Vec<Hunk> {
    files.iter().flat_map(|f| f.hunks.iter().cloned()).collect()
}

/// Clears the session's `organizing` flag when the spawned task ends, on
/// every path — success, early return, or panic.
struct OrganizeGuard(Arc<Session>);

impl Drop for OrganizeGuard {
    fn drop(&mut self) {
        self.0.organizing.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

impl Session {
    /// Claim the single organization slot. Returns None (caller should skip
    /// spawning) when a run is already in flight.
    fn try_claim_organize(self: &Arc<Self>) -> Option<OrganizeGuard> {
        use std::sync::atomic::Ordering;
        self.organizing
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .ok()
            .map(|_| OrganizeGuard(Arc::clone(self)))
    }
}

impl Session {
    pub async fn boot(
        repo: &Path,
        base: &str,
        token: String,
        llm_client: AnyLlm,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let registry = Registry::with_defaults(repo);
        let files = gitio::diff_against(repo, base).await?;
        let hunks = all_hunks(&files);
        let scores: BTreeMap<HunkId, ImpactScore> =
            hunks.iter().map(|h| (h.id.clone(), score_hunk(h, &registry.signals_for(h)))).collect();
        let tree_state = gitio::tree_state(repo, base).await?;

        // Resume persisted review. The store keeps state keyed by content
        // hash; on boot we reconcile the persisted hunk set against the fresh
        // diff so flags migrate and stale "viewed" marks downgrade through the
        // same honesty rules as the live watcher. Best-effort throughout.
        let store = Store::open(repo).ok();
        let mut review = ReviewState::default();
        let mut resumed_walkthrough = None;
        if let Some(store) = &store {
            if let Some((old_hunks, mut persisted, walkthrough)) = store.load(base).await {
                let report = reconcile(&old_hunks, &hunks);
                apply_to_review(&mut persisted, &report);
                review = persisted;
                // Reuse the saved organization only when the tree is
                // identical to when it was saved — no re-asking the LLM to
                // rescope unchanged work. Degraded fallbacks are not reused
                // so a restart retries the LLM.
                if !walkthrough.degraded && walkthrough.tree_state == tree_state {
                    resumed_walkthrough = Some(walkthrough);
                }
            }
        }

        // Initial organization is part of boot. The first snapshot must
        // already describe the current tree; Regenerate remains background.
        let walkthrough = match resumed_walkthrough {
            Some(w) => w,
            None => {
                let w = llm::generate(&llm_client, &hunks, &scores, &tree_state, 1, 2).await;
                // Save immediately so a restart before any review action
                // still resumes this organization instead of re-generating.
                if let Some(store) = &store {
                    let _ = store.save(base, &hunks, &review, &w).await;
                }
                w
            }
        };

        let (events, _) = broadcast::channel(64);
        Ok(Self {
            repo: repo.to_path_buf(),
            base: base.to_string(),
            token,
            state: Mutex::new(Snapshot { walkthrough, files, scores, review }),
            pending: Mutex::new(None),
            events,
            writer: Arc::new(Mutex::new(())),
            organizing: std::sync::atomic::AtomicBool::new(false),
            registry,
            llm: llm_client,
            store,
        })
    }

    /// Mirror current review state to the `.diffthing/` store. Best-effort:
    /// a failed write is logged and dropped, never surfaced to the reviewer.
    /// Call after every review-state transition.
    pub async fn persist(&self) {
        let Some(store) = &self.store else { return };
        let (hunks, review, walkthrough) = {
            let state = self.state.lock().await;
            (all_hunks(&state.files), state.review.clone(), state.walkthrough.clone())
        };
        if let Err(e) = store.save(&self.base, &hunks, &review, &walkthrough).await {
            eprintln!("diffthing: failed to persist review state: {e}");
        }
    }

    /// Counts printed once initial diff collection and organization finish.
    pub async fn startup_counts(&self) -> (usize, usize, usize, bool) {
        let state = self.state.lock().await;
        (
            state.files.len(),
            all_hunks(&state.files).len(),
            state.walkthrough.scopes.len(),
            state.walkthrough.degraded,
        )
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
        *self.pending.lock().await =
            Some(Pending { revision, files: new_files, report: report.clone() });
        let _ = self.events.send(ServerMsg::UpdateAvailable { revision, report });
    }

    /// Record human review. Once every hunk in a file is viewed and no open
    /// flag remains on that file, stage it as approved and refresh active
    /// review. Returns true when staging happened.
    pub async fn mark_viewed(&self, id: HunkId) -> bool {
        {
            let mut state = self.state.lock().await;
            state.review.mark_viewed(id.clone());
        }
        let staged = self.stage_if_approved(&id).await;
        self.persist().await;
        staged
    }

    /// Re-check file approval after any action that can clear its final
    /// blocker, including resolving its last open comment.
    pub async fn stage_if_approved(&self, id: &HunkId) -> bool {
        let approved_path = {
            let state = self.state.lock().await;
            let Some(path) = state
                .files
                .iter()
                .flat_map(|file| file.hunks.iter())
                .find(|hunk| &hunk.id == id)
                .map(|hunk| hunk.path.clone())
            else {
                return false;
            };
            let file_hunks: Vec<&HunkId> = state
                .files
                .iter()
                .filter(|file| file.path == path)
                .flat_map(|file| file.hunks.iter().map(|hunk| &hunk.id))
                .collect();
            let all_viewed = file_hunks.iter().all(|hunk| {
                state.review.status_of(hunk) == diffthing_core::review::HunkStatus::Viewed
            });
            let has_open_flag = state
                .review
                .open_flags()
                .any(|flag| file_hunks.iter().any(|hunk| **hunk == flag.hunk));
            (all_viewed && !has_open_flag).then_some(path)
        };

        let Some(path) = approved_path else { return false };
        if gitio::stage_path(&self.repo, &path).await.is_err() {
            return false;
        }
        self.on_fs_quiescence().await;
        true
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
        let tree_state =
            gitio::tree_state(&self.repo, &self.base).await.unwrap_or_else(|_| "unknown".into());
        // Incremental assignment (M1): the existing structure PERSISTS.
        // Carried/changed hunks keep their step, new hunks in claimed files
        // inherit the step, orphans land in an appended "New changes" scope.
        // Never a full regenerate — stability beats optimality.
        let walkthrough = diffthing_core::assign::carry_walkthrough(
            &state.walkthrough,
            &pending.report,
            &hunks,
            &tree_state,
            pending.revision,
        );

        state.files = pending.files;
        state.scores = scores;
        state.walkthrough = walkthrough;
        drop(state);
        // Reconciliation just migrated flags and re-queued changed hunks;
        // persist so a restart mid-review doesn't replay stale state.
        self.persist().await;
        true
    }

    /// Background LLM upgrade of the served walkthrough. Never blocks boot
    /// or apply: generation runs off-lock, and the result is installed only
    /// if the tree hasn't moved underneath it (stale organization is
    /// dropped, not forced). Noop client = nothing to do.
    pub fn spawn_walkthrough_upgrade(self: &Arc<Self>) {
        if matches!(self.llm, AnyLlm::Noop(_)) {
            return;
        }
        let Some(guard) = self.try_claim_organize() else { return };
        let session = Arc::clone(self);
        tokio::spawn(async move {
            let _guard = guard;
            let _ = session.events.send(ServerMsg::GenerationProgress {
                message: format!("organizing walkthrough via {}", session.llm.describe()),
            });
            let (hunks, scores, tree_state, revision) = {
                let state = session.state.lock().await;
                (
                    all_hunks(&state.files),
                    state.scores.clone(),
                    state.walkthrough.tree_state.clone(),
                    state.walkthrough.revision,
                )
            };
            let w =
                llm::generate(&session.llm, &hunks, &scores, &tree_state, revision + 1, 2).await;
            if w.degraded {
                let _ = session.events.send(ServerMsg::GenerationProgress {
                    message: "agent unavailable — keeping deterministic walkthrough".into(),
                });
                return;
            }
            let mut state = session.state.lock().await;
            if state.walkthrough.tree_state != tree_state {
                return; // tree moved while generating; watcher pipeline owns it now
            }
            state.walkthrough = w;
            let snap = ServerMsg::Snapshot {
                walkthrough: state.walkthrough.clone(),
                files: state.files.clone(),
                scores: state.scores.clone(),
                review: state.review.clone(),
            };
            drop(state);
            let _ = session.events.send(snap);
            session.persist().await;
        });
    }

    /// Organize only orphan hunks appended by incremental assignment. This
    /// replaces the placeholder "New changes" scope without reshuffling
    /// scopes the reviewer may already be working through.
    pub fn spawn_new_changes_upgrade(self: &Arc<Self>) {
        if matches!(self.llm, AnyLlm::Noop(_)) {
            return;
        }
        let Some(guard) = self.try_claim_organize() else { return };
        let session = Arc::clone(self);
        tokio::spawn(async move {
            let _guard = guard;
            let (hunks, scores, tree_state, revision, orphan_ids) = {
                let state = session.state.lock().await;
                let Some(scope) =
                    state.walkthrough.scopes.iter().find(|s| s.id == "scope:new-changes")
                else {
                    return;
                };
                let orphan_ids: BTreeSet<HunkId> =
                    scope.steps.iter().flat_map(|step| step.hunks.iter().cloned()).collect();
                let hunks: Vec<Hunk> = all_hunks(&state.files)
                    .into_iter()
                    .filter(|hunk| orphan_ids.contains(&hunk.id))
                    .collect();
                let scores = state
                    .scores
                    .iter()
                    .filter(|(id, _)| orphan_ids.contains(*id))
                    .map(|(id, score)| (id.clone(), score.clone()))
                    .collect();
                (
                    hunks,
                    scores,
                    state.walkthrough.tree_state.clone(),
                    state.walkthrough.revision,
                    orphan_ids,
                )
            };
            if hunks.is_empty() {
                return;
            }

            let _ = session.events.send(ServerMsg::GenerationProgress {
                message: format!("summarizing new changes via {}", session.llm.describe()),
            });
            let mut generated =
                llm::generate(&session.llm, &hunks, &scores, &tree_state, revision, 2).await;
            if generated.degraded {
                let _ = session.events.send(ServerMsg::GenerationProgress {
                    message: "agent unavailable — keeping New changes summary".into(),
                });
                return;
            }

            // Generated ids start at scope:0 / step:0:0 and may collide with
            // preserved walkthrough ids. Namespace them before merging.
            for (scope_index, scope) in generated.scopes.iter_mut().enumerate() {
                scope.id = format!("scope:new-ai:{revision}:{scope_index}");
                for (step_index, step) in scope.steps.iter_mut().enumerate() {
                    step.id = format!("step:new-ai:{revision}:{scope_index}:{step_index}");
                }
            }

            let mut state = session.state.lock().await;
            if state.walkthrough.tree_state != tree_state
                || state.walkthrough.revision != revision
                || state
                    .walkthrough
                    .scopes
                    .iter()
                    .find(|scope| scope.id == "scope:new-changes")
                    .map(|scope| {
                        scope
                            .steps
                            .iter()
                            .flat_map(|step| step.hunks.iter().cloned())
                            .collect::<BTreeSet<_>>()
                    })
                    != Some(orphan_ids)
            {
                return;
            }
            let Some(index) =
                state.walkthrough.scopes.iter().position(|scope| scope.id == "scope:new-changes")
            else {
                return;
            };
            state.walkthrough.scopes.splice(index..=index, generated.scopes);
            let snap = ServerMsg::Snapshot {
                walkthrough: state.walkthrough.clone(),
                files: state.files.clone(),
                scores: state.scores.clone(),
                review: state.review.clone(),
            };
            drop(state);
            let _ = session.events.send(snap);
            session.persist().await;
        });
    }

    /// Human label of the walkthrough organizer, for the client UI.
    pub fn llm_label(&self) -> String {
        self.llm.describe()
    }

    /// Agent selected for walkthrough generation. Dispatch `auto` reuses
    /// this exact agent so one session cannot organize with Codex then edit
    /// with Claude|Codex merely because it's appears first on PATH.
    pub fn agent_name(&self) -> Option<&str> {
        self.llm.agent_name()
    }

    pub async fn export_markdown(&self) -> String {
        let state = self.state.lock().await;
        let mut out = String::from("## diffthing review export\n\n");
        for f in state.review.open_flags() {
            if let Some(h) =
                state.files.iter().flat_map(|fd| fd.hunks.iter()).find(|h| h.id == f.hunk)
            {
                out.push_str(&format!(
                    "### {} (line {})\nhunk: `{}`\n\n> {}\n\n```diff\n{}\n```\n\n",
                    h.path,
                    h.new_start,
                    h.id.0,
                    f.headline(),
                    h.lines.join("\n")
                ));
            }
        }
        out.push_str(
            "Address only the flags above. Do not refactor, reformat, or touch unrelated files.\n",
        );
        out
    }
}
