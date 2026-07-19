//! Agent dispatch (M2). The human's judgment, executed by the human's
//! agent — never the machine's judgment.
//!
//! Flow: RequestChange -> single-writer lock -> snapshot for revert ->
//! run the user's agent CLI to EDIT the flagged code -> bound its blast
//! radius (scope check) -> record the agent's SUMMARY as an AgentClaim
//! (what it did, never a verdict). The edited files then flow back through
//! the SAME watcher->reconcile pipeline as any other change — no special
//! apply path (CLAUDE.md invariants 3, 4, 9). Reconcile independently
//! confirms the hunk moved and flips `addressed_claim`; the human still
//! clicks Close.

use crate::gitio;
use crate::llm;
use crate::session::Session;
use diffthing_core::hunk::HunkId;
use diffthing_core::protocol::{ErrorCode, JobStatus, ServerMsg};
use diffthing_core::review::FlagEntryKind;
use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

/// Agent CLIs we know how to drive in EDIT mode — they modify files in the
/// working dir. Distinct from the JSON-emitting walkthrough call in llm.rs:
/// here the prompt is the last positional arg and the tool writes to disk.
/// Auto-accept flags keep the run headless; the snapshot + revert is the
/// safety net, and scope-check is the honesty net.
const RUNNERS: &[(&str, &[&str])] = &[
    ("claude", &["-p", "--permission-mode", "acceptEdits"]),
    ("codex", &["exec", "--full-auto"]),
    ("aider", &["--yes", "--no-auto-commits", "--message"]),
    ("gemini", &["-p"]),
];

const DISPATCH_TIMEOUT: Duration = Duration::from_secs(600);

