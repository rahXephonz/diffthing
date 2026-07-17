// Extension -> Shiki language id. Deliberately small and best-effort: an
// unknown extension just renders as plain text, never an error.
const EXT_LANG: Record<string, string> = {
  ts: "typescript",
  tsx: "tsx",
  js: "javascript",
  jsx: "jsx",
  mjs: "javascript",
  cjs: "javascript",
  json: "json",
  jsonc: "jsonc",
  rs: "rust",
  toml: "toml",
  yml: "yaml",
  yaml: "yaml",
  css: "css",
  scss: "scss",
  html: "html",
  md: "markdown",
  mdx: "mdx",
  sh: "bash",
  bash: "bash",
  zsh: "bash",
  py: "python",
  go: "go",
  rb: "ruby",
  php: "php",
  sql: "sql",
  c: "c",
  h: "c",
  cpp: "cpp",
  hpp: "cpp",
  java: "java",
  kt: "kotlin",
  swift: "swift",
  dockerfile: "dockerfile",
  graphql: "graphql",
  xml: "xml",
  ini: "ini",
  vue: "vue",
  svelte: "svelte",
};

export const SUPPORTED_LANGS = Array.from(new Set(Object.values(EXT_LANG)));

export function langFromPath(path: string): string {
  const base = path.split("/").pop() ?? path;
  if (base.toLowerCase() === "dockerfile") return "dockerfile";
  const ext = base.includes(".") ? base.split(".").pop()!.toLowerCase() : "";
  return EXT_LANG[ext] ?? "text";
}
