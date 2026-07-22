//! LLM boundary. The single narrow job: propose walkthrough structure from
//! hunk DIGESTS (never full bodies for grouping — shape, not content).
//! Output is serde-deserialized and validator-gated; failure regenerates
//! with violations as feedback, then falls back deterministically.
//!
//! The real provider is the agent CLI the user already uses (`claude -p`,
//! `codex exec`, `gemini -p`) under their existing login — diffthing brings
//! no keys and no provider of its own. `NoopLlm` keeps the loop running
//! end-to-end with zero agents installed: the fallback walkthrough IS the
//! product in degraded mode.

use diffthing_core::hunk::{Hunk, HunkId};
use diffthing_core::schema::{ImpactScore, Scope, Step, Walkthrough, WALKTHROUGH_SCHEMA_VERSION};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// What the model sees per hunk. Deliberately compact: path, symbols-ish
/// first line, counts, score. ~10x cheaper than bodies and makes huge
/// diffs possible rather than lossy.
#[derive(Debug, Serialize)]
pub struct HunkDigest<'a> {
    pub id: &'a HunkId,
    pub path: &'a str,
    pub added: u32,
    pub removed: u32,
    pub impact: &'a ImpactScore,
    pub head: &'a str,
}

pub fn digest<'a>(h: &'a Hunk, score: &'a ImpactScore) -> HunkDigest<'a> {
    HunkDigest {
        id: &h.id,
        path: &h.path,
        added: h.added,
        removed: h.removed,
        impact: score,
        head: h.lines.first().map(|s| s.as_str()).unwrap_or(""),
    }
}

#[allow(async_fn_in_trait)]
pub trait LlmClient: Send + Sync {
    /// Returns None when unavailable — caller falls back deterministically.
    async fn propose_walkthrough(
        &self,
        digests: &[HunkDigest<'_>],
        prior_violations: &[String],
    ) -> Option<Walkthrough>;
}

/// Zero-key scaffold client: always defers to the fallback.
pub struct NoopLlm;

impl LlmClient for NoopLlm {
    async fn propose_walkthrough(
        &self,
        _digests: &[HunkDigest<'_>],
        _prior_violations: &[String],
    ) -> Option<Walkthrough> {
        None
    }
}

/// What the model returns — structure only. Ids, revision, tree_state, and
/// the degraded flag are assigned by our code, never by the model.
#[derive(Debug, Deserialize)]
struct Proposal {
    focus: String,
    scopes: Vec<ProposalScope>,
}

#[derive(Debug, Deserialize)]
struct ProposalScope {
    title: String,
    steps: Vec<ProposalStep>,
}

#[derive(Debug, Deserialize)]
struct ProposalStep {
    title: String,
    framing: String,
    hunks: Vec<String>,
}

fn proposal_to_walkthrough(p: Proposal) -> Walkthrough {
    let scopes = p
        .scopes
        .into_iter()
        .enumerate()
        .map(|(si, s)| Scope {
            id: format!("scope:{si}"),
            title: s.title,
            steps: s
                .steps
                .into_iter()
                .enumerate()
                .map(|(ti, st)| Step {
                    id: format!("step:{si}:{ti}"),
                    title: st.title,
                    framing: st.framing,
                    hunks: st.hunks.into_iter().map(HunkId).collect(),
                })
                .collect(),
        })
        .collect();
    let focus = p.focus.trim();
    Walkthrough {
        schema_version: WALKTHROUGH_SCHEMA_VERSION,
        // Placeholders — `generate` stamps the real values on the validated result.
        revision: 0,
        tree_state: String::new(),
        focus: (!focus.is_empty()).then(|| focus.to_string()),
        scopes,
        degraded: false,
    }
}

const SYSTEM_PROMPT: &str = "\
You organize a code diff into a review walkthrough. You are an organizer, not a reviewer: \
you group, name, and order — you NEVER evaluate quality, correctness, or style, and you \
NEVER approve or criticize. Framing lines are one-sentence descriptions of what changed, \
never judgments.

You receive one JSON object per hunk: id, path, added/removed line counts, a deterministic \
impact score with human-readable reasons, and the hunk's first line. You see shape, not \
content.

Produce a walkthrough as scopes (thematic groups, e.g. a data-contract change, wiring, \
tests, support) containing steps (atomic reading units, each holding hunk ids), plus a \
'focus' field: 1-2 sentences describing the reading order's logic (what shift the order \
tracks, e.g. \"Review order tracks the data contract shift: prompt preparation, then \
consumption, then wiring, then UI behavior.\"). The focus describes the walk — it never \
evaluates the code.

Hard rules — a validator rejects your output if any is violated:
1. Every hunk id appears in exactly one step. No omissions, no duplicates, no invented ids.
2. Order scopes so the highest-impact work comes first. A hunk scored 'highest' must never \
sit in the final scope when there is more than one scope.
3. The final scope must not be a dumping ground holding most of the changed lines.
4. Scope and step titles are short and descriptive of the change's intent.";

fn schema_json() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "focus": { "type": "string" },
            "scopes": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "title": { "type": "string" },
                        "steps": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "title": { "type": "string" },
                                    "framing": { "type": "string" },
                                    "hunks": {
                                        "type": "array",
                                        "items": { "type": "string" }
                                    }
                                },
                                "required": ["title", "framing", "hunks"],
                                "additionalProperties": false
                            }
                        }
                    },
                    "required": ["title", "steps"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["focus", "scopes"],
        "additionalProperties": false
    })
}

