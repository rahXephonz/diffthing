//! Git access = shell out to the git binary. Deliberate decision: git
//! correctness is not a problem this product should own (no libgit2, no
//! reimplementation). The parser for its output lives in core and is
//! unit-tested there.

use diffthing_core::hunk::{parse_unified_diff, FileDiff};
use std::path::Path;
use tokio::process::Command;

pub fn is_git_repo(root: &Path) -> bool {
    root.join(".git").exists()
}

pub async fn diff_against(root: &Path, base: &str) -> std::io::Result<Vec<FileDiff>> {
    // --no-color --no-ext-diff: stable machine output.
    // -U3 default context; hunk identity normalizes trailing ws anyway.
    let out = Command::new("git")
        .current_dir(root)
        .args(["diff", "--no-color", "--no-ext-diff", base])
        .output()
        .await?;
    let text = String::from_utf8_lossy(&out.stdout);
    Ok(parse_unified_diff(&text))
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
    let diff =
        Command::new("git").current_dir(root).args(["diff", "--no-color", base]).output().await?;
    use diffthing_core::hunk::hunk_id;
    let lines: Vec<String> =
        String::from_utf8_lossy(&diff.stdout).lines().map(|s| s.to_string()).collect();
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
