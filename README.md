# diffthing

> Your agent writes the code. You still own the judgment.

Local-first code review for the agent era. AI structures your diff into a
prioritized walkthrough — grouped scopes, impact-scored hunks, a reading
order that puts load-bearing changes first. **It never reviews.** Every
judgment (viewed / flag / comment) is human, and stays on your machine.

```
npx diffthing        # (distribution wrapper lands in M1 — for now: cargo run)
  open https://local.diffthing.dev/#port=4821&token=…
```

- **Live-sync:** your agent keeps writing while you read; changes reconcile
  by content hash and arrive as an "Apply" banner — never a reflow under
  your cursor. Hunks you viewed that later changed are honestly downgraded.
- **Deterministic impact scoring:** low → highest, with reasons
  ("exported signature changed, imported by 23 files, payment path").
  No LLM anywhere in scoring.
- **Validator-gated walkthroughs:** the LLM proposes structure; code
  verifies coverage, exclusivity, and ordering invariants. Fallback is a
  deterministic walkthrough — the tool works with zero API keys.
- **Anchored agent dispatch (M2):** flag a hunk, type the fix instruction,
  dispatch to Claude Code / aider / Codex CLI / Gemini CLI — snapshot
  first, scope-validated after, verified by reconciliation.
- **Local-first for real:** daemon binds 127.0.0.1, token-gated WS,
  origin allowlist. Your code never leaves your machine except to the LLM
  provider you configure.

## Dev

```bash
cargo test --workspace     # core logic tests (parser/scorer/reconciler/validator)
cargo run -p diffthing     # boot the daemon in any git repo
cd web && pnpm install && pnpm dev   # SPA (pnpm approve-builds once for esbuild)
```

Architecture, invariants, and the milestone plan live in [CLAUDE.md](./CLAUDE.md).
Protocol source of truth: `crates/core/src/protocol.rs`.