/// Agent CLIs we know how to drive, in installed-tool fallback order.
/// The user's tool, the user's login, the user's model choice — diffthing
/// brings no keys and no provider of its own.
///
/// The walkthrough call is pure text-in/JSON-out: hunk digests carry
/// untrusted first lines, so where the CLI can express it, tool use is
/// disabled outright — organization needs no shell, no edits, no network.
const KNOWN_AGENTS: &[(&str, &[&str])] = &[
    // `--disallowedTools` is variadic in the claude CLI: the space-separated
    // form swallows every following argument — including the prompt — as
    // more deny rules. The `=` form binds exactly one value.
    ("claude", &["-p", "--disallowedTools=Bash,Edit,Write,WebFetch,WebSearch"]),
    ("codex", &["exec"]),
    ("gemini", &["-p"]),
    ("kimi", &["-p"]),
    ("qwen", &["--prompt"]),
    ("opencode", &["run"]),
];

pub(crate) fn on_path(bin: &str) -> bool {
    let Some(paths) = std::env::var_os("PATH") else { return false };
    std::env::split_paths(&paths).any(|d| d.join(bin).is_file())
}

/// Agent session that launched this process. Session markers beat PATH:
/// users commonly keep multiple CLIs installed, so installation order says
/// nothing about which agent currently owns the terminal/session.
pub fn detect_session_agent() -> Option<&'static str> {
    if std::env::var_os("CODEX_THREAD_ID").is_some() || std::env::var_os("CODEX_CI").is_some() {
        return Some("codex");
    }
    if std::env::var_os("CLAUDECODE").is_some() {
        return Some("claude");
    }
    if std::env::var_os("GEMINI_CLI").is_some() {
        return Some("gemini");
    }
    None
}

/// Detect one running agent app/CLI from process names. Covers launching
/// diffthing from an unrelated terminal, where session environment markers
/// cannot be inherited. Multiple running agents are ambiguous, so do not
/// guess in that case.
fn detect_running_agent() -> Option<&'static str> {
    let out = std::process::Command::new("ps").args(["-axo", "comm="]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let processes = String::from_utf8_lossy(&out.stdout).to_lowercase();
    let running: Vec<&str> = KNOWN_AGENTS
        .iter()
        .map(|(name, _)| *name)
        .filter(|name| {
            processes.lines().any(|line| {
                let executable = std::path::Path::new(line.trim())
                    .file_name()
                    .and_then(|part| part.to_str())
                    .unwrap_or(line.trim());
                executable == *name || executable.starts_with(&format!("{name} "))
            })
        })
        .collect();
    (running.len() == 1).then_some(running[0])
}

