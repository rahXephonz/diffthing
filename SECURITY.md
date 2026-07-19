# Security policy

## Supported versions

diffthing is pre-1.0. Security fixes target latest `master` revision.

## Reporting a vulnerability

Do not open public issue for suspected vulnerability. Report privately to `rimzzlabs@proton.me` with:

- Affected revision.
- Reproduction steps or proof of concept.
- Expected impact.
- Suggested mitigation, if available.

Avoid including secrets, private source code, or unrelated personal data.

## Security boundaries

Changes touching loopback binding, session tokens, origin validation, WebSocket handshake, agent execution, Git scope validation, or filesystem access require explicit security review.
