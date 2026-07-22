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
   the agent _may_ invoke does not fix skipping. A stop hook does — when the
   agent claims completion, surface "8 hunks, 0 reviewed — review at <url>",
   with an opt-in blocking mode for people who want it enforced. This turns
   the product boundary from a stated principle into a structural one, and no
   generic plugin can copy it because it needs the daemon. Note the direction
   of information flow: the hook reads review state to tell the _human_ what
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
15. **Bundled trusted cert (Drizzle-style).** A real CA-trusted cert for
    `local.diffthing.dev` is embedded in the binary (`crates/daemon/certs/`,
    gated by `build.rs`) and served by default, so every browser — Safari
    included — loads zero-prompt. Per-install self-signed remains the fallback
    when no cert is committed. This **reverses the earlier "no shared key ships"
    stance**: shipping the private key lets anyone who can bend a victim's
    DNS/hosts for `local.diffthing.dev` serve trusted JS and read the ephemeral
    fragment token. Accepted (it requires already owning the victim's name
    resolution); `--offline` stays the zero-shared-trust path. Certificate
    renewal is the owner's to rotate; see `certs/README.md`.
16. **Connect UX.** Auto-open the browser to the printed URL on start, and use a
    fixed default port (`4983`, falling back if busy) for a stable, bookmarkable
    `https://local.diffthing.dev:4983`. Removes the bare-domain confusion. We will take it
    in the future, for now current flow its okay.

Ergonomics are what make people stay.

17. OS-level sandboxing for every dispatch runner (repo-only filesystem,
    network off by default) — completes the agent trust boundary beyond
    prompt fencing + per-CLI capability flags + scope rollback. Needs a
    per-platform strategy (Seatbelt on macOS, landlock/bwrap on Linux).

## v2.x+ — "engineering intelligence"

Long-term direction, deliberately sequenced after the core review workflow.
Generation is becoming cheap; judgment is becoming valuable. Everything here
exists to make human judgment scale — with context and memory — never to
replace it or automate approval.

18. **Engineering memory.** Persist decisions alongside review state, not only
    viewed/resolved status: why a comment was resolved, why an approach was
    rejected, rationale for important changes, recurring review discussions.
    Future reviews benefit from past human judgment, and the repository
    accumulates a durable record of _why_ things changed, not only _what_.
19. **Repository memory.** Learn repo-specific conventions over time: naming,
    architectural boundaries, common migration patterns, known risky areas.
    Local by default, fully under the developer's control.
20. **Review recipes.** Capture review patterns that worked (React major
    upgrades, database migrations, auth refactors, API version migrations) and
    reuse them instead of restating the same guidance every review.
21. **Context engine.** Feed items 18–20 — prior decisions, conventions,
    architecture, known constraints — to supported agents before they execute
    explicit requests. Better inputs, same boundary: the human still reviews
    and approves.
22. **Review analytics.** Metrics for improving review practice: completion
    time, high-risk change distribution, reopened changes, churn, files with
    recurring issues, coverage over time. For improving practices, never for
    evaluating individuals.
23. **Knowledge graph (experimental).** Explore representing the memory from
    items 18–20 as connected entities (files, modules, decisions, migrations,
    recipes) to deepen context for future reviews and agent interactions —
    never to automate approval.

## Deferred — intentionally out of scope for now

- Team / multi-user review.
- Fully remote-hosted SPA (static build served from `diffthing.dev`, daemon
  API-only). Not needed for browser trust — item 15's bundled trusted cert
  already gives the Drizzle zero-prompt UX from the daemon-served SPA. Revisit
  only if we want the SPA to update independently of the installed daemon.
- Non-git version control.

## Sequencing

Persistence shipped in v0.2. Next: **shell installer → agent plugin → VS Code
extension.** The installer unblocks the plugin, the plugin puts diffthing in
front of developers at the moment an agent finishes editing, and the extension
meets the ones who never leave their editor. Everything else is polish on top.