fn resolve_runner(
    choice: &str,
    session_agent: Option<&str>,
) -> Option<(&'static str, &'static [&'static str])> {
    match choice {
        "auto" => session_agent
            .and_then(|name| {
                RUNNERS.iter().copied().find(|(bin, _)| *bin == name && llm::on_path(bin))
            })
            .or_else(|| RUNNERS.iter().copied().find(|(bin, _)| llm::on_path(bin))),
        name => RUNNERS.iter().copied().find(|(bin, _)| *bin == name && llm::on_path(bin)),
    }
}

/// Files the agent touched that were neither in scope nor already dirty
/// before it ran — its out-of-scope blast radius. Pure so it's testable.
fn scope_violations(
    scope: &BTreeSet<String>,
    pre: &BTreeSet<String>,
    post: &BTreeSet<String>,
) -> Vec<String> {
    post.difference(pre).filter(|p| !scope.contains(*p)).cloned().collect()
}

/// The agent's own summary of what it changed. We ask it to end with a
/// `SUMMARY:` line; failing that, fall back to its last non-empty line.
/// Either way this is a CLAIM the human reads — reconcile does the verifying.
fn extract_summary(stdout: &str) -> String {
    if let Some(s) = stdout
        .lines()
        .rev()
        .find_map(|l| l.trim().strip_prefix("SUMMARY:").map(|s| s.trim().to_string()))
    {
        if !s.is_empty() {
            return s;
        }
    }
    stdout
        .lines()
        .rev()
        .map(|l| l.trim())
        .find(|l| !l.is_empty())
        .map(|l| l.to_string())
        .unwrap_or_else(|| "agent finished; changes pending your review".into())
}

fn build_prompt(flagged: &[FlaggedHunk], instruction: &str) -> String {
    let mut p = String::from(
        "You are executing a reviewer's change request on a git working tree. \
Make ONLY the changes described. Do not refactor, reformat, or touch files \
that are not part of this request.\n\n",
    );
    p.push_str(
        "Reviewer's instruction (GitHub-flavored Markdown; interpret headings, lists, task lists, links, and fenced code as structured requirements):\n",
    );
    p.push_str(instruction.replace("\r\n", "\n").replace('\r', "\n").trim());
    p.push_str("\n\nThe change is anchored to these hunks:\n");
    for h in flagged {
        p.push_str(&format!("\n--- {} (around line {}) ---\n", h.path, h.line));
        if !h.comment.is_empty() {
            p.push_str(&format!("reviewer note (Markdown):\n{}\n", h.comment));
        }
        p.push_str(&h.body);
        p.push('\n');
    }
    p.push_str(
        "\nAfter you finish editing, print a single final line of the form:\n\
SUMMARY: <one sentence describing what you changed>\n\
Describe only what you did — do not assess whether the code is correct or good.\n",
    );
    p
}

/// A flagged hunk's context for the prompt, gathered under the state lock.
struct FlaggedHunk {
    path: String,
    line: u32,
    body: String,
    comment: String,
}

fn status(job_id: &str, status: JobStatus, detail: Option<String>) -> ServerMsg {
    ServerMsg::DispatchStatus { job_id: job_id.to_string(), status, detail }
}

/// Spawn a dispatch. Fails fast (BusyWriterLock) if another runner holds the
/// writer lock. Everything after acquiring the lock runs in the background
/// task; results are announced over the broadcast channel.
pub fn spawn(
    session: Arc<Session>,
    hunks: Vec<HunkId>,
    line: Option<u32>,
    instruction: String,
    runner: String,
) {
    tokio::spawn(async move {
        let job_id = format!("job-{:08x}", rand::random::<u32>());

        let Some((bin, args)) = resolve_runner(&runner, session.agent_name()) else {
            let _ = session.events.send(status(
                &job_id,
                JobStatus::Failed,
                Some(format!(
                    "no runnable agent for '{runner}' — install one of: {}",
                    RUNNERS.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", ")
                )),
            ));
            return;
        };

        // Single writer. Held for the whole run; released on drop.
        let Ok(_guard) = session.writer.try_lock() else {
            let _ = session.events.send(ServerMsg::Error {
                code: ErrorCode::BusyWriterLock,
                message: "an agent is already editing — wait for it to finish".into(),
            });
            return;
        };

        // Gather flagged hunk context + the in-scope file set, under lock.
        let (flagged, scope): (Vec<FlaggedHunk>, BTreeSet<String>) = {
            let st = session.state.lock().await;
            let want: BTreeSet<&HunkId> = hunks.iter().collect();
            let mut flagged = Vec::new();
            let mut scope = BTreeSet::new();
            for f in &st.files {
                for h in &f.hunks {
                    if want.contains(&h.id) {
                        scope.insert(h.path.clone());
                        let comment = st
                            .review
                            .flags
                            .iter()
                            .find(|fl| fl.hunk == h.id && fl.line == line && fl.open)
                            .map(|fl| fl.headline().to_string())
                            .unwrap_or_default();
                        flagged.push(FlaggedHunk {
                            path: h.path.clone(),
                            line: h.new_start,
                            body: h.lines.join("\n"),
                            comment,
                        });
                    }
                }
            }
            (flagged, scope)
        };

        if flagged.is_empty() {
            let _ = session.events.send(status(
                &job_id,
                JobStatus::Failed,
                Some("none of the requested hunks are in the current diff".into()),
            ));
            return;
        }

        let snapshot = gitio::snapshot(&session.repo).await.ok().flatten();
        let pre: BTreeSet<String> =
            gitio::modified_paths(&session.repo).await.unwrap_or_default().into_iter().collect();

        let _ = session.events.send(status(
            &job_id,
            JobStatus::Running,
            Some(format!("{bin} is applying your request…")),
        ));

        let prompt = build_prompt(&flagged, &instruction);
        let run = tokio::process::Command::new(bin)
            .args(args)
            .arg(&prompt)
            .current_dir(&session.repo)
            .stdin(std::process::Stdio::null())
            .output();

        let out = match tokio::time::timeout(DISPATCH_TIMEOUT, run).await {
            Ok(Ok(out)) => out,
            Ok(Err(e)) => {
                let _ = session.events.send(status(
                    &job_id,
                    JobStatus::Failed,
                    Some(format!("{bin} failed to start: {e}")),
                ));
                return;
            }
            Err(_) => {
                // Timed out: kill already handled by drop; revert tracked files.
                if let Some(snap) = &snapshot {
                    let _ = gitio::restore_tracked(&session.repo, snap).await;
                }
                let _ = session.events.send(status(
                    &job_id,
                    JobStatus::TimedOutReverted,
                    Some("agent exceeded 10 min — tracked files restored".into()),
                ));
                return;
            }
        };

        if !out.status.success() {
            if let Some(snap) = &snapshot {
                let _ = gitio::restore_tracked(&session.repo, snap).await;
            }
            let _ = session.events.send(status(
                &job_id,
                JobStatus::Failed,
                Some(format!(
                    "{bin} exited {} — changes reverted",
                    out.status.code().unwrap_or(-1)
                )),
            ));
            return;
        }

        let stdout = String::from_utf8_lossy(&out.stdout);
        let summary = extract_summary(&stdout);

        let post: BTreeSet<String> =
            gitio::modified_paths(&session.repo).await.unwrap_or_default().into_iter().collect();
        let out_of_scope = scope_violations(&scope, &pre, &post);

        // Record the agent's claim on every dispatched flag. This is a
        // CLAIM entry — reconcile independently flips addressed_claim when
        // it sees the hunk actually moved, and the human still closes.
        {
            let mut st = session.state.lock().await;
            let rev = st.walkthrough.revision;
            let want: BTreeSet<&HunkId> = hunks.iter().collect();
            for f in st.review.flags.iter_mut() {
                if f.open && f.line == line && want.contains(&f.hunk) {
                    f.push(FlagEntryKind::AgentClaim, summary.clone(), rev);
                    if !out_of_scope.is_empty() {
                        f.push(
                            FlagEntryKind::DispatchNote,
                            format!("⚠ agent also touched: {}", out_of_scope.join(", ")),
                            rev,
                        );
                    }
                }
            }
            // Push the claim to the reader immediately; the diff itself
            // updates when the watcher reconciles and the client applies.
            let snap = ServerMsg::Snapshot {
                walkthrough: st.walkthrough.clone(),
                files: st.files.clone(),
                scores: st.scores.clone(),
                review: st.review.clone(),
            };
            drop(st);
            let _ = session.events.send(snap);
        }

        let (final_status, detail) = if out_of_scope.is_empty() {
            (JobStatus::Done, summary)
        } else {
            (
                JobStatus::ScopeViolation,
                format!("{summary} — but also modified {} unflagged file(s)", out_of_scope.len()),
            )
        };
        let _ = session.events.send(status(&job_id, final_status, Some(detail)));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn scope_ok_when_only_flagged_files_change() {
        let scope = set(&["a.ts"]);
        let pre = set(&["a.ts", "b.ts"]);
        let post = set(&["a.ts", "b.ts"]);
        assert!(scope_violations(&scope, &pre, &post).is_empty());
    }

    #[test]
    fn scope_violation_flags_newly_touched_out_of_scope_file() {
        let scope = set(&["a.ts"]);
        let pre = set(&["a.ts"]);
        let post = set(&["a.ts", "unrelated.ts"]);
        assert_eq!(scope_violations(&scope, &pre, &post), vec!["unrelated.ts".to_string()]);
    }

    #[test]
    fn already_dirty_out_of_scope_file_is_not_a_violation() {
        // It was dirty before the agent ran — not the agent's doing.
        let scope = set(&["a.ts"]);
        let pre = set(&["a.ts", "already.ts"]);
        let post = set(&["a.ts", "already.ts"]);
        assert!(scope_violations(&scope, &pre, &post).is_empty());
    }

    #[test]
    fn summary_prefers_marked_line() {
        let out = "editing files...\nSUMMARY: moved fetch server-side\ndone";
        assert_eq!(extract_summary(out), "moved fetch server-side");
    }

    #[test]
    fn summary_falls_back_to_last_line() {
        let out = "did the thing\nall good";
        assert_eq!(extract_summary(out), "all good");
    }
}
