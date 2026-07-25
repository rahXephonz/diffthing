import { useEffect, useMemo, useRef, useState } from "react";
import { Dialog } from "@base-ui-components/react/dialog";
import { History } from "lucide-react";
import clsx from "clsx";
import {
  iconUrlForPath,
  STATUS_CLASS,
  STATUS_LETTER,
  type FileStatus,
} from "../libs/fileIcon";
import { basename } from "../libs/utils";
import { Highlight } from "./ReviewChrome";
import type { FileDiff } from "../libs/protocol";

interface CommandPaletteProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  files: FileDiff[];
  iconsVersion: number;
  /** Navigate to the step that owns this file, then close. */
  onOpenFile: (path: string) => void;
}

interface Ranked {
  file: FileDiff;
  added: number;
  removed: number;
  /** Lower is better: 0 basename-prefix, 1 basename, 2 path. */
  rank: number;
  /** true when the file was opened from the palette before. */
  recent: boolean;
  /** recency position (0 = most recent), Infinity when never opened. */
  rec: number;
  /** original file-list index — the stable tiebreaker. */
  orig: number;
}

// Recently-opened file paths, most-recent-first, persisted so the list
// survives reloads. Capped to keep the palette focused.
const RECENT_KEY = "diffthing:palette-recent";
const RECENT_MAX = 8;

function loadRecent(): string[] {
  try {
    const v = JSON.parse(localStorage.getItem(RECENT_KEY) ?? "[]");
    return Array.isArray(v) ? v.filter((p): p is string => typeof p === "string") : [];
  } catch {
    return [];
  }
}

function saveRecent(v: string[]): void {
  try {
    localStorage.setItem(RECENT_KEY, JSON.stringify(v));
  } catch {
    /* storage unavailable — recency is best-effort */
  }
}

function dirname(path: string): string {
  const i = path.lastIndexOf("/");
  return i === -1 ? "" : path.slice(0, i + 1);
}

/** Substring match over basename then full path. Empty query lists recently
 *  opened files first (then original order); a query ranks tightest matches
 *  first, breaking ties toward more-recently-opened files. */
function rankFiles(files: FileDiff[], query: string, recency: Map<string, number>): Ranked[] {
  const q = query.trim().toLowerCase();
  const rows: Ranked[] = [];
  files.forEach((file, orig) => {
    let added = 0;
    let removed = 0;
    for (const hunk of file.hunks) {
      added += hunk.added;
      removed += hunk.removed;
    }
    const base = basename(file.path).toLowerCase();
    const path = file.path.toLowerCase();
    let rank: number;
    if (q === "") rank = 0;
    else if (base.startsWith(q)) rank = 0;
    else if (base.includes(q)) rank = 1;
    else if (path.includes(q)) rank = 2;
    else return;
    const rec = recency.get(file.path);
    rows.push({ file, added, removed, rank, recent: rec !== undefined, rec: rec ?? Infinity, orig });
  });
  if (q === "") {
    rows.sort((a, b) => a.rec - b.rec || a.orig - b.orig);
  } else {
    rows.sort((a, b) => a.rank - b.rank || a.rec - b.rec || a.file.path.localeCompare(b.file.path));
  }
  return rows;
}

