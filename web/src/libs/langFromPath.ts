import { bundledLanguagesInfo } from "shiki";

// Extension / filename -> Shiki language id, derived from Shiki's own bundled
// language registry rather than a hand-maintained list. Every grammar's id and
// aliases (which are almost always the file extensions: ts/cts/mts, rs, py, …)
// map to the canonical id, so all ~235 bundled languages are covered and the
// table can never drift out of sync with the installed Shiki version.
const LANG_BY_KEY = new Map<string, string>();
for (const lang of bundledLanguagesInfo) {
  LANG_BY_KEY.set(lang.id.toLowerCase(), lang.id);
  for (const alias of lang.aliases ?? []) LANG_BY_KEY.set(alias.toLowerCase(), lang.id);
}

// The handful of extensions Shiki doesn't expose as an id or alias — mostly
// C-family headers. Kept tiny on purpose; everything else comes from the map.
const EXT_OVERRIDES: Record<string, string> = {
  h: "c",
  hpp: "cpp",
  hh: "cpp",
  hxx: "cpp",
  cc: "cpp",
  cxx: "cpp",
};

/** Best-effort: an unknown extension renders as plain text, never an error. */
export function langFromPath(path: string): string {
  const base = (path.split("/").pop() ?? path).toLowerCase();
  // Extensionless, name-based files (Dockerfile, Makefile, …) resolve by name.
  if (!base.includes(".")) return LANG_BY_KEY.get(base) ?? "text";
  const ext = base.slice(base.lastIndexOf(".") + 1);
  return EXT_OVERRIDES[ext] ?? LANG_BY_KEY.get(ext) ?? "text";
}
