---
name: debug-issue
description: Trace a bug/regression/panic graph-first (callers, callees, impact radius) before scanning source. Use for debug, fix, "why is X failing", root cause, crash, stack trace.
---

# Debug issue

Drive the `code-review-graph` MCP — cheaper and more precise than broad source
scanning.

1. `get_minimal_context_tool` for the symbol/file in the reported behavior.
2. `traverse_graph_tool` to walk callers and callees around it.
3. `get_affected_flows_tool` + `detect_changes_tool` / `get_impact_radius_tool`
   to correlate the fault with recent changes and blast radius.
4. Read only the source the graph points at to confirm the cause.
5. Reproduce with a focused test or command.
6. Explain root cause before editing unless the user requested a fix.

Keep graph calls minimal. Fall back to `rg` and focused reads when graph data is
missing or stale (the graph is incremental and can lag `HEAD`).
