# diffthing roadmap

Direction for turning diffthing from a working prototype into a tool developers
reach for on every AI-assisted change. Priorities are ordered by adoption
impact, not by effort.

The product boundary never changes: **AI organizes and executes; the human
reviews and approves.** Nothing below weakens that.

## Where it stands

- Strong core: deterministic scoring, validation, reconciliation, and a
  generated wire protocol, well covered by tests in `crates/core`.
- Shipped: npm distribution (`npx diffthing`), per-platform prebuilt binaries,
  and trusted local HTTPS via `local.diffthing.dev`.
- Thin edges: no persistence across restarts, little daemon/CLI integration
  testing, no editor integration, and a manual connect step. These — not the
  core — are what hold back real adoption.

## Now / next: v0.2 — "usable daily"

1. **Persist review state** (highest priority). Viewed / commented / resolved
   state currently lives in memory, so restarting the daemon loses it — which
   breaks the core promise of stable review state while agents keep editing.
   Land the `.diffthing/` SQLite store noted in `crates/daemon/Cargo.toml`.
   - Schema keyed by content hash (survives line shifts).
   - Write on every state transition; load on boot and reconcile against the
     current diff.
   - Version the store; migrate or discard cleanly on protocol bumps.
2. **Connect UX.** Auto-open the browser to the printed URL on start, and use a
   fixed default port (`4983`, falling back if busy) for a stable, bookmarkable
   `https://local.diffthing.dev:4983`. Removes the bare-domain confusion.
3. **Robustness pass.** Daemon/CLI integration tests (boot, WebSocket
   handshake, reconciliation) — currently `main`, `serve`, and `ServeMode` are
   untested — plus a real Windows runtime smoke test (cross-compiled today,
   never actually run).

## v0.3 — "fits my workflow"

4. **VS Code extension.** Open a review from the command palette, jump to
   `file:line`, comment inline. Meets developers where they already work; likely
   the single biggest reach-expander.
5. **Commit / PR flow.** After approval, generate the commit message via the
   user's agent; optionally open a PR. Closes the loop from review to merged.
6. **Base flexibility.** Review any range (branch vs main, staged, a specific
   commit), not only working-tree vs `HEAD`.

## v1.0 — "real product"

7. Settings UI (agent selection, ignore globs) instead of TOML-only config.
8. Opt-in error reporting and an update-available check.
9. Distribution breadth:
    - **One-line shell installer** — `curl -fsSL https://diffthing.dev/install.sh | bash`
      (Claude Code style). No Node/npm needed. Requires: CI uploads the raw
      per-platform binaries (already built for the npm packages) as GitHub
      Release assets with a checksums file; an `install.sh` that detects
      OS/arch, downloads the matching binary from the latest release, verifies
      the checksum, and installs to `~/.local/bin` (or `/usr/local/bin`); host
      the script on `diffthing.dev` via Cloudflare Pages/Worker. A `.ps1`
      variant covers Windows.
    - Homebrew tap, `cargo-binstall`, Scoop.
10. Docs site and a 60-second demo — the value is not obvious cold.
11. Certificate-renewal automation for `local.diffthing.dev` (Let's Encrypt is
    90 days) as a scheduled job, so a release never silently ships an expired
    cert.

## UI adjustments (fold into v0.2)

- Keyboard navigation between hunks and steps.
- File tree and in-review search.
- Landing / connect-state polish (branded not-connected screen already landed).

Ergonomics are what make people stay.

## Deferred — intentionally out of scope for now

- Team / multi-user review.
- Remote-hosted SPA mode (Drizzle-style). Our loopback model avoids the mixed
  content / Local Network Access fragility; keep it.
- Non-git version control.

## Sequencing

**Persistence → connect UX → VS Code extension.** Those three convert diffthing
from "neat demo" into "I run it on every PR." Everything else is polish on top.
