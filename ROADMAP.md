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
   The source material already exists and is currently thrown away at
   approval time: the walkthrough is a scope-by-scope narrative of the
   change, which is exactly what a PR description should be. Generating from
   approved content also means the message describes what a human actually
   signed off on, not what the agent believes it did.
6. **Base flexibility.** Review any range (branch vs main, staged, a specific
   commit), not only working-tree vs `HEAD`.
7. **One-line shell installer.** `curl -fsSL https://diffthing.dev/install.sh | bash`
   (CLI style). No Node/npm needed. CI already builds the per-platform
   binaries for the npm packages but only publishes them there; it must also
   upload them as GitHub Release assets with a checksums file. Then an
   `install.sh` that detects OS/arch, downloads the matching binary from the
   latest release, verifies the checksum, and installs to `~/.local/bin` (or
   `/usr/local/bin`), hosted on `diffthing.dev` via Cloudflare Pages/Worker.
   A `.ps1` variant covers Windows. Prerequisite for everything below it:
   without raw release binaries a plugin can only wrap `npx`.
8. **Agent plugin / skill distribution.** Ship diffthing as an installable
   plugin for the agent CLIs where AI-assisted changes are made, so the agent
   opens a review itself when it finishes a batch of edits — no one has to
   remember the command. One repo carries a shared `SKILL.md` plus a manifest
   per ecosystem: Claude Code (`.claude-plugin/marketplace.json`), Codex
   (`skills/` + `hooks/`), Kimi Code CLI (`plugin.json`), Gemini CLI
   (extension). Depends on item 7 — without raw release binaries the plugin
   can only shell out to `npx`, which defeats the point.
   The skill must state the boundary explicitly: the agent opens the review
   and stops; it never marks viewed, closes a flag, or infers approval from
   silence.
9. **Stop-time review gate** (ships in the plugin from item 8). The real
   failure mode is not that review is hard, it is that review gets skipped:
   the agent declares itself done, the human skims, the change ships. A skill
   the agent *may* invoke does not fix skipping. A stop hook does — when the
   agent claims completion, surface "8 hunks, 0 reviewed — review at <url>",
   with an opt-in blocking mode for people who want it enforced. This turns
   the product boundary from a stated principle into a structural one, and no
   generic plugin can copy it because it needs the daemon. Note the direction
   of information flow: the hook reads review state to tell the *human* what
   is unreviewed. The agent still never acts on that state.
10. **Session-start pickup of open comments** (also plugin-side). Persistence
   landed in v0.2 but nothing consumes it across sessions yet: a reviewer
   leaves comments, closes the laptop, and the next morning the agent has no
   idea they exist. A session-start hook reads `.diffthing/review.db` and
   tells the agent "2 open review comments from your last session" so work
   resumes there instead of with "what should I do next?". Cheap — the data
   is already on disk, keyed by content hash, and survives line shifts.

## v1.0 — "real product"

11. Settings UI (agent selection, ignore globs) instead of TOML-only config.
12. Opt-in error reporting and an update-available check.
13. Distribution breadth: Homebrew tap, `cargo-binstall`, Scoop. All three
    consume the release assets published by item 7.
14. Docs site and a 60-second demo — the value is not obvious cold.
15. Certificate-renewal automation for `local.diffthing.dev` (Let's Encrypt is
    90 days) as a scheduled job, so a release never silently ships an expired
    cert.
16. **Connect UX.** Auto-open the browser to the printed URL on start, and use a
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
