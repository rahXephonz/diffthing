#!/usr/bin/env node
"use strict";

// Launcher for the `diffthing` npm package. Resolves the platform-specific
// prebuilt Rust binary shipped as an optional dependency and execs it,
// forwarding argv, stdio, and exit status.

const { spawnSync } = require("node:child_process");

// npm platform key -> { pkg, bin } for the optional dependency that carries
// the compiled binary for this host. Keys match `${platform}-${arch}`.
const TARGETS = {
  "darwin-arm64": { pkg: "diffthing-darwin-arm64", bin: "diffthing" },
  "darwin-x64": { pkg: "diffthing-darwin-x64", bin: "diffthing" },
  "linux-x64": { pkg: "diffthing-linux-x64", bin: "diffthing" },
  "linux-arm64": { pkg: "diffthing-linux-arm64", bin: "diffthing" },
  "win32-x64": { pkg: "diffthing-win32-x64", bin: "diffthing.exe" },
};

function resolveBinary() {
  const key = `${process.platform}-${process.arch}`;
  const target = TARGETS[key];
  if (!target) {
    const supported = Object.keys(TARGETS).join(", ");
    fail(
      `diffthing has no prebuilt binary for ${key}.\n` +
        `Supported platforms: ${supported}.\n` +
        `Build from source: https://github.com/rahXephonz/diffthing`,
    );
  }

  let pkgJson;
  try {
    pkgJson = require.resolve(`${target.pkg}/package.json`);
  } catch {
    fail(
      `The platform package "${target.pkg}" is not installed.\n` +
        `This usually means the optional dependency was skipped. Reinstall with:\n` +
        `  npm install diffthing\n` +
        `and make sure optional dependencies are enabled ` +
        `(do not pass --no-optional / --omit=optional).`,
    );
  }

  const path = require("node:path");
  return path.join(path.dirname(pkgJson), "bin", target.bin);
}

function fail(message) {
  process.stderr.write(`diffthing: ${message}\n`);
  process.exit(1);
}

function main() {
  const binary = resolveBinary();
  // Default flow is HTTPS via local.diffthing.dev (SPA embedded in the
  // binary). Pass --offline for the plain-http 127.0.0.1 fallback. All args
  // forward through unchanged.
  const args = process.argv.slice(2);

  const result = spawnSync(binary, args, { stdio: "inherit" });

  if (result.error) {
    if (result.error.code === "ENOENT") {
      fail(`binary not found at ${binary}. Try reinstalling: npm install diffthing`);
    }
    fail(result.error.message);
  }

  // Propagate signal-based termination as the conventional 128+signal code.
  if (result.signal) {
    process.exit(1);
  }
  process.exit(result.status === null ? 1 : result.status);
}

main();