/// Active session/process agent, otherwise first installed agent CLI.
pub fn detect_agent() -> Option<&'static str> {
    detect_session_agent()
        .filter(|name| on_path(name))
        .or_else(|| detect_running_agent().filter(|name| on_path(name)))
        .or_else(|| KNOWN_AGENTS.iter().map(|(name, _)| *name).find(|name| on_path(name)))
}

/// Pull candidate JSON objects out of agent stdout — CLIs wrap answers in
/// prose or code fences, and trailing prose can itself contain braces.
/// First-`{`-to-last-`}` slicing broke exactly there ("trailing characters",
/// "expected `,` or `]`"), so this walks BALANCED objects instead: for each
/// top-level `{`, scan brace depth (string- and escape-aware) to its true
/// closing `}`. The caller tries candidates in order; the validator gate
/// judges whichever parses.
fn extract_json_candidates(out: &str) -> Vec<&str> {
    let bytes = out.as_bytes();
    let mut candidates = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'{' {
            i += 1;
            continue;
        }
        let start = i;
        let mut depth = 0usize;
        let mut in_string = false;
        let mut escaped = false;
        let mut end = None;
        for (j, &b) in bytes.iter().enumerate().skip(start) {
            if in_string {
                if escaped {
                    escaped = false;
                } else if b == b'\\' {
                    escaped = true;
                } else if b == b'"' {
                    in_string = false;
                }
                continue;
            }
            match b {
                b'"' => in_string = true,
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(j);
                        break;
                    }
                }
                _ => {}
            }
        }
        match end {
            Some(j) => {
                candidates.push(&out[start..=j]);
                i = j + 1;
            }
            None => break, // unbalanced tail — no further complete object
        }
    }
    candidates
}

