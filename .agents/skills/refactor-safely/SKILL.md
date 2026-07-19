---
name: refactor-safely
description: Plan and verify refactors using dependency and flow analysis
---

# Refactor safely

1. Establish current behavior with tests.
2. Inspect dependency radius and affected flows.
3. Preview rename or structural edits before applying them.
4. Preserve protocol, reconciliation, review, and security invariants.
5. Make smallest coherent change.
6. Run focused tests, then repository verification.

Never mix unrelated cleanup into refactor.
