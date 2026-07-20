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
  trusted local HTTPS via `local.diffthing.dev`, review-state persistence
  across restarts, and daemon e2e tests running on Linux and Windows.
- Thin edges: no editor integration, a manual connect step, and no way to
  reach developers inside the agent tools where AI-assisted changes actually
  originate. These — not the core — are what hold back real adoption.

## v0.3 — "fits my workflow"

4. **VS Code extension.** Open a review from the command palette, jump to
   `file:line`, comment inline. Meets developers where they already work; likely
   the single biggest reach-expander.
5. **Commit / PR flow.** After approval, generate the commit message via the
   user's agent; optionally open a PR. Closes the loop from review to merged.
6. **Base flexibility.** Review any range (branch vs main, staged, a specific
   commit), not only working-tree vs `HEAD`.
7. **Agent plugin / skill distribution.** Ship diffthing as an installable
   plugin for the agent CLIs where AI-assisted changes are made, so the agent
   opens a review itself when it finishes a batch of edits — no one has to
   remember the command. One repo carries a shared `SKILL.md` plus a manifest
   per ecosystem: Claude Code (`.claude-plugin/marketplace.json`), Codex
   (`skills/` + `hooks/`), Kimi Code CLI (`plugin.json`), Gemini CLI
   (extension). Depends on the shell installer below — without raw release
   binaries the plugin can only shell out to `npx`, which defeats the point.
   The skill must state the boundary explicitly: the agent opens the review
   and stops; it never marks viewed, closes a flag, or infers approval from
   silence.

## v1.0 — "real product"

8. Settings UI (agent selection, ignore globs) instead of TOML-only config.
9. Opt-in error reporting and an update-available check.
10. Distribution breadth:
   - **One-line shell installer** — `curl -fsSL https://diffthing.dev/install.sh | bash`
     (CLI style). No Node/npm needed. Requires: CI uploads the raw
     per-platform binaries (already built for the npm packages) as GitHub
     Release assets with a checksums file; an `install.sh` that detects
     OS/arch, downloads the matching binary from the latest release, verifies
     the checksum, and installs to `~/.local/bin` (or `/usr/local/bin`); host
     the script on `diffthing.dev` via Cloudflare Pages/Worker. A `.ps1`
     variant covers Windows.
   - Homebrew tap, `cargo-binstall`, Scoop.
11. Docs site and a 60-second demo — the value is not obvious cold.
12. Certificate-renewal automation for `local.diffthing.dev` (Let's Encrypt is
    90 days) as a scheduled job, so a release never silently ships an expired
    cert.
13. **Connect UX.** Auto-open the browser to the printed URL on start, and use a
    fixed default port (`4983`, falling back if busy) for a stable, bookmarkable
    `https://local.diffthing.dev:4983`. Removes the bare-domain confusion. We will take it
    in the future, for now current flow its okay.

Ergonomics are what make people stay.

## Deferred — intentionally out of scope for now

- Team / multi-user review.
- Remote-hosted SPA mode (Drizzle-style). Our loopback model avoids the mixed
  content / Local Network Access fragility; keep it.
- Non-git version control.

## Sequencing

Persistence shipped in v0.2. Next: **shell installer → agent plugin → VS Code
extension.** The installer unblocks the plugin, the plugin puts diffthing in
front of developers at the moment an agent finishes editing, and the extension
meets the ones who never leave their editor. Everything else is polish on top.
