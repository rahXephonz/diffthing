//! Deterministic impact scoring. NO LLM anywhere in this file, ever.
//! Every score must be reproducible and carry human-readable reasons.
//!
//! Signals (v1): path priors, change class heuristics, size tiebreak,
//! lockfile/new-dependency detection. Fan-in and public-surface delta are
//! injected by the analyzers crate via `ExternalSignals` — this keeps core
//! free of any language-specific parsing.

use crate::hunk::Hunk;
use crate::schema::{Impact, ImpactScore};

/// Signals computed by ecosystem analyzers (fan-in, API surface).
/// `None` means "unknown", not "zero" — the fallback analyzer supplies None.
#[derive(Debug, Clone, Default)]
pub struct ExternalSignals {
    pub importer_count: Option<u32>,
    pub public_surface_changed: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct PathPrior {
    pub pattern: &'static str,
    pub points: i32,
    pub label: &'static str,
}

/// Sane defaults; per-repo overrides land in `.diffthing/config` later.
pub const DEFAULT_PRIORS: &[PathPrior] = &[
    PathPrior { pattern: "auth", points: 30, label: "auth path" },
    PathPrior { pattern: "payment", points: 30, label: "payment path" },
    PathPrior { pattern: "pay/", points: 30, label: "payment path" },
    PathPrior { pattern: "security", points: 30, label: "security path" },
    PathPrior { pattern: "migration", points: 25, label: "migration" },
    PathPrior { pattern: "src/core", points: 20, label: "core module" },
    PathPrior { pattern: ".github/workflows", points: 20, label: "CI workflow" },
    PathPrior { pattern: "__tests__", points: -25, label: "test file" },
    PathPrior { pattern: ".test.", points: -25, label: "test file" },
    PathPrior { pattern: "_test.", points: -25, label: "test file" },
    PathPrior { pattern: ".spec.", points: -25, label: "test file" },
    PathPrior { pattern: ".stories.", points: -30, label: "storybook" },
    PathPrior { pattern: "__snapshots__", points: -35, label: "snapshot" },
    PathPrior { pattern: "fixtures", points: -25, label: "fixture" },
];

const LOCKFILES: &[&str] = &[
    "package-lock.json", "pnpm-lock.yaml", "yarn.lock", "Cargo.lock",
    "mix.lock", "poetry.lock", "bun.lockb", "bun.lock",
];

fn is_lockfile(path: &str) -> bool {
    LOCKFILES.iter().any(|l| path.ends_with(l))
}

/// Grep-grade change-class heuristic on hunk body. AST precision is a v2
/// refinement behind the analyzers trait; this catches the 90% case.
fn control_flow_touched(hunk: &Hunk) -> bool {
    const KEYWORDS: &[&str] = &[
        "if ", "if(", "else", "match ", "case ", "switch", "for ", "for(",
        "while ", "while(", "return", "throw ", "raise ", "await ", "async ",
        "catch", "rescue", "unwrap", "?.", "&&", "||",
    ];
    hunk.lines
        .iter()
        .filter(|l| l.starts_with('+') || l.starts_with('-'))
        .any(|l| KEYWORDS.iter().any(|k| l.contains(k)))
}

fn is_declarative_only(hunk: &Hunk) -> bool {
    let p = hunk.path.as_str();
    p.ends_with(".css") || p.ends_with(".scss") || p.ends_with(".md")
        || p.ends_with(".txt") || p.ends_with(".svg") || p.ends_with(".json")
}

fn adds_dependency(hunk: &Hunk) -> bool {
    if !is_lockfile(&hunk.path) && !hunk.path.ends_with("package.json")
        && !hunk.path.ends_with("Cargo.toml") && !hunk.path.ends_with("mix.exs")
    {
        return false;
    }
    hunk.lines.iter().any(|l| {
        l.starts_with('+')
            && (l.contains("resolved") || l.contains("version") || l.contains("integrity")
                || l.contains(" = \"") || l.contains("\": \""))
    })
}

pub fn score_hunk(hunk: &Hunk, ext: &ExternalSignals) -> ImpactScore {
    let mut points: i32 = 0;
    let mut reasons: Vec<String> = Vec::new();

    // Hard floors first.
    if ext.public_surface_changed == Some(true) {
        points += 60;
        reasons.push("public API surface changed".into());
    }
    if let Some(n) = ext.importer_count {
        if n > 0 {
            // log2-scaled fan-in.
            let fan = (32 - n.leading_zeros()) * 8;
            points += fan as i32;
            reasons.push(format!("imported by {n} file(s)"));
        }
    }

    if is_lockfile(&hunk.path) {
        if adds_dependency(hunk) {
            points += 55;
            reasons.push("new dependency added".into());
        } else {
            points -= 30;
            reasons.push("lockfile churn".into());
        }
    }

    for prior in DEFAULT_PRIORS {
        if hunk.path.contains(prior.pattern) {
            points += prior.points;
            reasons.push(prior.label.to_string());
            break; // strongest matching prior only, keep reasons readable
        }
    }

    if is_declarative_only(hunk) {
        points -= 15;
        reasons.push("declarative change".into());
    } else if control_flow_touched(hunk) {
        points += 25;
        reasons.push("control flow edited".into());
    }

    // Size is a tiebreaker ONLY — capped so it can never dominate.
    let size = (hunk.added + hunk.removed).min(200);
    points += (size / 40) as i32; // max +5

    let points_u = points.max(0) as u32;
    let impact = match points_u {
        0..=19 => Impact::Low,
        20..=44 => Impact::Medium,
        45..=79 => Impact::High,
        _ => Impact::Highest,
    };
    if reasons.is_empty() {
        reasons.push("no strong signals".into());
    }
    ImpactScore { impact, points: points_u, reasons }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hunk::hunk_id;

    fn mk(path: &str, lines: &[&str]) -> Hunk {
        let lines: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
        Hunk {
            id: hunk_id(path, &lines),
            path: path.into(),
            new_start: 1,
            old_start: 1,
            added: lines.iter().filter(|l| l.starts_with('+')).count() as u32,
            removed: lines.iter().filter(|l| l.starts_with('-')).count() as u32,
            lines,
        }
    }

    #[test]
    fn exported_api_change_with_fanin_is_highest() {
        let h = mk("src/payment/charge.ts", &["+if (amount <= 0) throw new Error()"]);
        let ext = ExternalSignals { importer_count: Some(23), public_surface_changed: Some(true) };
        let s = score_hunk(&h, &ext);
        assert_eq!(s.impact, Impact::Highest);
        assert!(s.reasons.iter().any(|r| r.contains("public API")));
        assert!(s.reasons.iter().any(|r| r.contains("23")));
    }

    #[test]
    fn tiny_auth_control_flow_beats_big_css() {
        let auth = mk("src/auth/session.ts", &["+if (!token) return deny()"]);
        let css: Vec<&str> = std::iter::repeat("+.btn { color: red }").take(300).collect::<Vec<_>>();
        let css = mk("src/styles/theme.css", &css);
        let none = ExternalSignals::default();
        let sa = score_hunk(&auth, &none);
        let sc = score_hunk(&css, &none);
        assert!(sa.points > sc.points, "2-line auth must outrank 300-line CSS");
    }

    #[test]
    fn snapshots_score_low() {
        let h = mk("src/__snapshots__/App.test.tsx.snap", &["+<div>hi</div>"]);
        let s = score_hunk(&h, &ExternalSignals::default());
        assert_eq!(s.impact, Impact::Low);
    }

    #[test]
    fn scoring_is_deterministic() {
        let h = mk("src/core/engine.ts", &["+return compute(x)"]);
        let a = score_hunk(&h, &ExternalSignals::default());
        let b = score_hunk(&h, &ExternalSignals::default());
        assert_eq!(a.points, b.points);
        assert_eq!(a.reasons, b.reasons);
    }
}
