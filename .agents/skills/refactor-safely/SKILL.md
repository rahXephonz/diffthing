---
name: refactor-safely
description: Plan/verify a refactor via dependency + flow analysis and a rename preview. Use for refactor, rename, move, extract, restructure, split a large function.
---

# Refactor safely

Drive the `code-review-graph` MCP to size the blast radius before editing.

1. Establish current behavior with tests.
2. `get_impact_radius_tool` + `get_affected_flows_tool` for the dependency and
   flow blast radius; `find_large_functions_tool` to spot split targets.
3. `refactor_tool` to preview a rename/structural edit, `apply_refactor_tool` to
   apply it.
4. Preserve protocol, reconciliation, review, and security invariants.
5. Make the smallest coherent change.
6. Run focused tests, then repository verification.

Never mix unrelated cleanup into a refactor. Fall back to `rg` when graph data
is missing or stale (incremental, can lag `HEAD`).
