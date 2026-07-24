# AI contributor guide

Applies to every coding agent working in this repository: Claude Code, Codex, Gemini CLI, Kimi, Qwen Code, OpenCode, Copilot, Cursor, and future compatible tools.

## Product boundary

AI organizes and executes. Human reviews.

- Never make AI approve, reject, or judge code quality.
- Impact scoring stays deterministic.
- Walkthrough structure from LLM must pass code validation.
- Agent claims never resolve comments automatically.
- Questions must receive answers without edits unless user explicitly requests code changes.

Read [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) before changing protocol, reconciliation, dispatch, Git staging, or security behavior.

## Working rules

- Preserve user changes and unrelated dirty files.
- For debug, explore, refactor, or review tasks, invoke the matching `.agents/skills` workflow (`debug-issue`, `explore-codebase`, `refactor-safely`, `review-changes`) — they drive the `code-review-graph` MCP for token-cheap, impact-aware navigation. Fall back to `rg` when graph data is missing or stale.
- Use `rg` for search.
- Keep protocol source in Rust. Regenerate TypeScript with `pnpm protocol:generate`.
- Add tests for core state transitions and bug fixes.
- Run focused tests during work; run full verification before handoff.
- Do not commit unless user asks.
- Never weaken loopback binding, token validation, origin validation, or protocol handshake.

## Verification

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
pnpm web:lint
pnpm web:build
```

## Repository map

- `crates/core` — pure review logic and wire model.
- `crates/analyzers` — deterministic analyzers.
- `crates/daemon` — IO, Git, agents, watcher, server.
- `web` — review UI.
- `web/src/libs/generated` — generated; never hand-edit.
- `.agents/skills` — reusable vendor-neutral workflows.

## Communication style

Respond terse like smart caveman. Technical substance stays; fluff dies.

- Drop filler and unnecessary pleasantries.
- Fragments OK. Technical terms exact. Code, commits, and PR text use normal language.
- Pattern: `[thing] [action] [reason]. [next step].`
- Drop caveman style for security warnings, irreversible actions, or user confusion.
- User controls: `/caveman lite|full|ultra|wenyan`; `stop caveman` or `normal mode` disables it.
