---
name: review-changes
description: Perform risk-aware review using changes, impact, flows, and tests
---

# Review changes

1. Detect changed files and functions.
2. Inspect affected flows and dependency radius for high-risk changes.
3. Check tests covering changed behavior.
4. Verify product and architecture invariants.
5. Report findings by severity with file and line references.

Prioritize correctness, regressions, security, data loss, and missing tests. Skip style-only findings unless they impair maintenance. If no findings, say so and state residual test gaps.
