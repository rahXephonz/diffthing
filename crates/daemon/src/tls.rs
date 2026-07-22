//! TLS material for the `local.diffthing.dev` HTTPS flow.
//!
//! The daemon serves this name (public DNS → loopback) over HTTPS. Three ways
//! to get a cert, in order of preference:
//!
//!   1. **Env override** (`DIFFTHING_TLS_CERT` / `DIFFTHING_TLS_KEY`) — bring
//!      your own material, e.g. an mkcert pair for local testing.
//!   2. **Bundled trusted cert** — when a real, CA-trusted cert for
//!      `local.diffthing.dev` is committed under `certs/`, `build.rs` sets the
//!      `bundled_cert` cfg and we `include_bytes!` it. Browsers load
//!      zero-prompt (the Drizzle Studio model). This is the default in
//!      published releases.
//!   3. **Per-install self-signed** — generated on first run and cached in the
//!      user's config dir (`~/.config/diffthing/tls`) when no bundled cert is
//!      present (contributor builds without the private key). Browsers show a
//!      trust prompt once; Safari refuses it outright — hence the bundled cert.
//!
//! Security tradeoff of (2): shipping the private key lets anyone who can bend
//! a victim's DNS / hosts resolution for `local.diffthing.dev` serve trusted
//! JavaScript under that name and read the fragment session token. Accepted
//! because it requires already controlling the victim's name resolution and the
//! token is ephemeral per run. `--offline` (plain HTTP on loopback, itself a
//! secure context) remains the zero-shared-trust path. See `certs/README.md`.

use std::env;
use std::fs;
use std::path::PathBuf;

type Error = Box<dyn std::error::Error + Send + Sync>;

/// A PEM certificate chain paired with its private key.
pub type PemPair = (Vec<u8>, Vec<u8>);

/// PEM cert chain committed under `certs/`, embedded when `build.rs` finds it.
#[cfg(bundled_cert)]
const BUNDLED_CERT: &[u8] = include_bytes!("../certs/local.diffthing.dev.pem");
#[cfg(bundled_cert)]
const BUNDLED_KEY: &[u8] = include_bytes!("../certs/local.diffthing.dev.key.pem");

/// The bundled trusted pair, present only when a real cert was committed.
#[cfg(bundled_cert)]
fn bundled() -> Option<PemPair> {
    Some((BUNDLED_CERT.to_vec(), BUNDLED_KEY.to_vec()))
}
#[cfg(not(bundled_cert))]
fn bundled() -> Option<PemPair> {
    None
}

/// True when the served cert is a genuinely trusted bundled one (no browser
/// trust prompt). Drives the boot messaging: only the self-signed path warns.
pub fn is_trusted() -> bool {
    if env::var_os("DIFFTHING_TLS_CERT").is_some() && env::var_os("DIFFTHING_TLS_KEY").is_some() {
        // Caller-provided material: assume they know what they mounted.
        return true;
    }
    bundled().is_some()
}

/// The cert + key to serve, in order of preference:
///   1. the env-provided pair, if both are set
///   2. the bundled trusted pair, if compiled in
///   3. the cached per-install self-signed pair
///   4. freshly generated self-signed material, persisted for next boot
pub fn material() -> Result<PemPair, Error> {
    if let (Ok(cert), Ok(key)) = (env::var("DIFFTHING_TLS_CERT"), env::var("DIFFTHING_TLS_KEY")) {
        return Ok((fs::read(cert)?, fs::read(key)?));
    }
    if let Some(pair) = bundled() {
        return Ok(pair);
    }
    let dir = tls_dir().ok_or("no config directory found (set HOME or XDG_CONFIG_HOME)")?;
    material_in(&dir)
}

fn material_in(dir: &std::path::Path) -> Result<PemPair, Error> {
    let cert_path = dir.join("cert.pem");
    let key_path = dir.join("key.pem");
    if cert_path.exists() && key_path.exists() {
        return Ok((fs::read(cert_path)?, fs::read(key_path)?));
    }

    let (cert_pem, key_pem) = generate()?;
    fs::create_dir_all(dir)?;
    write_key(&key_path, key_pem.as_bytes())?;
    fs::write(&cert_path, cert_pem.as_bytes())?;
    eprintln!(
        "diffthing: generated a per-install TLS certificate at {} — your browser will ask to trust it once",
        dir.display()
    );
    Ok((cert_pem.into_bytes(), key_pem.into_bytes()))
}

/// Self-signed cert for every name the daemon is reachable as. Long validity
/// on purpose: browsers only cap lifetimes for certs chaining to public
/// roots, and a far-future date avoids silent expiry-rotation machinery.
fn generate() -> Result<(String, String), Error> {
    let mut params = rcgen::CertificateParams::new(vec![
        crate::HOSTED_DOMAIN.to_string(),
        "localhost".to_string(),
    ])?;
    params
        .subject_alt_names
        .push(rcgen::SanType::IpAddress(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)));
    let key_pair = rcgen::KeyPair::generate()?;
    let cert = params.self_signed(&key_pair)?;
    Ok((cert.pem(), key_pair.serialize_pem()))
}

/// Private key gets owner-only permissions on unix; created before content
/// lands so there is no readable window.
fn write_key(path: &std::path::Path, pem: &[u8]) -> Result<(), Error> {
    let mut opts = fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    use std::io::Write;
    opts.open(path)?.write_all(pem)?;
    Ok(())
}

/// `~/.config/diffthing/tls` (or platform equivalents). Mirrors the lookup
/// in `config.rs`, plus `USERPROFILE` so hosted mode works on Windows.
fn tls_dir() -> Option<PathBuf> {
    let base = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .or_else(|| env::var_os("USERPROFILE").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("diffthing").join("tls"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir() -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("diffthing-tls-test-{:08x}", rand::random::<u32>()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn generates_persists_and_reuses() {
        let dir = tmp_dir();
        let (cert1, key1) = material_in(&dir).unwrap();
        assert!(String::from_utf8_lossy(&cert1).contains("BEGIN CERTIFICATE"));
        assert!(String::from_utf8_lossy(&key1).contains("PRIVATE KEY"));

        // Second call must reuse the cached pair, not mint a new identity.
        let (cert2, key2) = material_in(&dir).unwrap();
        assert_eq!(cert1, cert2);
        assert_eq!(key1, key2);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn generated_material_is_valid_rustls_input() {
        let dir = tmp_dir();
        let (cert, key) = material_in(&dir).unwrap();
        let certs: Vec<_> = rustls_pemfile_certs(&cert).expect("cert PEM parses");
        assert!(!certs.is_empty());
        assert!(String::from_utf8_lossy(&key).contains("BEGIN PRIVATE KEY"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[cfg(unix)]
    #[test]
    fn key_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tmp_dir();
        material_in(&dir).unwrap();
        let mode = std::fs::metadata(dir.join("key.pem")).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "key must be 0600, got {mode:o}");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Minimal PEM cert-block splitter so the test needs no extra dep.
    fn rustls_pemfile_certs(pem: &[u8]) -> Option<Vec<String>> {
        let text = String::from_utf8_lossy(pem);
        let blocks: Vec<String> = text
            .split("-----BEGIN CERTIFICATE-----")
            .skip(1)
            .map(|b| b.split("-----END CERTIFICATE-----").next().unwrap_or("").to_string())
            .collect();
        (!blocks.is_empty()).then_some(blocks)
    }
}
