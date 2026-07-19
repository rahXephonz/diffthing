# Architecture and invariants

diffthing is a local Rust daemon plus React SPA. Daemon reads Git changes, builds a validated walkthrough, tracks human review state, and streams revisions over a token-gated WebSocket.

## Product rule

AI organizes changes and executes explicit user requests. Human reviewer owns every judgment and approval.

## Invariants

1. **LLM proposes; code verifies.** Walkthrough output must cover every hunk exactly once, reference only real hunks, and satisfy ordering rules. Invalid output retries, then falls back to deterministic file order.
2. **Hunk identity uses content.** Review state keys by hash of path and normalized body, never UI position.
3. **Reconciliation preserves honesty.** Viewed hunk changed underneath reviewer becomes `changed_since_viewed`. Removed flagged hunk keeps comment history.
4. **Agent claims do not approve.** Agent response or change claim becomes thread entry. Human resolves thread.
5. **Git index is approval ledger.** File stages only when all current hunks are viewed and no open flag remains. Staged-only files leave active review; later worktree edits return.
6. **Impact scoring is deterministic.** LLM cannot assign risk or correctness.
7. **Protocol source is Rust.** `ts-rs` exports web bindings. Change Rust model, run `pnpm protocol:generate`, then update consumers.
8. **Security floor stays intact.** Loopback-only bind, token in fragment, origin allowlist, version handshake first.
9. **Git uses system binary.** Do not replace behavior with libgit2 or custom repository semantics.
10. **Dispatch stays anchored.** Agent interaction belongs to hunk comments. No free-floating general chat.

## Data flow

```text
git diff
  → parse hunks
  → deterministic scores
  → LLM or fallback walkthrough
  → validator
  → session snapshot
  → WebSocket UI

filesystem change
  → debounce
  → new diff
  → reconcile hashes and lineage
  → publish revision
```

## Components

### Core

`crates/core` contains IO-free domain logic: hunk identity, schemas, scoring, validation, fallback assignment, reconciliation, review threads, and protocol types.

### Analyzers

`crates/analyzers` enriches deterministic scoring. Universal fallback keeps unknown languages usable.

### Daemon

`crates/daemon` owns Git subprocesses, configuration, CLI-agent invocation, session mutation, file watching, embedded assets, HTTP, and WebSocket transport.

Agent selection order:

1. Explicit `--llm`.
2. Active-session environment marker.
3. `~/.config/diffthing/config.toml`.
4. Installed supported CLI.
5. Deterministic fallback.

### Web

`web` renders scopes, unified/split diffs, comments, Markdown preview, dispatch state, and revision updates. It does not recompute domain decisions.

## Change checklist

- Protocol: bump version when wire compatibility changes; regenerate bindings.
- Reconciliation: test carried, changed, removed, and new hunks.
- Review state: test comments, resolution, viewed state, and staging gate.
- Dispatch: distinguish response-only runs from edited-tree claims.
- Security: test rejected token, origin, and protocol mismatch.
- UI: test or manually verify unified and split layouts.