fn build_prompt(digests: &[HunkDigest<'_>], prior_violations: &[String]) -> String {
    let hunks = serde_json::to_string(digests).unwrap_or_else(|_| "[]".into());
    let mut msg = format!(
        "{SYSTEM_PROMPT}\n\nRespond with ONLY a JSON object matching this schema, no prose:\n{}\n\nHunks to organize:\n{hunks}\n",
        schema_json()
    );
    if !prior_violations.is_empty() {
        msg.push_str("\nYour previous attempt was rejected by the validator. Violations:\n");
        for v in prior_violations {
            msg.push_str(&format!("- {v}\n"));
        }
        msg.push_str("Produce a corrected walkthrough.\n");
    }
    msg
}

/// Real provider: shell out to the agent CLI the user already uses
/// (`claude -p`, `codex exec`, `gemini -p`). Runs under their existing
/// login — no keys pass through diffthing. Output goes through the same
/// validator gate as everything else.
pub struct CliLlm {
    program: String,
    args: Vec<String>,
}

impl CliLlm {
    pub fn new(agent: &str) -> Option<Self> {
        let (name, args) = KNOWN_AGENTS.iter().find(|(n, _)| *n == agent)?;
        if !on_path(name) {
            eprintln!("diffthing: agent '{agent}' not found on PATH");
            return None;
        }
        Some(Self { program: name.to_string(), args: args.iter().map(|s| s.to_string()).collect() })
    }

    pub fn agent_name(&self) -> &str {
        &self.program
    }
}

impl LlmClient for CliLlm {
    async fn propose_walkthrough(
        &self,
        digests: &[HunkDigest<'_>],
        prior_violations: &[String],
    ) -> Option<Walkthrough> {
        let prompt = build_prompt(digests, prior_violations);
        let run = tokio::process::Command::new(&self.program)
            .args(&self.args)
            .arg(&prompt)
            .stdin(std::process::Stdio::null())
            .output();
        let out = match tokio::time::timeout(std::time::Duration::from_secs(180), run).await {
            Ok(Ok(out)) => out,
            Ok(Err(e)) => {
                eprintln!("diffthing: {} failed to run: {e}", self.program);
                return None;
            }
            Err(_) => {
                eprintln!("diffthing: {} timed out", self.program);
                return None;
            }
        };
        if !out.status.success() {
            eprintln!(
                "diffthing: {} exited {}: {}",
                self.program,
                out.status,
                String::from_utf8_lossy(&out.stderr)
            );
            return None;
        }
        let stdout = String::from_utf8_lossy(&out.stdout);
        // Agent output may hold several balanced objects (echoed schema,
        // examples, the answer); the first that deserializes as a Proposal
        // wins. Only total failure is reported.
        let candidates = extract_json_candidates(&stdout);
        if candidates.is_empty() {
            eprintln!("diffthing: {} returned no JSON object", self.program);
            return None;
        }
        let mut last_err = None;
        for json in &candidates {
            match serde_json::from_str::<Proposal>(json) {
                Ok(proposal) => return Some(proposal_to_walkthrough(proposal)),
                Err(e) => last_err = Some(e),
            }
        }
        if let Some(e) = last_err {
            eprintln!("diffthing: {} proposal parse failed: {e}", self.program);
        }
        None
    }
}

/// Dispatch without dyn: `LlmClient` has an async method, so it is not
/// object-safe. The session stores this enum instead of a Box<dyn>.
pub enum AnyLlm {
    Noop(NoopLlm),
    Cli(CliLlm),
}

impl AnyLlm {
    /// `choice` comes from config::resolve_agent: None = disabled,
    /// Some("auto") = detect, Some(name) = that agent or degrade.
    pub fn from_choice(choice: Option<String>) -> Self {
        let agent = match choice.as_deref() {
            None => return AnyLlm::Noop(NoopLlm),
            Some("auto") => match detect_agent() {
                Some(a) => a.to_string(),
                None => return AnyLlm::Noop(NoopLlm),
            },
            Some(a) => a.to_string(),
        };
        match CliLlm::new(&agent) {
            Some(cli) => AnyLlm::Cli(cli),
            None => AnyLlm::Noop(NoopLlm),
        }
    }

    pub fn describe(&self) -> String {
        match self {
            AnyLlm::Noop(_) => "none (deterministic fallback)".into(),
            AnyLlm::Cli(c) => format!("{} (your login)", c.agent_name()),
        }
    }

    pub fn agent_name(&self) -> Option<&str> {
        match self {
            AnyLlm::Noop(_) => None,
            AnyLlm::Cli(c) => Some(c.agent_name()),
        }
    }
}

impl LlmClient for AnyLlm {
    async fn propose_walkthrough(
        &self,
        digests: &[HunkDigest<'_>],
        prior_violations: &[String],
    ) -> Option<Walkthrough> {
        match self {
            AnyLlm::Noop(l) => l.propose_walkthrough(digests, prior_violations).await,
            AnyLlm::Cli(l) => l.propose_walkthrough(digests, prior_violations).await,
        }
    }
}

/// Shared generation pipeline: LLM proposal -> validator gate -> retry with
/// feedback -> deterministic fallback. This function is the philosophy in
/// code: the LLM proposes, code verifies, the user never sees an
/// unvalidated walkthrough.
pub async fn generate<L: LlmClient>(
    llm: &L,
    hunks: &[Hunk],
    scores: &BTreeMap<HunkId, ImpactScore>,
    tree_state: &str,
    revision: u64,
    max_retries: usize,
) -> Walkthrough {
    use diffthing_core::validate::{validate, ValidatorConfig};

    let known: BTreeMap<HunkId, u32> =
        hunks.iter().map(|h| (h.id.clone(), h.added + h.removed)).collect();
    let digests: Vec<HunkDigest> =
        hunks.iter().filter_map(|h| scores.get(&h.id).map(|s| digest(h, s))).collect();

    let cfg = ValidatorConfig::default();
    let mut violations_feedback: Vec<String> = Vec::new();

    for _ in 0..=max_retries {
        match llm.propose_walkthrough(&digests, &violations_feedback).await {
            Some(mut w) => {
                let violations = validate(&w, &known, scores, &cfg);
                if violations.is_empty() {
                    w.revision = revision;
                    w.tree_state = tree_state.to_string();
                    w.degraded = false;
                    return w;
                }
                violations_feedback = violations.iter().map(|v| v.to_string()).collect();
            }
            None => break,
        }
    }

    diffthing_core::fallback::build_fallback(hunks, scores, tree_state, revision)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proposal_maps_to_walkthrough() {
        let p: Proposal = serde_json::from_str(
            r#"{"focus":"Order tracks the auth contract shift.","scopes":[{"title":"Auth contract","steps":[
                {"title":"Token expiry","framing":"expiry check tightened","hunks":["h1","h2"]}
            ]},{"title":"Tests","steps":[
                {"title":"Coverage","framing":"new expiry tests","hunks":["h3"]}
            ]}]}"#,
        )
        .unwrap();
        let w = proposal_to_walkthrough(p);
        assert_eq!(w.focus.as_deref(), Some("Order tracks the auth contract shift."));
        assert_eq!(w.scopes.len(), 2);
        assert_eq!(w.scopes[0].title, "Auth contract");
        assert_eq!(w.scopes[0].steps[0].hunks, vec![HunkId("h1".into()), HunkId("h2".into())]);
        assert_eq!(w.scopes[1].steps[0].id, "step:1:0");
        assert!(!w.degraded);
    }

    #[test]
    fn prompt_includes_violations_on_retry() {
        let msg = build_prompt(&[], &["hunk X uncovered".into()]);
        assert!(msg.contains("rejected by the validator"));
        assert!(msg.contains("hunk X uncovered"));
    }

    #[test]
    fn prompt_clean_on_first_attempt() {
        let msg = build_prompt(&[], &[]);
        assert!(!msg.contains("rejected"));
    }

    #[test]
    fn extract_json_strips_prose_and_fences() {
        let out = "Sure, here it is:\n```json\n{\"scopes\":[]}\n```\nDone.";
        assert_eq!(extract_json_candidates(out), vec!["{\"scopes\":[]}"]);
        assert!(extract_json_candidates("no json here").is_empty());
    }

    #[test]
    fn extract_json_survives_braces_in_trailing_prose() {
        // The old first-{-to-last-} slice swallowed the trailing brace and
        // produced "trailing characters" / "expected `,` or `]`" errors.
        let out = "{\"scopes\":[]}\nNote: wrap ids like {this} in production.";
        let candidates = extract_json_candidates(out);
        assert_eq!(candidates[0], "{\"scopes\":[]}");
        assert!(serde_json::from_str::<serde_json::Value>(candidates[0]).is_ok());
    }

    #[test]
    fn extract_json_yields_each_object_when_output_has_several() {
        // Agents sometimes echo the schema before answering.
        let out = "schema: {\"type\":\"object\"}\nanswer:\n{\"focus\":\"f\",\"scopes\":[]}";
        let candidates = extract_json_candidates(out);
        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[1], "{\"focus\":\"f\",\"scopes\":[]}");
    }

    #[test]
    fn extract_json_ignores_braces_inside_strings() {
        let out = r#"{"focus":"touches {a} and \"b\"","scopes":[]} tail }"#;
        let candidates = extract_json_candidates(out);
        assert_eq!(candidates, vec![r#"{"focus":"touches {a} and \"b\"","scopes":[]}"#]);
    }

    #[test]
    fn extract_json_skips_unbalanced_tail() {
        let out = "{\"scopes\":[]} then broken {\"oops\": ";
        assert_eq!(extract_json_candidates(out), vec!["{\"scopes\":[]}"]);
    }

    #[test]
    fn from_choice_none_is_noop() {
        assert!(matches!(AnyLlm::from_choice(None), AnyLlm::Noop(_)));
    }

    #[test]
    fn from_choice_unknown_agent_degrades_to_noop() {
        assert!(matches!(
            AnyLlm::from_choice(Some("definitely-not-installed-xyz".into())),
            AnyLlm::Noop(_)
        ));
    }
}
