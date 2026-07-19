---
name: debug-issue
description: Trace bugs using repository graph before broad source scanning
---

# Debug issue

1. Start with minimal graph context for reported behavior.
2. Trace related callers, callees, and affected flows.
3. Compare recent changes and inspect impact radius.
4. Read only source needed to confirm cause.
5. Reproduce with focused test or command.
6. Explain root cause before editing unless user requested fix.

Keep graph calls minimal. Fall back to `rg` when graph data is missing or stale.
