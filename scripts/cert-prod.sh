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

if ! command -v acme.sh >/dev/null 2>&1; then
  echo "cert-prod: acme.sh not found." >&2
  echo "  install: https://github.com/acmesh-official/acme.sh#1-how-to-install" >&2
  exit 1
fi

echo "cert-prod: issuing $DOMAIN via $DNS_PROVIDER (DNS-01)…"
acme.sh --issue --dns "$DNS_PROVIDER" -d "$DOMAIN"

# --install-cert copies the current material to fixed paths and is the
# renew-safe way to keep these files fresh (acme.sh re-runs it on renewal).
acme.sh --install-cert -d "$DOMAIN" \
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
