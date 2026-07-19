//! LLM agent selection. The user's choice, always — diffthing never brings
//! its own provider or key. Resolution order:
//!   1. --llm flag (claude | codex | gemini | kimi | qwen | opencode | none | auto)
//!   2. active agent session inherited from environment
//!   3. ~/.config/diffthing/config.toml  ([llm] agent = "claude")
//!   4. auto-detect: first installed agent CLI on PATH
//!
//! Nothing found -> NoopLlm -> deterministic fallback walkthrough
//! (degraded mode, shown honestly in the UI).

use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Default, Deserialize)]
struct ConfigFile {
    #[serde(default)]
    llm: LlmSection,
}

#[derive(Debug, Default, Deserialize)]
struct LlmSection {
    agent: Option<String>,
}

fn config_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("diffthing").join("config.toml"))
}

fn parse(contents: &str) -> ConfigFile {
    toml::from_str(contents).unwrap_or_else(|e| {
        eprintln!("diffthing: ignoring malformed config.toml: {e}");
        ConfigFile::default()
    })
}

/// Resolve the agent name the user wants: flag beats config; "auto"/absent
/// means detect. Returns None for explicit "none".
pub fn resolve_agent(flag: &str) -> Option<String> {
    match flag {
        "none" => None,
        "auto" => {
            if let Some(active) = crate::llm::detect_session_agent() {
                return Some(active.to_string());
            }
            let file = config_path()
                .and_then(|p| std::fs::read_to_string(p).ok())
                .map(|s| parse(&s))
                .unwrap_or_default();
            match file.llm.agent.as_deref() {
                Some("none") => None,
                Some(a) => Some(a.to_string()),
                None => Some("auto".to_string()),
            }
        }
        other => Some(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_agent() {
        let c = parse("[llm]\nagent = \"claude\"\n");
        assert_eq!(c.llm.agent.as_deref(), Some("claude"));
    }

    #[test]
    fn missing_section_defaults() {
        assert!(parse("").llm.agent.is_none());
    }

    #[test]
    fn malformed_toml_degrades_to_default() {
        assert!(parse("[llm\nagent=").llm.agent.is_none());
    }

    #[test]
    fn explicit_flag_beats_everything() {
        assert_eq!(resolve_agent("codex").as_deref(), Some("codex"));
        assert_eq!(resolve_agent("none"), None);
    }
}
