# diffthing

> Your agent writes code. You still own judgment.

Local-first diff review for AI-assisted development. diffthing turns working-tree changes into a prioritized walkthrough, keeps review state stable while agents continue editing, and stages files only after human approval.

AI organizes and executes. It never approves code.

## Why diffthing

Agent-generated changes are fast, wide, and difficult to review as one flat patch. diffthing adds a review loop built for that workflow:

- Groups related hunks into named scopes and ordered steps.
- Scores impact deterministically; LLMs never decide risk or correctness.
- Tracks viewed, changed-since-viewed, commented, and resolved state by content hash.
- Reconciles live edits without silently preserving stale approval.
- Sends anchored questions or change requests back to your active coding agent.
- Uses Git index as approval ledger: fully reviewed files move to staged changes.
- Runs on `127.0.0.1` with token-gated WebSocket access.

## Status

Early development. Install via npm or run from source.

## Install

Run inside any Git repository:

```bash
npx diffthing
```

```text
  diffthing 0.1.0
  reviewing /path/to/project against HEAD
  llm       claude (your login)
  ✓ ready   0 files, 0 changes, 1 AI-organized scopes

  open  https://local.diffthing.dev:58826/#port=58826&token=…
```

Open the printed URL. Or install globally:

```bash
npm install -g diffthing
diffthing
```

The npm build ships a prebuilt binary with the review UI embedded. It serves
the UI over **HTTPS via `local.diffthing.dev`**, a domain whose public DNS
resolves to `127.0.0.1` — so the page loads from your own machine and the
WebSocket is same-origin (no mixed content, no Local Network Access prompt).
On first run the daemon generates a per-install self-signed certificate; your
browser asks to trust it once, then every later run is silent. See
[How local.diffthing.dev works](docs/LOCAL_DOMAIN.md).

If DNS or the certificate can't be reached (locked-down networks, air-gapped),
fall back to plain HTTP on loopback:

```bash
npx diffthing --offline
```

## Quick start (from source)

Requirements:

- Rust 1.75+
- Node.js 20+
- pnpm 10+
- Git

```bash
git clone https://github.com/rahXephonz/diffthing.git
cd diffthing
pnpm install
pnpm web:build
cargo run -p diffthing -- --offline --repo /path/to/project
```

diffthing prints a loopback URL and opens a review session for uncommitted changes against `HEAD`.

Run from target repository instead:

```bash
cd /path/to/project
cargo run --manifest-path /path/to/diffthing/Cargo.toml -p diffthing -- --offline
```

## Agent support

diffthing uses coding-agent CLIs already installed and authenticated on your machine. No provider key is stored by diffthing.

| Agent       | CLI value  | Walkthrough | Ask agent |
| ----------- | ---------- | :---------: | :-------: |
| Claude Code | `claude`   |      ✓      |     ✓     |
| Codex CLI   | `codex`    |      ✓      |     ✓     |
| Gemini CLI  | `gemini`   |      ✓      |     ✓     |
| Kimi CLI    | `kimi`     |      ✓      |     ✓     |
| Qwen Code   | `qwen`     |      ✓      |     ✓     |
| OpenCode    | `opencode` |      ✓      |     ✓     |

Automatic selection prefers active session markers, then configured agent, then one installed CLI. Force selection with `--llm`:

```bash
diffthing --offline --llm codex
diffthing --offline --llm none
```

Persistent configuration lives at `~/.config/diffthing/config.toml`:

```toml
[llm]
agent = "codex"
```

`none` disables LLM organization and uses deterministic file-order fallback.

## Review workflow

1. Start diffthing inside or against a Git repository.
2. Read scopes in suggested order.
3. Mark hunks viewed or attach Markdown comments to specific lines.
4. Use **Ask agent** for questions and explicit change requests. Questions receive answers without file edits.
5. Resolve open comments after verifying results.
6. When every hunk in a file is viewed and no open comment remains, diffthing stages that file and removes it from active review.
7. New edits reappear automatically. Previously viewed changed hunks return as `changed since viewed`.

## CLI

```text
diffthing [OPTIONS]

--base <BASE>  Diff base; default HEAD
--offline      Serve over plain HTTP on 127.0.0.1 instead of HTTPS via
               local.diffthing.dev
--port <PORT>  Fixed port; default first free port
--repo <REPO>  Repository root; default current directory
--llm <LLM>    claude | codex | gemini | kimi | qwen | opencode | none | auto
```

## Built with Codex and GPT-5.6

Codex, powered by GPT-5.6, was used as a development partner while building diffthing.

It helped us:

- Map the Rust daemon, WebSocket protocol, and React review interface before making changes.
- Implement focused UI and workflow changes, including direct hunk-comment dispatch to an agent.
- Trace review-state and dispatch flows to preserve the product boundary: AI may organize or execute an explicit request, but only a human can approve code or resolve a review thread.
- Run focused verification such as Rust tests, web linting, and production builds.

Codex was used to accelerate implementation and analysis. It is not the authority that approves code quality or review decisions; diffthing keeps those decisions with the developer.

## Architecture

```text
Git working tree
      │
      ▼
Rust daemon ── parse → score → organize → validate → reconcile
      │                                      │
      └──────── token-gated WebSocket ───────┘
                         │
                         ▼
                  React review UI
```

- `crates/core` — diff model, scoring, validation, reconciliation, review state, protocol.
- `crates/analyzers` — deterministic language analysis with universal fallback.
- `crates/daemon` — CLI, Git integration, agent runners, watcher, WebSocket server.
- `web` — React/Vite review interface.
- `.agents/skills` — vendor-neutral agent workflows.

Protocol TypeScript types are generated from Rust using `ts-rs`:

```bash
pnpm protocol:generate
```

Detailed invariants: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Security model

- Daemon binds only to `127.0.0.1`, in both HTTPS and `--offline` modes.
- Session token stays in URL fragment, not query string or server logs.
- WebSocket handshake validates token, origin, and protocol version.
- Source leaves machine only through agent CLI selected by user.
- Agent edits are reconciled into same review pipeline as manual edits.
- Diff content is treated as untrusted in agent prompts: hunk bodies are
  fenced with a per-run random boundary, and runners are invoked with the
  narrowest capabilities their CLI supports (shell/network tools disabled
  for `claude`, OS sandbox for `codex`). Out-of-scope edits are rolled back
  after every run. Details:
  [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md#agent-trust-boundary).
- `local.diffthing.dev` resolves only to loopback, and its TLS certificate is
  **generated per install** (cached in `~/.config/diffthing/tls`, key `0600`).
  No private key ships in releases or source control, so no shared key exists
  for an attacker to combine with DNS/hosts manipulation. The session token
  and loopback bind remain the real access controls.
  Details: [docs/LOCAL_DOMAIN.md](docs/LOCAL_DOMAIN.md).

## Development

```bash
pnpm install
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
pnpm web:lint
pnpm web:build
```

Useful commands:

```bash
pnpm web:dev
pnpm protocol:generate
cargo run -p diffthing -- --offline
```

AI contributors should read [AGENTS.md](AGENTS.md). Claude, Gemini, Copilot, and Cursor adapters point to same project rules.

## Roadmap

Direction and priorities live in [ROADMAP.md](ROADMAP.md). Near-term focus:
persistent review state, a smoother connect flow, and a VS Code extension.

## Contributing

Issues and focused pull requests are welcome. Read [CONTRIBUTING.md](CONTRIBUTING.md) and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) before contributing. Preserve core boundary: AI may organize changes and execute explicit requests; human reviewer owns approval.

## License

[MIT](LICENSE).
