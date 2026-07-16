# diffthing spec pointers

Single sources of truth (code > prose; this file only points):

- Wire protocol + handshake: `crates/core/src/protocol.rs` (PROTOCOL_VERSION)
- Walkthrough schema: `crates/core/src/schema.rs` (WALKTHROUGH_SCHEMA_VERSION)
- Hunk identity rules: `crates/core/src/hunk.rs` (normalization doc comment)
- Reconciliation + honesty rules: `crates/core/src/reconcile.rs`
- Validator invariants: `crates/core/src/validate.rs`
- Scoring signals & priors: `crates/core/src/score.rs`
- Connection state machine (client): `web/src/connection.ts`
- Security model: `crates/daemon/src/server.rs` header comment

Any change to protocol.rs or schema.rs MUST bump the respective version
constant and update `web/src/protocol.ts` (until ts-rs codegen lands, M1).
