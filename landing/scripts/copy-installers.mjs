// Copy the canonical install scripts into public/ so the built site serves them
// at the domain root: https://diffthing.dev/install.sh and /install.ps1. The
// scripts stay single-sourced at the repo root — these copies are generated at
// build time and gitignored. Runs before `astro build`.
import { copyFileSync, existsSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url)); // landing/scripts
const repoRoot = join(here, "..", ".."); // repo root
const publicDir = join(here, "..", "public");

mkdirSync(publicDir, { recursive: true });

for (const file of ["install.sh", "install.ps1"]) {
  const src = join(repoRoot, file);
  if (!existsSync(src)) {
    console.error(`copy-installers: missing ${src}`);
    process.exit(1);
  }
  copyFileSync(src, join(publicDir, file));
  console.log(`copy-installers: ${file} -> public/`);
}
