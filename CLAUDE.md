# CLAUDE.md — diffthing

> Your agent writes the code. You still own the judgment.

Local-first code review for the agent era. A Rust daemon on the developer's
machine + a hosted SPA (`local.diffthing.dev`) that connects back over a
token-gated WebSocket — the Drizzle Studio pattern.

## The one rule that defines this product

**AI organizes and executes. Only the human reviews.**
The LLM's only job is structuring the diff into a walkthrough (grouping,
naming, reading order). It never evaluates, never comments on quality,
never approves. Agents may be dispatched to _execute_ the human's judgment
(fix flags), and reconciliation _verifies_ their claims — but no judgment
ever comes from a machine. If a feature request would make the AI judge
code, the answer is no.

## Architecture invariants (do not violate)

1. **LLM proposes, code verifies.** Every LLM output passes the validator
   gate (`core/validate.rs`): full hunk coverage, exclusive assignment, no
   hallucinated hunks, no dumping-ground final scope, no `highest` hunk
   buried last. Fail → regenerate with violations as feedback → after
   retries, deterministic fallback (`core/fallback.rs`). The user NEVER
   sees an unvalidated walkthrough.
2. **Hunk identity = content hash** (path + normalized body, `core/hunk.rs`).
   Review state keys off this, never off ordinals or step positions.
3. **Reconciliation applies automatically, integrity is enforced by
   invariant 4, not by gating the snapshot.** Background changes apply the
   moment they're reconciled — no manual "Apply" step. What used to guard
   the reader (withholding the snapshot) is now the honesty rules below:
   a hunk you already viewed that changes underneath you flips to
   `ChangedSinceViewed` and re-enters the queue, so nothing is silently
   lost even though the screen can move.
4. **Honesty rules** (`core/reconcile.rs`): viewed hunk changed →
   `ChangedSinceViewed`, re-enters queue. Deleted flagged hunk → tombstone,
   comment preserved. Agent-changed flag → `addressed_claim` only; closing
   a flag is a human click, always.
5. **Impact scoring is deterministic** (`core/score.rs`). No LLM. Every
   score carries human-readable reasons. Size is a tiebreaker, capped.
