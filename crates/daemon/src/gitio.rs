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

/// Reject base revisions git could parse as command-line options. A
/// dash-prefixed `--base` value (e.g. `--output=/path`) would otherwise be
/// consumed by `git diff` as a flag, not a revision — argument injection.
pub fn validate_base(base: &str) -> std::io::Result<()> {
    if base.is_empty() || base.starts_with('-') {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("invalid --base {base:?}: must be a git revision, not an option"),
        ));
    }
    Ok(())
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
    validate_base(base)?;
    // tempfile: unique 0700 directory, so the index path inside is neither
    // predictable nor pre-creatable by another local user (no PID/timestamp
    // guessing, no TOCTOU). Dropping `tmp` removes dir + index.
    let tmp = tempfile::Builder::new().prefix("diffthing-index-").tempdir()?;
    let tmp_index = tmp.path().join("index");
    diff_text_with_temp_index(root, base, &tmp_index).await
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
        .args(["diff", "--no-color", "--no-ext-diff", "--cached", base, "--"])
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
/// and no silent data loss (destructive git ops stay opt-in).
pub async fn restore_tracked(root: &Path, snapshot_ref: &str) -> std::io::Result<()> {
    Command::new("git")
        .current_dir(root)
        .args(["checkout", snapshot_ref, "--", "."])
        .output()
        .await?;
    Ok(())
}

/// One `git status --porcelain` record with the tracking split the rollback
/// planner needs: untracked files can't be restored from a stash snapshot
/// (stash create never captures them), so they take the quarantine path.
pub struct StatusPath {
    pub path: String,
    pub untracked: bool,
}

pub async fn status_paths(root: &Path) -> std::io::Result<Vec<StatusPath>> {
    let out = Command::new("git")
        .current_dir(root)
        .args(["status", "--porcelain", "--no-renames", "-z"])
        .output()
        .await?;
    let text = String::from_utf8_lossy(&out.stdout);
    // -z: NUL-separated records, each "XY <path>". Rename disabled so a
    // record is always a single path (no " -> " to split).
    Ok(text
        .split('\0')
        .filter(|r| r.len() > 3)
        .map(|r| StatusPath { path: r[3..].to_string(), untracked: r.starts_with("??") })
        .collect())
}

/// Gitignored paths (collapsed: an ignored directory is one entry). Best
/// effort — a new file inside an already-ignored directory is invisible in
/// this view, which keeps the listing bounded on big trees (node_modules).
pub async fn ignored_paths(root: &Path) -> std::io::Result<BTreeSet<String>> {
    let out = Command::new("git")
        .current_dir(root)
        .args(["status", "--porcelain", "--no-renames", "--ignored=traditional", "-z"])
        .output()
        .await?;
    let text = String::from_utf8_lossy(&out.stdout);
    Ok(text
        .split('\0')
        .filter(|r| r.len() > 3 && r.starts_with("!!"))
        .map(|r| r[3..].to_string())
        .collect())
}

/// Content fingerprints for `paths` (git blob hashes, one invocation).
/// Missing/unreadable files simply have no entry. Used to catch an agent
/// editing a file that was ALREADY dirty before it ran — the path-set diff
/// alone can't see that.
pub async fn hash_paths(
    root: &Path,
    paths: &[String],
) -> std::io::Result<std::collections::BTreeMap<String, String>> {
    if paths.is_empty() {
        return Ok(Default::default());
    }
    let mut cmd = Command::new("git");
    cmd.current_dir(root).args(["hash-object", "--"]);
    let existing: Vec<&String> = paths.iter().filter(|p| root.join(p).is_file()).collect();
    if existing.is_empty() {
        return Ok(Default::default());
    }
    for p in &existing {
        cmd.arg(p);
    }
    let out = cmd.output().await?;
    let text = String::from_utf8_lossy(&out.stdout);
    Ok(existing.iter().map(|p| (*p).clone()).zip(text.lines().map(str::to_string)).collect())
}

/// Restore specific TRACKED paths from a snapshot ref. Narrow sibling of
/// `restore_tracked`: rollback for out-of-scope agent edits must not touch
/// the in-scope work the user asked for.
pub async fn restore_paths(
    root: &Path,
    snapshot_ref: &str,
    paths: &[String],
) -> std::io::Result<()> {
    if paths.is_empty() {
        return Ok(());
    }
    let mut cmd = Command::new("git");
    cmd.current_dir(root).args(["checkout", snapshot_ref, "--"]);
    for p in paths {
        cmd.arg(p);
    }
    cmd.output().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_base;

    #[test]
    fn validate_base_accepts_revisions() {
        for ok in ["HEAD", "HEAD~3", "main", "origin/main", "v0.2.1", "abc123"] {
            assert!(validate_base(ok).is_ok(), "should accept {ok:?}");
        }
    }

    #[test]
    fn validate_base_rejects_option_shaped_input() {
        for bad in ["", "-x", "--output=/tmp/pwn", "--ext-diff", "-", "--"] {
            assert!(validate_base(bad).is_err(), "should reject {bad:?}");
        }
    }
}
