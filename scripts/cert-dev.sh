#!/usr/bin/env bash
# Dev-only: mint a LOCALLY-trusted cert for local.diffthing.dev with mkcert and
# run the daemon with it via the DIFFTHING_TLS_CERT/KEY env override. This makes
# Safari open https://local.diffthing.dev:PORT zero-prompt on THIS machine.
#
# NOT for release: mkcert certs are trusted only where `mkcert -install` ran.
# The generated pair lands in .certs-dev/ (gitignored) — never commit it. For
# shipping to all users you need a real CA cert in crates/daemon/certs/ (see
# crates/daemon/certs/README.md).
#
# Usage:  pnpm cert:dev            # or: bash scripts/cert-dev.sh
#         pnpm cert:dev -- --base main   # extra args forward to the daemon
set -euo pipefail

if ! command -v mkcert >/dev/null 2>&1; then
  echo "cert-dev: mkcert not found. Install it first:" >&2
  echo "  macOS:  brew install mkcert" >&2
  echo "  other:  https://github.com/FiloSottile/mkcert#installation" >&2
  exit 1
fi

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CERT_DIR="$ROOT/.certs-dev"
CERT="$CERT_DIR/local.diffthing.dev.pem"
KEY="$CERT_DIR/local.diffthing.dev-key.pem"

mkdir -p "$CERT_DIR"

# Install the local CA into the system/browser trust stores. Idempotent —
# a no-op once already installed.
mkcert -install

# (Re)generate only when missing so repeat runs stay fast.
if [[ ! -f "$CERT" || ! -f "$KEY" ]]; then
  mkcert -cert-file "$CERT" -key-file "$KEY" local.diffthing.dev localhost 127.0.0.1
fi

echo "cert-dev: serving with locally-trusted cert from $CERT_DIR"
exec env DIFFTHING_TLS_CERT="$CERT" DIFFTHING_TLS_KEY="$KEY" \
  cargo run -p diffthing -- "$@"
