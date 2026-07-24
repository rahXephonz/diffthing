---
name: explore-codebase
description: Map architecture graph-first (communities, flows, entry points) before reading files. Use for explore, understand, "how does X work", "where is Y", onboarding, architecture.
---

# Explore codebase

Drive the `code-review-graph` MCP for a structural map before opening files.

1. Read `AGENTS.md` and `docs/ARCHITECTURE.md`.
2. `get_architecture_overview_tool` for the high-level shape.
3. `list_communities_tool` / `get_community_tool` and
   `list_flows_tool` / `get_flow_tool` to find entry points and execution flows.
4. `traverse_graph_tool` for targeted imports and call relationships.
5. Verify graph results with `rg` and focused file reads.

Return a concise component map, key invariants, and relevant file paths. Fall
back to `rg` when graph data is missing or stale (incremental, can lag `HEAD`).
