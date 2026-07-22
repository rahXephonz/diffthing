#!/usr/bin/env bash
# Issue (or renew) the PUBLIC CA cert for local.diffthing.dev via acme.sh's
# DNS-01 challenge and place it where build.rs/tls.rs bundle it. This is the
# production, all-browsers-trust-it path (Drizzle model). Run it yourself — it
# needs your DNS provider's API credentials; those never leave your machine.
#
# The hostname resolves to 127.0.0.1, so DNS-01 (not HTTP-01) is the only
# option — no inbound port is ever exposed.
#
# Prereqs:
#   - acme.sh installed        https://github.com/acmesh-official/acme.sh
#   - DNS provider API creds exported the way acme.sh expects, e.g. Cloudflare:
#       export CF_Token=...      export CF_Account_ID=...
#     Provider var names: https://github.com/acmesh-official/acme.sh/wiki/dnsapi
#
# Usage:
#   DNS_PROVIDER=dns_cf ./scripts/cert-prod.sh            # issue / renew
#   DNS_PROVIDER=dns_gd ./scripts/cert-prod.sh            # GoDaddy, etc.
#
# After it runs: rebuild (`cargo build -p diffthing`) so bundled_cert turns on,
# commit the two files under crates/daemon/certs/, and cut a release. Repeat
# roughly every 90 days (Let's Encrypt lifetime) to rotate.
set -euo pipefail

DOMAIN="local.diffthing.dev"
DNS_PROVIDER="${DNS_PROVIDER:-dns_cf}"

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CERT_DIR="$ROOT/crates/daemon/certs"
CERT="$CERT_DIR/$DOMAIN.pem"
KEY="$CERT_DIR/$DOMAIN.key.pem"

# Prefer the upstream install (~/.acme.sh/acme.sh) over a package-manager one:
# newer DNS providers (e.g. dns_hostinger) ship on master but lag in Homebrew.
ACME=""
if [ -x "$HOME/.acme.sh/acme.sh" ]; then
  ACME="$HOME/.acme.sh/acme.sh"
elif command -v acme.sh >/dev/null 2>&1; then
  ACME="$(command -v acme.sh)"
else
  echo "cert-prod: acme.sh not found." >&2
  echo "  install (recommended, has all dnsapi providers):" >&2
  echo "    curl https://get.acme.sh | sh -s email=you@example.com" >&2
  exit 1
fi

# Fail early with a clear message if this acme.sh lacks the requested provider,
# rather than a cryptic hook error mid-issue.
_dnsapi="$(dirname "$ACME")/dnsapi/$DNS_PROVIDER.sh"
if [ ! -f "$_dnsapi" ]; then
  echo "cert-prod: $ACME has no $DNS_PROVIDER provider ($_dnsapi missing)." >&2
  echo "  its version predates that provider. Reinstall from upstream master:" >&2
  echo "    curl https://get.acme.sh | sh -s email=you@example.com" >&2
  echo "  then open a new shell and re-run this script." >&2
  exit 1
fi

# FORCE=1 rotates: reissue even if the current cert is still valid AND mint a
# fresh private key. Use it after a key compromise / when replacing a leaked
# cert — a plain renewal reuses the existing key, which is not a real rotation.
FORCE_ARGS=()
if [ "${FORCE:-}" = "1" ]; then
  FORCE_ARGS=(--force --always-force-new-domain-key)
  echo "cert-prod: FORCE=1 — reissuing with a brand-new key"
fi

echo "cert-prod: issuing ${DOMAIN} via ${DNS_PROVIDER} (DNS-01) using ${ACME}"
"$ACME" --issue --dns "$DNS_PROVIDER" -d "$DOMAIN" "${FORCE_ARGS[@]}"

# --install-cert copies the current material to fixed paths and is the
# renew-safe way to keep these files fresh (acme.sh re-runs it on renewal).
"$ACME" --install-cert -d "$DOMAIN" \
  --fullchain-file "$CERT" \
  --key-file "$KEY"

# Private key stays owner-only on disk even though it will ship in the binary.
chmod 600 "$KEY"

echo
echo "cert-prod: wrote"
echo "  $CERT"
echo "  $KEY"
echo
echo "next: cargo build -p diffthing   # bundled_cert turns on"
echo "      git add crates/daemon/certs/$DOMAIN.pem crates/daemon/certs/$DOMAIN.key.pem"
echo "      git commit && release"
