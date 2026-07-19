# Contributing to diffthing

Thanks for helping improve diffthing.

## Before opening a change

- Search existing issues and pull requests.
- Keep changes focused on one behavior.
- Discuss large features or architecture changes first.
- Read [AGENTS.md](AGENTS.md) when using a coding agent.
- Preserve invariants in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Development setup

```bash
pnpm install
pnpm web:build
cargo test --workspace
```

## Verification

Run before opening a pull request:

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
pnpm web:lint
pnpm web:build
```

Protocol changes require regenerated bindings:

```bash
pnpm protocol:generate
```

Commit generated output with source-model change.

## Pull requests

Include:

- Problem and intended behavior.
- Implementation summary.
- Tests or manual verification.
- Screenshots for visible UI changes.
- Compatibility or security implications.

AI-generated work follows same bar as human-written work. Author remains responsible for understanding and verifying every change.

By participating, you agree to [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).
