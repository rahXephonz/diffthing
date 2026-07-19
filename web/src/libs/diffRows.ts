import type { Hunk } from "./protocol";

export type LineType = "context" | "add" | "del";

export interface UnifiedRow {
  type: LineType;
  oldLine: number | null;
  newLine: number | null;
  content: string;
  /** Index into the old/new side's token array (see HunkModel), or null
   *  if this row has no counterpart on that side. */
  oldIdx: number | null;
  newIdx: number | null;
  /** Index into hunk.lines — the stable per-line comment anchor. Stable as
   *  long as the hunk content is (content hash), so keying a comment off it
   *  respects invariant 2: identity is the hunk, this is a render offset. */
  rawIdx: number;
}

export interface SplitCell {
  type: LineType;
  line: number;
  content: string;
  idx: number;
  /** Index into hunk.lines for per-line review comments. */
  rawIdx: number;
}

export interface SplitRow {
  left: SplitCell | null;
  right: SplitCell | null;
}

export interface HunkModel {
  /** Old-file text reconstructed from context+removed lines, one line per
   *  entry in oldText.split("\n") — index N here is token-array index N. */
  oldText: string;
  /** Same for context+added lines. */
  newText: string;
  unified: UnifiedRow[];
  split: SplitRow[];
}

function classify(raw: string): LineType | "nonewline" {
  if (raw.startsWith("+")) return "add";
  if (raw.startsWith("-")) return "del";
  if (raw.startsWith("\\")) return "nonewline";
  return "context";
}

const stripMarker = (raw: string) => raw.slice(1);

/**
 * Single pass over the hunk's raw lines producing everything the diff pane
 * needs: reconstructed old/new side text (for per-side Shiki tokenization,
 * which needs surrounding-line context to highlight correctly) plus both
 * row layouts, each row carrying the index into that tokenized side so
 * rendering never re-derives "which token line is this" — one walk is the
 * single source of truth for the old/new line filters.
 */
export function buildHunkModel(hunk: Hunk): HunkModel {
  const oldLines: string[] = [];
  const newLines: string[] = [];
  const unified: UnifiedRow[] = [];

  // Split view: buffer consecutive del/add runs, pair them off row by row
  // (same convention GitHub/GitLab use), context lines flush the buffer.
  const split: SplitRow[] = [];
  let delBuf: { line: number; idx: number; content: string; rawIdx: number }[] = [];
  let addBuf: { line: number; idx: number; content: string; rawIdx: number }[] = [];
  const flushSplit = () => {
    const n = Math.max(delBuf.length, addBuf.length);
    for (let i = 0; i < n; i++) {
      const d = delBuf[i];
      const a = addBuf[i];
      split.push({
        left: d
          ? { type: "del", line: d.line, content: d.content, idx: d.idx, rawIdx: d.rawIdx }
          : null,
        right: a
          ? { type: "add", line: a.line, content: a.content, idx: a.idx, rawIdx: a.rawIdx }
          : null,
      });
    }
    delBuf = [];
    addBuf = [];
  };

  let oldLine = hunk.old_start;
  let newLine = hunk.new_start;

  for (let rawIdx = 0; rawIdx < hunk.lines.length; rawIdx++) {
    const raw = hunk.lines[rawIdx];
    const type = classify(raw);
    if (type === "nonewline") continue;
    const text = stripMarker(raw);

    const oldIdx = type !== "add" ? oldLines.length : null;
    const newIdx = type !== "del" ? newLines.length : null;
    if (type !== "add") oldLines.push(text);
    if (type !== "del") newLines.push(text);

    unified.push({
      type,
      oldLine: type !== "add" ? oldLine : null,
      newLine: type !== "del" ? newLine : null,
      content: text,
      oldIdx,
      newIdx,
      rawIdx,
    });

    if (type === "del") {
      delBuf.push({ line: oldLine, idx: oldIdx!, content: text, rawIdx });
    } else if (type === "add") {
      addBuf.push({ line: newLine, idx: newIdx!, content: text, rawIdx });
    } else {
      flushSplit();
      split.push({
        left: { type: "context", line: oldLine, content: text, idx: oldIdx!, rawIdx },
        right: { type: "context", line: newLine, content: text, idx: newIdx!, rawIdx },
      });
    }

    if (type !== "add") oldLine++;
    if (type !== "del") newLine++;
  }
  flushSplit();

  return { oldText: oldLines.join("\n"), newText: newLines.join("\n"), unified, split };
}
