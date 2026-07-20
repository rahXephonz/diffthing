//! Review-state persistence. The daemon holds review truth in memory; this
//! mirrors it to `.diffthing/review.db` so a restart resumes where the
//! reviewer left off instead of dropping every viewed / commented / resolved
//! mark while agents keep editing. Best-effort by design: a store that fails
//! to open never blocks review — the session just runs without persistence.
//!
//! Keyed by content hash (HunkId), so persisted state survives line shifts
//! for free. On boot we reconcile the stored hunk set against the fresh diff,
//! migrating flags and downgrading stale "viewed" marks through the SAME
//! honesty rules the live watcher uses — no second code path.

use diffthing_core::hunk::Hunk;
use diffthing_core::review::ReviewState;
use diffthing_core::schema::Walkthrough;
use rusqlite::Connection;
use std::path::Path;
use tokio::sync::Mutex;

/// Bump when the persisted shape changes incompatibly. On mismatch `load`
/// returns None (fresh review) rather than mis-reading an old blob — we
/// discard cleanly on a protocol bump, never silently migrate.
const SCHEMA_VERSION: u32 = 2;

type BoxErr = Box<dyn std::error::Error + Send + Sync>;

pub struct Store {
    conn: Mutex<Connection>,
}

impl Store {
    /// Open (creating if needed) `.diffthing/review.db` under `repo`. Also
    /// drops a `.diffthing/.gitignore` (`*`) so the store itself never shows
    /// up in the diff being reviewed.
    pub fn open(repo: &Path) -> Result<Store, BoxErr> {
        let dir = repo.join(".diffthing");
        std::fs::create_dir_all(&dir)?;
        // Keep the whole store out of the reviewed working tree.
        let _ = std::fs::write(dir.join(".gitignore"), "*\n");
        let conn = Connection::open(dir.join("review.db"))?;
        // A table from an older schema (no walkthrough column) can't be
        // written to, let alone read. Discard it — same policy as the
        // version check in `load`.
        let current_shape = conn
            .prepare("SELECT 1 FROM pragma_table_info('review') WHERE name = 'walkthrough'")?
            .exists([])?;
        if !current_shape {
            conn.execute_batch("DROP TABLE IF EXISTS review;")?;
        }
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS review (
                 id          INTEGER PRIMARY KEY CHECK (id = 1),
                 version     INTEGER NOT NULL,
                 base        TEXT NOT NULL,
                 hunks       TEXT NOT NULL,
                 review      TEXT NOT NULL,
                 walkthrough TEXT NOT NULL
             );",
        )?;
        Ok(Store { conn: Mutex::new(conn) })
    }

    /// Load persisted state for `base`. Returns None (⇒ fresh review) when
    /// nothing is stored, the schema version differs, the base differs, or a
    /// blob fails to parse. Every "can't fully trust it" path discards.
    pub async fn load(&self, base: &str) -> Option<(Vec<Hunk>, ReviewState, Walkthrough)> {
        let conn = self.conn.lock().await;
        let (version, stored_base, hunks_json, review_json, walkthrough_json) = conn
            .query_row(
                "SELECT version, base, hunks, review, walkthrough FROM review WHERE id = 1",
                [],
                |r| {
                    Ok((
                        r.get::<_, u32>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, String>(4)?,
                    ))
                },
            )
            .ok()?;
        if version != SCHEMA_VERSION || stored_base != base {
            return None;
        }
        let hunks = serde_json::from_str(&hunks_json).ok()?;
        let review = serde_json::from_str(&review_json).ok()?;
        let walkthrough = serde_json::from_str(&walkthrough_json).ok()?;
        Some((hunks, review, walkthrough))
    }

    /// Mirror the current review state to disk. Overwrites the single row.
    /// Storing the hunk set alongside the review is what lets boot reconcile
    /// the persisted flags/statuses against the next diff; storing the
    /// walkthrough lets boot skip LLM re-organization when the tree hasn't
    /// moved (the walkthrough's own tree_state is the validity key).
    pub async fn save(
        &self,
        base: &str,
        hunks: &[Hunk],
        review: &ReviewState,
        walkthrough: &Walkthrough,
    ) -> Result<(), BoxErr> {
        let hunks_json = serde_json::to_string(hunks)?;
        let review_json = serde_json::to_string(review)?;
        let walkthrough_json = serde_json::to_string(walkthrough)?;
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO review (id, version, base, hunks, review, walkthrough)
             VALUES (1, ?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET
                 version     = excluded.version,
                 base        = excluded.base,
                 hunks       = excluded.hunks,
                 review      = excluded.review,
                 walkthrough = excluded.walkthrough",
            rusqlite::params![SCHEMA_VERSION, base, hunks_json, review_json, walkthrough_json],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use diffthing_core::hunk::hunk_id;
    use diffthing_core::review::Flag;

    fn tmp_repo() -> std::path::PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("diffthing-store-test-{:08x}", rand::random::<u32>()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn mk_walkthrough(tree_state: &str) -> Walkthrough {
        Walkthrough {
            schema_version: 2,
            revision: 1,
            tree_state: tree_state.into(),
            focus: Some("test".into()),
            scopes: vec![],
            degraded: false,
        }
    }

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

    #[tokio::test]
    async fn roundtrips_review_state() {
        let repo = tmp_repo();
        let hunks = vec![mk("a.ts", "+one"), mk("b.ts", "+two")];
        let mut review = ReviewState::default();
        review.mark_viewed(hunks[0].id.clone());
        review.flags.push(Flag::new(hunks[1].id.clone(), Some(3), "why?".into()));

        let store = Store::open(&repo).unwrap();
        store.save("HEAD", &hunks, &review, &mk_walkthrough("tree-a")).await.unwrap();

        // Fresh handle simulates a daemon restart.
        let store2 = Store::open(&repo).unwrap();
        let (loaded_hunks, loaded_review, loaded_walkthrough) = store2.load("HEAD").await.unwrap();
        assert_eq!(loaded_hunks.len(), 2);
        assert_eq!(loaded_review.status.len(), 1);
        assert_eq!(loaded_review.flags.len(), 1);
        assert_eq!(loaded_review.flags[0].headline(), "why?");
        assert_eq!(loaded_walkthrough.tree_state, "tree-a");
        assert!(!loaded_walkthrough.degraded);

        std::fs::remove_dir_all(&repo).ok();
    }

    #[tokio::test]
    async fn different_base_discards() {
        let repo = tmp_repo();
        let store = Store::open(&repo).unwrap();
        store.save("HEAD", &[], &ReviewState::default(), &mk_walkthrough("t")).await.unwrap();
        assert!(store.load("main").await.is_none(), "base mismatch must not load");
        std::fs::remove_dir_all(&repo).ok();
    }

    #[tokio::test]
    async fn old_schema_table_discarded() {
        let repo = tmp_repo();
        let dir = repo.join(".diffthing");
        std::fs::create_dir_all(&dir).unwrap();
        // Hand-build a v1 table (no walkthrough column) with a row in it.
        let conn = Connection::open(dir.join("review.db")).unwrap();
        conn.execute_batch(
            "CREATE TABLE review (
                 id      INTEGER PRIMARY KEY CHECK (id = 1),
                 version INTEGER NOT NULL,
                 base    TEXT NOT NULL,
                 hunks   TEXT NOT NULL,
                 review  TEXT NOT NULL
             );
             INSERT INTO review VALUES (1, 1, 'HEAD', '[]', '{}');",
        )
        .unwrap();
        drop(conn);

        // Open must discard the stale shape, and the store must be writable.
        let store = Store::open(&repo).unwrap();
        assert!(store.load("HEAD").await.is_none(), "v1 blob must not load");
        store.save("HEAD", &[], &ReviewState::default(), &mk_walkthrough("t")).await.unwrap();
        assert!(store.load("HEAD").await.is_some());
        std::fs::remove_dir_all(&repo).ok();
    }

    #[tokio::test]
    async fn empty_store_loads_nothing() {
        let repo = tmp_repo();
        let store = Store::open(&repo).unwrap();
        assert!(store.load("HEAD").await.is_none());
        // .gitignore keeps the store out of the reviewed tree.
        assert_eq!(std::fs::read_to_string(repo.join(".diffthing/.gitignore")).unwrap(), "*\n");
        std::fs::remove_dir_all(&repo).ok();
    }
}
