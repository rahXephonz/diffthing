#!/bin/sh
# diffthing installer.
#
# Detects your OS/arch, downloads the matching prebuilt binary from the latest
# GitHub release, verifies its SHA-256 checksum against the release's
# SHA256SUMS, and installs it to ~/.local/bin (or $DIFFTHING_INSTALL_DIR).
#
#   curl -fsSL https://diffthing.dev/install.sh | sh
#
# Environment overrides:
#   DIFFTHING_VERSION      release tag to install, e.g. v0.3.0 (default: latest)
#   DIFFTHING_INSTALL_DIR  install directory (default: ~/.local/bin)
#
# No Node/npm required. For Windows use install.ps1.
set -eu

REPO="rahXephonz/diffthing"
BINARY="diffthing"

info() { printf '%s\n' "$*" >&2; }
err() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}
need() { command -v "$1" >/dev/null 2>&1 || err "required command not found: $1"; }

# uname os/arch -> Rust target triple that names the release archive.
detect_target() {
  os="$(uname -s)"
  arch="$(uname -m)"
  case "$arch" in
    x86_64 | amd64) arch_part="x86_64" ;;
    arm64 | aarch64) arch_part="aarch64" ;;
    *) err "unsupported architecture: $arch" ;;
  esac
  case "$os" in
    Linux) TARGET="${arch_part}-unknown-linux-gnu" ;;
    Darwin) TARGET="${arch_part}-apple-darwin" ;;
    *) err "unsupported OS: $os (use install.ps1 on Windows)" ;;
  esac
}

download() {
  # download <url> <dest>
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$1" -o "$2" || err "download failed: $1"
  elif command -v wget >/dev/null 2>&1; then
    wget -qO "$2" "$1" || err "download failed: $1"
  else
    err "need curl or wget to download"
  fi
}

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  else
    err "need sha256sum or shasum to verify the download"
  fi
}

main() {
  need uname
  need tar
  detect_target

  version="${DIFFTHING_VERSION:-latest}"
  if [ "$version" = "latest" ]; then
    base="https://github.com/$REPO/releases/latest/download"
  else
    base="https://github.com/$REPO/releases/download/$version"
  fi

  archive="diffthing-${TARGET}.tar.gz"
  tmp="$(mktemp -d)"
  # shellcheck disable=SC2064
  trap "rm -rf \"$tmp\"" EXIT INT TERM

  info "downloading $archive ($version)"
  download "$base/$archive" "$tmp/$archive"
  download "$base/SHA256SUMS" "$tmp/SHA256SUMS"

  info "verifying checksum"
  expected="$(awk -v f="$archive" '$2 == f {print $1}' "$tmp/SHA256SUMS")"
  [ -n "$expected" ] || err "no checksum for $archive in SHA256SUMS"
  actual="$(sha256_of "$tmp/$archive")"
  [ "$expected" = "$actual" ] || err "checksum mismatch: expected $expected, got $actual"

  tar -xzf "$tmp/$archive" -C "$tmp" || err "failed to extract $archive"
  [ -f "$tmp/$BINARY" ] || err "archive did not contain $BINARY"

  install_dir="${DIFFTHING_INSTALL_DIR:-$HOME/.local/bin}"
  mkdir -p "$install_dir" || err "cannot create $install_dir"
  if command -v install >/dev/null 2>&1; then
    install -m 755 "$tmp/$BINARY" "$install_dir/$BINARY"
  else
    cp "$tmp/$BINARY" "$install_dir/$BINARY"
    chmod 755 "$install_dir/$BINARY"
  fi

  info "installed diffthing to $install_dir/$BINARY"
  case ":$PATH:" in
    *":$install_dir:"*) ;;
    *)
      info ""
      info "note: $install_dir is not on your PATH. Add it to your shell profile:"
      info "  export PATH=\"$install_dir:\$PATH\""
      ;;
  esac
}

main "$@"
