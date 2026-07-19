#!/usr/bin/env bash
# Issue the real, publicly-trusted TLS certificate for local.diffthing.dev and
# drop it into crates/daemon/certs/, replacing whatever is there. After this,
# every user's browser trusts https://local.diffthing.dev with nothing to
# install.
#
# Requires control of the diffthing.dev DNS zone (the one step only the domain
# owner can perform). Uses an ACME DNS-01 challenge because the domain resolves
# to 127.0.0.1 and cannot be reached for HTTP-01.
#
# Uses lego (https://go-acme.github.io/lego): `brew install lego`.
#
# Usage:
#   CF_DNS_API_TOKEN=... ACME_EMAIL=you@example.com ./scripts/issue-cert.sh
#       Automated via Cloudflare (diffthing.dev is on Cloudflare).
#   ACME_EMAIL=you@example.com ./scripts/issue-cert.sh manual
#       Interactive: prints a TXT record to add on any DNS host, then continues.
#
# Renewal: Let's Encrypt certs last 90 days. Re-run and cut a new release.

set -euo pipefail

DOMAIN="local.diffthing.dev"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CERT_DIR="$ROOT/crates/daemon/certs"
EMAIL="${ACME_EMAIL:-}"
# Accept CF_API_TOKEN as an alias for lego's CF_DNS_API_TOKEN.
: "${CF_DNS_API_TOKEN:=${CF_API_TOKEN:-}}"
export CF_DNS_API_TOKEN
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

log() { printf '\033[36m==>\033[0m %s\n' "$*"; }
die() { printf '\033[31merror:\033[0m %s\n' "$*" >&2; exit 1; }

command -v lego >/dev/null || die "lego not found — install with: brew install lego"
[ -n "$EMAIL" ] || die "set ACME_EMAIL=you@example.com"

MODE="${1:-cloudflare}"
case "$MODE" in
  cloudflare)
    [ -n "$CF_DNS_API_TOKEN" ] || die "set CF_DNS_API_TOKEN (Cloudflare token with Zone:DNS:Edit on diffthing.dev)"
    log "issuing via Cloudflare DNS-01 for $DOMAIN (automated)"
    PROVIDER="cloudflare"
    ;;
  manual)
    log "issuing via manual DNS-01 for $DOMAIN (you'll add a TXT record)"
    PROVIDER="manual"
    ;;
  *)
    die "unknown mode '$MODE' (use: cloudflare | manual)"
    ;;
esac

# lego 5 puts all issuance flags after the `run` subcommand.
lego --path "$WORK" run \
  --accept-tos \
  --email "$EMAIL" \
  --dns "$PROVIDER" \
  --domains "$DOMAIN"

CRT="$WORK/certificates/$DOMAIN.crt"   # leaf + intermediate chain
KEY="$WORK/certificates/$DOMAIN.key"
[ -s "$CRT" ] && [ -s "$KEY" ] || die "issuance produced no certificate"

# Refuse a self-signed result: issuer must differ from subject.
if openssl x509 -in "$CRT" -noout -issuer -subject \
   | awk -F'issuer=|subject=' '/issuer/{i=$2} /subject/{s=$2} END{exit (i==s)?0:1}'; then
  die "issued cert appears self-signed — check the ACME output"
fi

cp "$CRT" "$CERT_DIR/$DOMAIN.pem"
cp "$KEY" "$CERT_DIR/$DOMAIN.key"
chmod 600 "$CERT_DIR/$DOMAIN.key"

log "installed into $CERT_DIR:"
openssl x509 -in "$CERT_DIR/$DOMAIN.pem" -noout -issuer -subject -dates
log "commit crates/daemon/certs/$DOMAIN.{pem,key} and cut a release."
