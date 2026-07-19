//! Hunk model + unified-diff parsing.
//!
//! Identity rule (load-bearing): a hunk is identified by the SHA-256 of
//! `path + normalized body`, NOT by ordinal position. Review state attaches
//! to this hash so it survives regeneration, reordering, and live updates.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[cfg(feature = "ts-export")]
use ts_rs::TS;

/// Stable content-derived hunk identity.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[cfg_attr(feature = "ts-export", derive(TS))]
pub struct HunkId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(TS))]
pub struct Hunk {
    pub id: HunkId,
    pub path: String,
    /// 1-based line in the NEW file where the hunk starts (0 for pure deletions).
    pub new_start: u32,
    pub old_start: u32,
    pub added: u32,
    pub removed: u32,
    /// Raw hunk body lines including +/-/space prefixes (no @@ header).
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(TS))]
pub struct FileDiff {
    pub path: String,
    pub old_path: Option<String>,
    pub status: FileStatus,
    pub hunks: Vec<Hunk>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts-export", derive(TS))]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
}

/// Normalization before hashing: strip trailing whitespace per line.
/// Deliberately do NOT strip leading whitespace — indentation changes are
/// real changes. Do NOT include line numbers — a hunk that merely shifted
/// position is the same hunk.
fn normalized_body(lines: &[String]) -> String {
    let mut s = String::new();
    for l in lines {
        s.push_str(l.trim_end());
        s.push('\n');
    }
    s
}

pub fn hunk_id(path: &str, lines: &[String]) -> HunkId {
    let mut h = Sha256::new();
    h.update(path.as_bytes());
    h.update([0u8]);
    h.update(normalized_body(lines).as_bytes());
    HunkId(hex::encode(&h.finalize()[..16]))
}

/// Minimal, strict unified-diff parser (git diff output).
/// Handles: file headers, renames, new/deleted files, multiple hunks,
/// `\ No newline at end of file` markers.
pub fn parse_unified_diff(input: &str) -> Vec<FileDiff> {
    let mut files: Vec<FileDiff> = Vec::new();
    let mut cur: Option<FileDiff> = None;
    let mut cur_hunk: Option<(u32, u32, Vec<String>)> = None; // old_start, new_start, lines

    fn flush_hunk(file: &mut FileDiff, hunk: Option<(u32, u32, Vec<String>)>) {
        if let Some((old_start, new_start, lines)) = hunk {
            let added = lines.iter().filter(|l| l.starts_with('+')).count() as u32;
            let removed = lines.iter().filter(|l| l.starts_with('-')).count() as u32;
            let id = hunk_id(&file.path, &lines);
            file.hunks.push(Hunk {
                id,
                path: file.path.clone(),
                new_start,
                old_start,
                added,
                removed,
                lines,
            });
        }
    }

    for line in input.lines() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            if let Some(mut f) = cur.take() {
                flush_hunk(&mut f, cur_hunk.take());
                files.push(f);
            }
            // `a/path b/path` — take the b-side; paths with spaces are rare
            // in v1, quoted paths are a documented TODO.
            let b = rest.split(" b/").nth(1).unwrap_or(rest).to_string();
            cur = Some(FileDiff {
                path: b,
                old_path: None,
                status: FileStatus::Modified,
                hunks: Vec::new(),
            });
        } else if let Some(f) = cur.as_mut() {
            if line.starts_with("new file mode") {
                f.status = FileStatus::Added;
            } else if line.starts_with("deleted file mode") {
                f.status = FileStatus::Deleted;
            } else if let Some(p) = line.strip_prefix("rename from ") {
                f.status = FileStatus::Renamed;
                f.old_path = Some(p.to_string());
            } else if let Some(p) = line.strip_prefix("rename to ") {
                f.path = p.to_string();
            } else if line.starts_with("@@") {
                flush_hunk(f, cur_hunk.take());
                // @@ -old_start[,n] +new_start[,n] @@
                let nums: Vec<&str> = line.split_whitespace().collect();
                let parse_start = |tok: &str| -> u32 {
                    tok.trim_start_matches(['-', '+'])
                        .split(',')
                        .next()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0)
                };
                let old_start = nums.get(1).map(|t| parse_start(t)).unwrap_or(0);
                let new_start = nums.get(2).map(|t| parse_start(t)).unwrap_or(0);
                cur_hunk = Some((old_start, new_start, Vec::new()));
            } else if let Some((_, _, lines)) = cur_hunk.as_mut() {
                if line.starts_with('+')
                    || line.starts_with('-')
                    || line.starts_with(' ')
                    || line.starts_with('\\')
                {
                    lines.push(line.to_string());
                }
            }
        }
    }
    if let Some(mut f) = cur.take() {
        flush_hunk(&mut f, cur_hunk.take());
        files.push(f);
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
diff --git a/src/pay.ts b/src/pay.ts
index 111..222 100644
--- a/src/pay.ts
+++ b/src/pay.ts
@@ -10,4 +10,5 @@ export function pay() {
 context
-old line
+new line
+added line
 context
diff --git a/README.md b/README.md
new file mode 100644
@@ -0,0 +1,2 @@
+# hi
+there
";

    #[test]
    fn parses_two_files_and_counts() {
        let files = parse_unified_diff(SAMPLE);
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].path, "src/pay.ts");
        assert_eq!(files[0].hunks.len(), 1);
        assert_eq!(files[0].hunks[0].added, 2);
        assert_eq!(files[0].hunks[0].removed, 1);
        assert_eq!(files[0].hunks[0].old_start, 10);
        assert_eq!(files[0].hunks[0].new_start, 10);
        assert_eq!(files[1].status, FileStatus::Added);
    }

    #[test]
    fn hash_is_position_independent() {
        let a = hunk_id("f.ts", &["+x".into(), " y".into()]);
        let b = hunk_id("f.ts", &["+x".into(), " y".into()]);
        assert_eq!(a, b);
    }

    #[test]
    fn hash_is_path_and_content_sensitive() {
        let a = hunk_id("f.ts", &["+x".into()]);
        let b = hunk_id("g.ts", &["+x".into()]);
        let c = hunk_id("f.ts", &["+z".into()]);
        assert_ne!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn trailing_whitespace_does_not_change_identity() {
        let a = hunk_id("f.ts", &["+x  ".into()]);
        let b = hunk_id("f.ts", &["+x".into()]);
        assert_eq!(a, b);
    }
}
