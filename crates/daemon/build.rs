//! Build-time detection of the bundled `local.diffthing.dev` TLS material.
//!
//! Drizzle-style flow: when a real, publicly-trusted cert for the loopback
//! hostname is present under `certs/`, embed it so every prebuilt binary serves
//! trusted TLS (Safari/Brave/Chrome load zero-prompt). The files are gitignored
//! — releases write them from CI secrets, local builds get them from
//! `scripts/cert-prod.sh`. When absent (forks, PRs, contributors without the
//! key) the cfg stays off and `tls.rs` falls back to per-install self-signed
//! material, so the crate always compiles.

use std::path::Path;

const CERT: &str = "certs/local.diffthing.dev.pem";
const KEY: &str = "certs/local.diffthing.dev.key.pem";

fn main() {
    // Single-colon form: MSRV is 1.75, which predates the `cargo::` syntax.
    // The `bundled_cert` cfg is declared to check-cfg via `[lints.rust]` in
    // Cargo.toml so it isn't flagged as unexpected on modern toolchains.
    println!("cargo:rerun-if-changed={CERT}");
    println!("cargo:rerun-if-changed={KEY}");

    if Path::new(CERT).exists() && Path::new(KEY).exists() {
        println!("cargo:rustc-cfg=bundled_cert");
    }
}
