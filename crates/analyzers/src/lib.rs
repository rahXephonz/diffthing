//! Ecosystem analyzers. diffthing is language-agnostic by architecture,
//! language-aware by plugin — this trait is the plugin boundary.
//!
//! Rule: analyzers are DETERMINISTIC. They feed `ExternalSignals` into the
//! core scorer. No LLM in this crate, ever.
//!
//! v1 depth ladder (business decision, see CLAUDE.md):
//!   TS/JS: full (module graph fan-in + export surface delta)  [M2]
//!   Rust/Elixir: parse-level (public surface only)            [M3]
//!   Solidity: premium domain analyzer                         [M4]
//!   Everything else: FallbackAnalyzer (universal signals) — day one.

use diffthing_core::hunk::Hunk;
use diffthing_core::score::ExternalSignals;
use std::collections::BTreeMap;
use std::path::Path;

/// Module dependency graph: file -> files that import it (reverse edges),
/// so fan-in lookup is O(1).
#[derive(Debug, Default, Clone)]
pub struct ModuleGraph {
    pub importers: BTreeMap<String, Vec<String>>,
}

impl ModuleGraph {
    pub fn fan_in(&self, path: &str) -> u32 {
        self.importers.get(path).map(|v| v.len() as u32).unwrap_or(0)
    }
}

pub trait Analyzer: Send + Sync {
    fn id(&self) -> &'static str;
    /// Does this analyzer own this file?
    fn matches(&self, path: &Path) -> bool;
    /// Build/refresh the module graph for the repo. Called at boot and
    /// invalidated incrementally by the watcher.
    fn build_graph(&self, repo_root: &Path) -> ModuleGraph;
    /// Did this hunk change the file's public surface?
    /// (exported symbols in TS, `pub` items in Rust, non-defp in Elixir,
    /// external/public functions in Solidity)
    fn public_surface_changed(&self, hunk: &Hunk) -> Option<bool>;
}

/// Universal signals only — path priors, size, lockfiles all live in the
/// core scorer already, so the fallback contributes "unknown" and the
/// product still works on any repo, day one. Graceful degradation, not a
/// "sorry, unsupported language" gate.
pub struct FallbackAnalyzer;

impl Analyzer for FallbackAnalyzer {
    fn id(&self) -> &'static str {
        "fallback"
    }
    fn matches(&self, _path: &Path) -> bool {
        true
    }
    fn build_graph(&self, _repo_root: &Path) -> ModuleGraph {
        ModuleGraph::default()
    }
    fn public_surface_changed(&self, _hunk: &Hunk) -> Option<bool> {
        None
    }
}

pub struct Registry {
    analyzers: Vec<Box<dyn Analyzer>>,
    graph: ModuleGraph,
}

impl Registry {
    pub fn with_defaults(repo_root: &Path) -> Self {
        let analyzers: Vec<Box<dyn Analyzer>> = vec![Box::new(FallbackAnalyzer)];
        let graph = analyzers
            .iter()
            .map(|a| a.build_graph(repo_root))
            .fold(ModuleGraph::default(), |mut acc, g| {
                acc.importers.extend(g.importers);
                acc
            });
        Self { analyzers, graph }
    }

    pub fn signals_for(&self, hunk: &Hunk) -> ExternalSignals {
        let path = Path::new(&hunk.path);
        let analyzer = self
            .analyzers
            .iter()
            .find(|a| a.matches(path))
            .expect("fallback always matches");
        ExternalSignals {
            importer_count: match self.graph.fan_in(&hunk.path) {
                0 => None,
                n => Some(n),
            },
            public_surface_changed: analyzer.public_surface_changed(hunk),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use diffthing_core::hunk::hunk_id;

    #[test]
    fn fallback_matches_anything_and_says_unknown() {
        let a = FallbackAnalyzer;
        assert!(a.matches(Path::new("weird/thing.zig")));
        let lines = vec!["+x".to_string()];
        let h = Hunk {
            id: hunk_id("weird/thing.zig", &lines),
            path: "weird/thing.zig".into(),
            new_start: 1,
            old_start: 1,
            added: 1,
            removed: 0,
            lines,
        };
        assert_eq!(a.public_surface_changed(&h), None);
    }
}