6. **Security floor:** daemon binds 127.0.0.1 only; token in URL FRAGMENT
   (never query string — fragments don't hit servers/logs); Origin
   allowlist; protocol version handshake is message #1. This is the
   defense against malicious tabs dialing localhost.
7. **Analyzers are deterministic plugins** (`crates/analyzers`).
   Language-agnostic by architecture, language-aware by plugin. The
   fallback analyzer means ANY repo works day one (graceful degradation,
   never "unsupported language").
8. **Git = shell out to the git binary.** Never libgit2, never reimplement.
9. **Anchored dispatch only.** `RequestChange` always attaches to hunks.
   Never build a free-floating chat box — that's a worse IDE, not a review
   tool.

## Repo layout

- `crates/core` — pure logic, zero IO. Schema, parser, scorer, reconciler,
  validator, protocol, fallback. This is where test discipline lives
  (22 tests and counting — keep the bar).
- `crates/analyzers` — `Analyzer` trait + `FallbackAnalyzer`. tree-sitter
  analyzers land here (deps pre-listed, commented, in Cargo.toml).
- `crates/daemon` — CLI (`clap`), axum WS server, notify watcher
  (debounce-to-quiescence 2s), git IO, LLM boundary (`NoopLlm` today).
- `web` — Vite/React/TS SPA. Connection state machine
  (connecting → probing → diagnosed → connected / session_ended) actively
  DIAGNOSES failures via /health probe — never an eternal spinner, never a
  wall of maybe-causes (this is the anti-Drizzle-Studio design decision).
- `web/src/protocol.ts` — hand-written mirror of `core/protocol.rs`.
  Replace with ts-rs codegen (M1). Until then: change both or change none.

## Toolchain notes

- Built against Rust 1.75 (Ubuntu apt) — Cargo.lock pins some transitive
  crates below edition2024 versions. On a modern rustup toolchain you can
  `cargo update` freely; keep `rust-version` honest in Cargo.toml.
- pnpm v10 blocks postinstall scripts: run `pnpm approve-builds` once
  (esbuild) or keep `onlyBuiltDependencies` in pnpm-workspace.yaml.
- Verify loop: `cargo test --workspace` + `cd web && pnpm build`.

## Milestones

**M1 — close the core loop (next)**

- [ ] Real LLM provider behind `LlmClient` (reqwest, BYO key from
      `~/.config/diffthing`, structured output → serde → validator gate).
      Prompt gets hunk DIGESTS (shape, not content) + impact scores +
      "order highest first" instruction + prior violations on retry.
- [ ] Incremental assignment on ApplyUpdate: carried/changed hunks keep
      their step; new hunks in claimed files inherit the step; only orphan
      hunks go to a small scoped LLM call. Existing step ORDER never
      reshuffles automatically (stability beats optimality).
- [ ] rusqlite persistence in `.diffthing/` (review state, timeline,
      dispatch log; gitignored). Resume-on-same-branch.
- [ ] ts-rs codegen replacing protocol.ts mirror.
- [ ] Embed SPA build into the binary (`include_dir`/`rust-embed`) so
      `--offline` serves it; pass real port into the Origin check
      (server.rs TODO).
- [ ] Timeline view: iteration N — flags addressed/untouched.

**M2 — analyzers + dispatch**

- [ ] TS/JS analyzer: tree-sitter module graph (fan-in) + export surface
      delta; incremental invalidation from watcher events.
- [ ] Agent dispatch (`RequestChange`): git snapshot (`gitio::snapshot`) →
      single-writer lock → runner adapter (`claude -p`, `aider --message`,
      `codex exec`, `gemini -p`; detect installed) → timeout+kill+revert →
      scope validation ("agent modified N unflagged files ⚠") →
      results flow through the SAME watcher→reconcile pipeline (no special
      code path — this is by design).
- [ ] Review export polish + per-runner prompt templates.

**M3 — breadth**

- [ ] Rust + Elixir analyzers (parse-level: pub surface; fan-in later).
- [ ] `--base` branch mode UX; PR mode design.
- [ ] MCP server exposing get_open_flags/mark_addressed (claims still
      verified by reconciliation).

**M4 — differentiation**

- [ ] Review-against-intent: `.diffthing/intent.toml` constraints
      (rendering strategy, bundle budget, token-only styling, dep
      allowlist) → violating hunks surface first with the violated rule as
      the reason string.
- [ ] Solidity domain analyzer (external/public surface, storage layout,
      delegatecall paths, access modifiers) — the premium wedge.
- [ ] Review memory: past decisions resurface on pattern reappearance.

## Positioning guardrails

- Never adopt "AI reviews your code" framing — trust decay is the disease
  we're the alternative to.
- Vocabulary is ours, not DiffDash's. Tagline pair: "Your agent writes the
  code. You still own the judgment." / "AI structures the diff. Never
  reviews it."
- Buyers who care: senior engineers, local-first crowd, security-conscious
  teams. They WILL audit the localhost security — invariant 6 is marketing.

<!-- code-review-graph MCP tools -->

## MCP Tools: code-review-graph

**IMPORTANT: This project has a knowledge graph. ALWAYS use the
code-review-graph MCP tools BEFORE using Grep/Glob/Read to explore
the codebase.** The graph is faster, cheaper (fewer tokens), and gives
you structural context (callers, dependents, test coverage) that file
scanning cannot.

### When to use graph tools FIRST

- **Exploring code**: `semantic_search_nodes` or `query_graph` instead of Grep
- **Understanding impact**: `get_impact_radius` instead of manually tracing imports
- **Code review**: `detect_changes` + `get_review_context` instead of reading entire files
- **Finding relationships**: `query_graph` with callers_of/callees_of/imports_of/tests_for
- **Architecture questions**: `get_architecture_overview` + `list_communities`

Fall back to Grep/Glob/Read **only** when the graph doesn't cover what you need.

### Key Tools

| Tool                        | Use when                                               |
| --------------------------- | ------------------------------------------------------ |
| `detect_changes`            | Reviewing code changes — gives risk-scored analysis    |
| `get_review_context`        | Need source snippets for review — token-efficient      |
| `get_impact_radius`         | Understanding blast radius of a change                 |
| `get_affected_flows`        | Finding which execution paths are impacted             |
| `query_graph`               | Tracing callers, callees, imports, tests, dependencies |
| `semantic_search_nodes`     | Finding functions/classes by name or keyword           |
| `get_architecture_overview` | Understanding high-level codebase structure            |
| `refactor_tool`             | Planning renames, finding dead code                    |

### Workflow

1. The graph auto-updates on file changes (via hooks).
2. Use `detect_changes` for code review.
3. Use `get_affected_flows` to understand impact.
4. Use `query_graph` pattern="tests_for" to check coverage.
