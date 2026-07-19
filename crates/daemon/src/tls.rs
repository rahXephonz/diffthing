//! TLS material for the `local.diffthing.dev` HTTPS flow.
//!
//! A certificate for `local.diffthing.dev` is bundled in the binary (see
//! `crates/daemon/certs`). The domain resolves (via public DNS) to
//! `127.0.0.1`, so serving it over HTTPS on loopback gives the browser a
//! trusted origin that reaches the local daemon — no mixed content, no Private
//! Network Access prompt, and the WebSocket is same-origin.
//!
//! The bundled private key is intentionally **not a secret**: because the
//! domain only ever resolves to loopback, the key can only serve HTTPS on a
//! user's own `127.0.0.1`, never authenticate a remote host. This is the same
//! tradeoff Drizzle Studio makes with `local.drizzle.studio`. Override with
//! `DIFFTHING_TLS_CERT` / `DIFFTHING_TLS_KEY` to serve your own cert instead.

use std::env;
use std::fs;

type Error = Box<dyn std::error::Error + Send + Sync>;

/// A PEM certificate chain paired with its private key.
pub type PemPair = (Vec<u8>, Vec<u8>);

const CERT: &[u8] = include_bytes!("../certs/local.diffthing.dev.pem");
const KEY: &[u8] = include_bytes!("../certs/local.diffthing.dev.key");

/// The cert + key to serve: the env-provided pair if both are set, otherwise
/// the bundled `local.diffthing.dev` material.
pub fn material() -> Result<PemPair, Error> {
    if let (Ok(cert), Ok(key)) = (env::var("DIFFTHING_TLS_CERT"), env::var("DIFFTHING_TLS_KEY")) {
        return Ok((fs::read(cert)?, fs::read(key)?));
    }
    Ok((CERT.to_vec(), KEY.to_vec()))
}
