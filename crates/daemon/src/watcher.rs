//! Watcher: notify events -> gitignore filter -> debounce to quiescence
//! (~2s of silence) -> session reconciliation. Agents burst-write; the
//! quiescence window turns a 15-file burst into one update, not fifteen.

use crate::session::Session;
use ignore::gitignore::GitignoreBuilder;
use notify::{RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

pub const QUIESCENCE: Duration = Duration::from_millis(2000);

pub fn spawn(repo: PathBuf, session: Arc<Session>) {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<()>();

    // gitignore filter (repo .gitignore + always-ignore .git and .diffthing)
    let mut gi = GitignoreBuilder::new(&repo);
    let _ = gi.add(repo.join(".gitignore"));
    let gitignore = gi.build().ok();

    let repo_for_watcher = repo.clone();
    std::thread::spawn(move || {
        let tx2 = tx;
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if let Ok(event) = res {
                let relevant = event.paths.iter().any(|p| {
                    let rel = p.strip_prefix(&repo_for_watcher).unwrap_or(p);
                    let s = rel.to_string_lossy();
                    if s.starts_with(".git") || s.starts_with(".diffthing") {
                        return false;
                    }
                    match &gitignore {
                        Some(gi) => !gi.matched(rel, p.is_dir()).is_ignore(),
                        None => true,
                    }
                });
                if relevant {
                    let _ = tx2.send(());
                }
            }
        })
        .expect("create watcher");
        watcher.watch(&repo, RecursiveMode::Recursive).expect("watch repo");
        // Park the thread; watcher lives as long as the process.
        loop {
            std::thread::park();
        }
    });

    tokio::spawn(async move {
        loop {
            // Wait for first event...
            if rx.recv().await.is_none() {
                break;
            }
            // ...then drain until QUIESCENCE of silence.
            loop {
                match tokio::time::timeout(QUIESCENCE, rx.recv()).await {
                    Ok(Some(())) => continue,
                    Ok(None) => return,
                    Err(_) => break, // quiet — fire
                }
            }
            session.on_fs_quiescence().await;
        }
    });
}