export default function CommandPalette({
  open,
  onOpenChange,
  files,
  iconsVersion,
  onOpenFile,
}: CommandPaletteProps) {
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const [recent, setRecent] = useState<string[]>(() => loadRecent());
  const listRef = useRef<HTMLUListElement>(null);

  const recency = useMemo(() => new Map(recent.map((p, i) => [p, i] as const)), [recent]);
  // iconsVersion is read so the memo recomputes once icons resolve, swapping
  // the placeholder for the real icon URL.
  const results = useMemo(() => rankFiles(files, query, recency), [files, query, recency]);
  void iconsVersion;

  // Query persists across opens — it only clears when the user empties the
  // input. Just reset the cursor to the top on open. Render-phase adjustment
  // (the React "previous value" pattern) rather than an effect, so there's no
  // extra commit + flash of stale results.
  const [prevOpen, setPrevOpen] = useState(open);
  if (open !== prevOpen) {
    setPrevOpen(open);
    if (open) {
      setActive(0);
    }
  }

  // Clamp on read so a shrinking result set can't leave the cursor past the
  // end — no effect, no transient out-of-range index.
  const activeIndex = results.length === 0 ? 0 : Math.min(active, results.length - 1);

  // Keep the active row visible as the cursor moves.
  useEffect(() => {
    const el = listRef.current?.querySelector<HTMLElement>(`[data-index="${activeIndex}"]`);
    el?.scrollIntoView({ block: "nearest" });
  }, [activeIndex]);

  const onChoose = (path: string) => {
    setRecent((prev) => {
      const next = [path, ...prev.filter((p) => p !== path)].slice(0, RECENT_MAX);
      saveRecent(next);
      return next;
    });
    onOpenFile(path);
    onOpenChange(false);
  };

  const onKeyDown = (event: React.KeyboardEvent) => {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setActive(Math.min(results.length - 1, activeIndex + 1));
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setActive(Math.max(0, activeIndex - 1));
    } else if (event.key === "Enter") {
      event.preventDefault();
      const hit = results[activeIndex];
      if (hit) onChoose(hit.file.path);
    }
  };

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Backdrop className="fixed inset-0 z-50 bg-black/50 backdrop-blur-[1px]" />
        <Dialog.Popup className="fixed left-1/2 top-[15%] z-50 w-[min(640px,90vw)] -translate-x-1/2 overflow-hidden rounded-xl border border-border bg-panel shadow-2xl">
          <Dialog.Title className="sr-only">Search files</Dialog.Title>
          <input
            autoFocus
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={onKeyDown}
            placeholder="Search files by name or path…"
            className="w-full border-b border-border bg-transparent px-4 py-3 text-sm text-text outline-none placeholder:text-muted"
            spellCheck={false}
            autoComplete="off"
          />
          <ul ref={listRef} className="max-h-[50vh] overflow-y-auto py-1">
            {results.length === 0 && (
              <li className="px-4 py-6 text-center text-sm text-muted">No matching files</li>
            )}
            {results.map((row, i) => {
              const iconUrl = iconUrlForPath(row.file.path);
              const dir = dirname(row.file.path);
              const status = row.file.status as FileStatus;
              return (
                <li key={row.file.path} data-index={i}>
                  <button
                    type="button"
                    onClick={() => onChoose(row.file.path)}
                    onMouseMove={() => setActive(i)}
                    className={clsx(
                      "flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm",
                      i === activeIndex ? "bg-accent/15 text-text" : "text-muted hover:text-text",
                    )}
                  >
                    <span className="flex h-4 w-4 shrink-0 items-center justify-center">
                      {iconUrl ? (
                        <img src={iconUrl} alt="" className="h-4 w-4" draggable={false} />
                      ) : (
                        <span className="h-4 w-4" />
                      )}
                    </span>
                    <span className="truncate text-text">
                      <Highlight text={basename(row.file.path)} query={query} />
                    </span>
                    {dir && (
                      <span className="truncate text-xs text-muted">
                        <Highlight text={dir} query={query} />
                      </span>
                    )}
                    <span className="ml-auto flex shrink-0 items-center gap-2 tabular-nums">
                      {row.recent ? (
                        <span className="flex items-center gap-1 text-xs text-muted">
                          <History size={12} aria-hidden="true" />
                          recently opened
                        </span>
                      ) : (
                        (row.added > 0 || row.removed > 0) && (
                          <span className="text-xs">
                            <span className="text-green-400">+{row.added}</span>{" "}
                            <span className="text-highest">-{row.removed}</span>
                          </span>
                        )
                      )}
                      <span className={clsx("text-xs font-medium", STATUS_CLASS[status])}>
                        {STATUS_LETTER[status]}
                      </span>
                    </span>
                  </button>
                </li>
              );
            })}
          </ul>
        </Dialog.Popup>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
