//! Git access = shell out to the git binary. Deliberate decision: git
//! correctness is not a problem this product should own (no libgit2, no
//! reimplementation). The parser for its output lives in core and is
//! unit-tested there.

use diffthing_core::hunk::{parse_unified_diff, FileDiff};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use tokio::process::Command;

pub fn is_git_repo(root: &Path) -> bool {
    root.join(".git").exists()
}

/// Full diff against `base`, INCLUDING untracked files. Plain `git diff`
/// only ever sees tracked changes — agents create new files constantly, so
/// that blind spot means agent-authored files are invisible to review.
///
/// Fix: snapshot the working tree (tracked + untracked, respecting
/// .gitignore) into a throwaway index via `GIT_INDEX_FILE`, diff that
/// against `base`, discard the temp index. Never touches the real
/// `.git/index` — this stays a read-only operation from the user's POV.
async fn diff_text(root: &Path, base: &str) -> std::io::Result<String> {
    let tmp_index = std::env::temp_dir().join(format!(
        "diffthing-index-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default()
    ));

    let result = diff_text_with_temp_index(root, base, &tmp_index).await;
    let _ = std::fs::remove_file(&tmp_index);
    result
}

async fn diff_text_with_temp_index(
    root: &Path,
    base: &str,
    tmp_index: &PathBuf,
) -> std::io::Result<String> {
    Command::new("git")
        .current_dir(root)
        .env("GIT_INDEX_FILE", tmp_index)
        .args(["add", "-A"])
        .output()
        .await?;

    // --no-color --no-ext-diff: stable machine output.
    // -U3 default context; hunk identity normalizes trailing ws anyway.
    let out = Command::new("git")
        .current_dir(root)
        .env("GIT_INDEX_FILE", tmp_index)
        .args(["diff", "--no-color", "--no-ext-diff", "--cached", base])
        .output()
        .await?;
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub async fn diff_against(root: &Path, base: &str) -> std::io::Result<Vec<FileDiff>> {
    let text = diff_text(root, base).await?;
    let staged = staged_only_paths(root).await?;
    Ok(parse_unified_diff(&text).into_iter().filter(|file| !staged.contains(&file.path)).collect())
}

/// Stage one human-approved file. `--` prevents path-like filenames from
/// being parsed as options. Git index becomes approval ledger: fully staged
/// files leave active review, while later working-tree edits reappear.
pub async fn stage_path(root: &Path, path: &str) -> std::io::Result<()> {
    let out = Command::new("git").current_dir(root).args(["add", "--", path]).output().await?;
    if out.status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(String::from_utf8_lossy(&out.stderr).into_owned()))
    }
}

/// Paths changed in index but clean in working tree. These are approved and
/// excluded from active review. A later edit sets worktree status too, making
/// path visible again despite its staged baseline.
async fn staged_only_paths(root: &Path) -> std::io::Result<BTreeSet<String>> {
    let out = Command::new("git")
        .current_dir(root)
        .args(["status", "--porcelain", "--no-renames", "-z"])
        .output()
        .await?;
    let text = String::from_utf8_lossy(&out.stdout);
    Ok(text
        .split('\0')
        .filter(|record| record.len() > 3)
        .filter(|record| {
            let status = record.as_bytes();
            status[0] != b' ' && status[1] == b' '
        })
        .map(|record| record[3..].to_string())
        .collect())
}

/// Tree state fingerprint: HEAD rev + short hash of the diff itself, so the
/// walkthrough records exactly what it was generated against.
pub async fn tree_state(root: &Path, base: &str) -> std::io::Result<String> {
    let head = Command::new("git")
        .current_dir(root)
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .await?;
    let head = String::from_utf8_lossy(&head.stdout).trim().to_string();
    let diff = diff_text(root, base).await?;
    use diffthing_core::hunk::hunk_id;
    let lines: Vec<String> = diff.lines().map(|s| s.to_string()).collect();
    let fp = hunk_id("__tree__", &lines).0;
    Ok(format!("{head}+{}", &fp[..8]))
}

/// Snapshot before agent dispatch — powers one-click revert.
/// `git stash create` returns a commit ref WITHOUT touching the tree.
pub async fn snapshot(root: &Path) -> std::io::Result<Option<String>> {
    let out = Command::new("git")
        .current_dir(root)
        .args(["stash", "create", "diffthing pre-dispatch snapshot"])
        .output()
        .await?;
    let r = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok(if r.is_empty() { None } else { Some(r) })
}

/// Restore TRACKED files to a snapshot ref (from `snapshot`). Deliberately
/// non-destructive: it overwrites tracked paths back to the snapshot but
/// never runs `git clean`, so untracked user files are never nuked. Files
/// the agent newly CREATED survive and surface through reconcile — honest,
/// and no silent data loss (CLAUDE.md: destructive git ops stay opt-in).
pub async fn restore_tracked(root: &Path, snapshot_ref: &str) -> std::io::Result<()> {
    Command::new("git")
        .current_dir(root)
        .args(["checkout", snapshot_ref, "--", "."])
        .output()
        .await?;
    Ok(())
}

/// Paths currently modified/added/untracked, per `git status --porcelain`.
/// Used to bound an agent's blast radius: files it touches that weren't in
/// scope and weren't already dirty are surfaced as a scope violation.
pub async fn modified_paths(root: &Path) -> std::io::Result<Vec<String>> {
    let out = Command::new("git")
        .current_dir(root)
        .args(["status", "--porcelain", "--no-renames", "-z"])
        .output()
        .await?;
    let text = String::from_utf8_lossy(&out.stdout);
    // -z: NUL-separated records, each "XY <path>". Rename disabled so a
    // record is always a single path (no " -> " to split).
    Ok(text.split('\0').filter(|r| r.len() > 3).map(|r| r[3..].to_string()).collect())
}
