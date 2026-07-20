import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// The SPA ships embedded in the daemon binary (rust-embed), so its version IS
// the daemon's. Read the workspace manifest rather than web/package.json,
// which nothing keeps in sync with releases.
function workspaceVersion(): string {
  const manifest = readFileSync(fileURLToPath(new URL("../Cargo.toml", import.meta.url)), "utf8");
  const version = manifest
    .split(/^\[/m)
    .find((section) => section.startsWith("workspace.package]"))
    ?.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
  if (!version) throw new Error("no [workspace.package] version in ../Cargo.toml");
  return version;
}

export default defineConfig({
  plugins: [react(), tailwindcss()],
  define: { __APP_VERSION__: JSON.stringify(workspaceVersion()) },
  build: { outDir: "dist", sourcemap: true },
});
