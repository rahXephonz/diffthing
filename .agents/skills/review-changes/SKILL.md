---
name: review-changes
description: Risk-aware review of the working diff via changed files, impact radius, flows, and test gaps. Use for review, "audit my diff", check changes, pre-commit review.
---

# Review changes

Drive the `code-review-graph` MCP to focus review on what actually changed.

1. `detect_changes_tool` for changed files and functions.
2. `get_impact_radius_tool` + `get_affected_flows_tool` on the high-risk
   changes.
3. `get_review_context_tool` / `get_minimal_context_tool` to check tests
   covering the changed behavior.
4. Verify product and architecture invariants.
5. Report findings by severity with file and line references.

Prioritize correctness, regressions, security, data loss, and missing tests.
Skip style-only findings unless they impair maintenance. If no findings, say so
and state residual test gaps. Fall back to `rg` when graph data is missing or
stale (incremental, can lag `HEAD`).
