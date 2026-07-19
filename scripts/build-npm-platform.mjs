#!/usr/bin/env node
// Package a single compiled `diffthing` binary into a platform-specific npm
// package so the `diffthing` launcher can resolve it as an optional dependency.
//
// Usage:
//   node scripts/build-npm-platform.mjs \
//     --target x86_64-unknown-linux-gnu \
//     --binary target/x86_64-unknown-linux-gnu/release/diffthing \
//     --version 0.1.0
//
// Emits: npm/<pkg-name>/{package.json, bin/<binary>, README.md}
// Prints the package directory to stdout.

import { mkdirSync, copyFileSync, writeFileSync, chmodSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");

// Rust target triple -> npm platform package descriptor.
const TARGETS = {
  "aarch64-apple-darwin": { pkg: "diffthing-darwin-arm64", os: "darwin", cpu: "arm64", bin: "diffthing" },
  "x86_64-apple-darwin": { pkg: "diffthing-darwin-x64", os: "darwin", cpu: "x64", bin: "diffthing" },
  "x86_64-unknown-linux-gnu": { pkg: "diffthing-linux-x64", os: "linux", cpu: "x64", bin: "diffthing" },
  "aarch64-unknown-linux-gnu": { pkg: "diffthing-linux-arm64", os: "linux", cpu: "arm64", bin: "diffthing" },
  "x86_64-pc-windows-msvc": { pkg: "diffthing-win32-x64", os: "win32", cpu: "x64", bin: "diffthing.exe" },
};

function arg(name) {
  const i = process.argv.indexOf(`--${name}`);
  if (i === -1 || i + 1 >= process.argv.length) {
    throw new Error(`missing required argument --${name}`);
  }
  return process.argv[i + 1];
}

const target = arg("target");
const binary = arg("binary");
const version = arg("version");

const desc = TARGETS[target];
if (!desc) {
  throw new Error(`unknown target "${target}". Known: ${Object.keys(TARGETS).join(", ")}`);
}

const pkgDir = join(ROOT, "npm", desc.pkg);
const binDir = join(pkgDir, "bin");
mkdirSync(binDir, { recursive: true });

const destBinary = join(binDir, desc.bin);
copyFileSync(binary, destBinary);
if (desc.os !== "win32") {
  chmodSync(destBinary, 0o755);
}

const pkgJson = {
  name: desc.pkg,
  version,
  description: `Prebuilt diffthing binary for ${desc.os} ${desc.cpu}.`,
  repository: {
    type: "git",
    url: "git+https://github.com/rahXephonz/diffthing.git",
    directory: `npm/${desc.pkg}`,
  },
  license: "MIT",
  os: [desc.os],
  cpu: [desc.cpu],
  files: [`bin/${desc.bin}`],
};

writeFileSync(join(pkgDir, "package.json"), JSON.stringify(pkgJson, null, 2) + "\n");
writeFileSync(
  join(pkgDir, "README.md"),
  `# ${desc.pkg}\n\nPrebuilt \`diffthing\` binary for ${desc.os} ${desc.cpu}.\n\n` +
    "This is an internal platform package. Install [`diffthing`](https://www.npmjs.com/package/diffthing) instead.\n",
);

process.stdout.write(pkgDir + "\n");
