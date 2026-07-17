import { getIconForFilePath } from "vscode-material-icons";

// Resolved straight from the package via Vite's asset pipeline — nothing
// copied into public/. Lazy (not eager): an eager glob pulled all 910
// SVGs' content into the main JS bundle (+900KB) despite `?url` — each
// icon is now only fetched as a separate asset when actually requested.
const iconLoaders = import.meta.glob<string>(
  "../../node_modules/vscode-material-icons/generated/icons/*.svg",
  { query: "?url", import: "default" },
);

const loaderByName = new Map<string, () => Promise<string>>();
for (const [path, loader] of Object.entries(iconLoaders)) {
  const name = path
    .split("/")
    .pop()!
    .replace(/\.svg$/, "");
  loaderByName.set(name, loader);
}

const resolvedCache = new Map<string, string>();

/** Cached by icon name (finite set, ~250 distinct icons) — same hunk/file
 *  never re-fetches. Returns undefined until the icon has loaded once;
 *  components should re-render when resolveIconUrl's promise settles. */
export function iconUrlForPath(path: string): string | undefined {
  const name = getIconForFilePath(path);
  return resolvedCache.get(name);
}

export async function preloadIconForPath(path: string): Promise<void> {
  const name = getIconForFilePath(path);
  if (resolvedCache.has(name)) return;
  const loader = loaderByName.get(name);
  if (!loader) return;
  resolvedCache.set(name, await loader());
}

export type FileStatus = "added" | "modified" | "deleted" | "renamed";

export const STATUS_LETTER: Record<FileStatus, string> = {
  added: "A",
  modified: "M",
  deleted: "D",
  renamed: "R",
};

export const STATUS_CLASS: Record<FileStatus, string> = {
  added: "text-green-400",
  modified: "text-warn",
  deleted: "text-highest",
  renamed: "text-medium",
};
