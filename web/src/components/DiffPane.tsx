import { useEffect, useMemo, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import clsx from "clsx";
import type { Highlighter, ThemedToken } from "shiki";
import { buildHunkModel, type SplitCell, type UnifiedRow } from "../libs/diffRows";
import { ensureLang, getHighlighter, tokenizeSide } from "../libs/highlighter";
import { langFromPath } from "../libs/langFromPath";
import type { Flag, Hunk, ImpactScore } from "../libs/protocol";
import type { DispatchState } from "../libs/store";
import CommentThread from "./CommentThread";

export type ViewMode = "unified" | "split";
export type HunkStatus = "unviewed" | "viewed" | "changed_since_viewed";

interface Props {
  hunks: Hunk[];
  scores: Record<string, ImpactScore>;
  statusOf: (id: string) => HunkStatus;
  onMarkViewed: (id: string) => void;
  /** New thread OR reply. line = index into hunk.lines, or null (hunk-level). */
  onFlag: (id: string, line: number | null, comment: string) => void;
  onResolve: (id: string, line: number | null) => void;
  onDispatch: (id: string, line: number | null, instruction: string) => void;
  flags: Flag[];
  dispatch: DispatchState | null;
  viewMode: ViewMode;
}

/** Stable composer/thread key for a comment anchor: a hunk line, or the
 *  hunk itself ("H"). */
const anchorKey = (hunkId: string, line: number | null) =>
  `${hunkId}::${line === null ? "H" : line}`;

const IMPACT_CLASS: Record<string, string> = {
  highest: "border-highest/60 text-highest bg-highest/10",
  high: "border-high/60 text-high bg-high/10",
  medium: "border-medium/60 text-medium bg-medium/10",
  low: "border-low/40 text-low bg-low/10",
};

const badge = "text-[11px] px-2 py-0.5 rounded-full border border-border text-muted";
const chromeButton =
  "bg-transparent border border-border text-text rounded-md px-2.5 py-1 cursor-pointer hover:border-accent";

const HEADER_HEIGHT = 41;
const LINE_HEIGHT = 20;
const THREAD_ESTIMATE = 160;

type FlatRow =
  | { kind: "header"; hunk: Hunk }
  | { kind: "unified"; hunk: Hunk; row: UnifiedRow }
  | { kind: "split"; hunk: Hunk; left: SplitCell | null; right: SplitCell | null }
  | { kind: "thread"; hunk: Hunk; line: number | null };

function Tokens({ tokens, plain }: { tokens: ThemedToken[] | undefined; plain: string }) {
  if (!tokens) return <>{plain}</>;
  return (
    <>
      {tokens.map((t, i) => (
        <span key={i} style={{ color: t.color }}>
          {t.content}
        </span>
      ))}
    </>
  );
}

const ROW_BG: Record<string, string> = {
  add: "bg-add",
  del: "bg-del",
  context: "",
};

const GUTTER_CLASS: Record<string, string> = {
  add: "text-green-400",
  del: "text-highest",
  context: "text-muted",
};

export default function DiffPane({
  hunks,
  scores,
  statusOf,
  onMarkViewed,
  onFlag,
  onResolve,
  onDispatch,
  flags,
  dispatch,
  viewMode,
}: Props) {
  const [highlighter, setHighlighter] = useState<Highlighter | null>(null);
  // Composer state lives here (not per-row), keyed by anchor, so a draft
  // survives the row unmounting when virtualization scrolls it out of view.
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [composerOpen, setComposerOpen] = useState<Set<string>>(new Set());
  const [expandedViewed, setExpandedViewed] = useState<Set<string>>(new Set());

  const threadsAt = (id: string, line: number | null) =>
    flags.filter((f) => f.hunk === id && (f.line ?? null) === line);
  const threadKeys = useMemo(
    () => new Set(flags.map((f) => anchorKey(f.hunk, f.line ?? null))),
    [flags],
  );
  const showThreadAt = (id: string, line: number | null) => {
    const k = anchorKey(id, line);
    return threadKeys.has(k) || composerOpen.has(k);
  };
  const hunkCommentCount = (id: string) =>
    flags.filter((f) => f.hunk === id).reduce((n, f) => n + f.thread.length, 0);
  // Comments remain attached while collapsed. Header count tells reviewer
  // they exist; expanding restores threads and drafts unchanged.
  const isCollapsed = (id: string) => statusOf(id) === "viewed" && !expandedViewed.has(id);
  const toggleViewed = (id: string) =>
    setExpandedViewed((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });

  const openComposer = (k: string) => setComposerOpen((s) => new Set(s).add(k));
  const closeComposer = (k: string) =>
    setComposerOpen((s) => {
      const n = new Set(s);
      n.delete(k);
      return n;
    });
  const setDraft = (k: string, v: string) => setDrafts((d) => ({ ...d, [k]: v }));
  const submitDraft = (id: string, line: number | null) => {
    const k = anchorKey(id, line);
    const text = (drafts[k] ?? "").trim();
    if (!text) return;
    onFlag(id, line, text);
    setDraft(k, "");
    closeComposer(k);
  };
  // Bumped once the langs needed by the current hunks finish loading —
  // tokensFor reads live off highlighter.getLoadedLanguages(), this exists
  // purely to force a re-render when that set changes underneath us.
  const [langsVersion, setLangsVersion] = useState(0);

  useEffect(() => {
    let cancelled = false;
    getHighlighter().then((h) => {
      if (!cancelled) setHighlighter(h);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (!highlighter) return;
    let cancelled = false;
    const langs = new Set(hunks.map((h) => langFromPath(h.path)));
    Promise.all([...langs].map((l) => ensureLang(highlighter, l))).then(() => {
      if (!cancelled) setLangsVersion((v) => v + 1);
    });
    return () => {
      cancelled = true;
    };
  }, [highlighter, hunks]);

  const models = useMemo(() => {
    const m = new Map<string, ReturnType<typeof buildHunkModel>>();
    for (const h of hunks) m.set(h.id, buildHunkModel(h));
    return m;
  }, [hunks]);

  const flatRows = useMemo<FlatRow[]>(() => {
    const rows: FlatRow[] = [];
    for (const hunk of hunks) {
      rows.push({ kind: "header", hunk });
      if (isCollapsed(hunk.id)) continue;
      // Hunk-level thread (the header 💬 button) sits directly under the
      // header — never off-screen at the bottom of a long hunk.
      if (showThreadAt(hunk.id, null)) rows.push({ kind: "thread", hunk, line: null });
      const model = models.get(hunk.id)!;
      if (viewMode === "unified") {
        // Per-line: each diff line can carry its own thread, GitHub-style.
        for (const row of model.unified) {
          rows.push({ kind: "unified", hunk, row });
          if (showThreadAt(hunk.id, row.rawIdx))
            rows.push({ kind: "thread", hunk, line: row.rawIdx });
        }
      } else {
        for (const r of model.split) {
          rows.push({ kind: "split", hunk, left: r.left, right: r.right });
          const anchors = new Set(
            [r.left?.rawIdx, r.right?.rawIdx].filter((line): line is number => line !== undefined),
          );
          for (const line of anchors) {
            if (showThreadAt(hunk.id, line)) rows.push({ kind: "thread", hunk, line });
          }
        }
      }
    }
    return rows;
    // Depends on threadKeys + composerOpen (drive showThreadAt).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [hunks, models, viewMode, threadKeys, composerOpen, expandedViewed, flags]);

  const scrollRef = useRef<HTMLDivElement>(null);
  // react-virtual returns non-memoizable functions by design; component already virtualizes manually.
  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({
    count: flatRows.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: (i) => {
      const k = flatRows[i].kind;
      if (k === "header") return HEADER_HEIGHT;
      if (k === "thread") return THREAD_ESTIMATE; // measured precisely after mount
      return LINE_HEIGHT;
    },
    overscan: 20,
  });

  const tokensFor = (
    hunk: Hunk,
    side: "old" | "new",
    idx: number | null,
  ): ThemedToken[] | undefined => {
    if (idx === null || !highlighter) return undefined;
    const lang = langFromPath(hunk.path);
    // Don't tokenize (and thus cache) against a language that hasn't
    // finished loading yet — tokenizeSide's cache is permanent per
    // hunk+side, so a premature "text" fallback would stick forever.
    if (lang !== "text" && !highlighter.getLoadedLanguages().includes(lang as never)) {
      return undefined;
    }

    void langsVersion; // re-run this render when the loaded-langs set changes
    const model = models.get(hunk.id)!;
    const text = side === "old" ? model.oldText : model.newText;
    const tokens = tokenizeSide(highlighter, hunk.id, side, text, lang);
    return tokens[idx];
  };

  return (
    <div ref={scrollRef} className="h-full overflow-y-auto">
      <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
        {virtualizer.getVirtualItems().map((vi) => {
          const item = flatRows[vi.index];
          const style: React.CSSProperties = {
            position: "absolute",
            top: 0,
            left: 0,
            width: "100%",
            transform: `translateY(${vi.start}px)`,
          };

          if (item.kind === "header") {
            const { hunk } = item;
            const score = scores[hunk.id];
            const status = statusOf(hunk.id);
            return (
              <div
                key={vi.key}
                data-index={vi.index}
                ref={virtualizer.measureElement}
                style={style}
                className={clsx(
                  "flex items-center gap-2 flex-wrap px-3 py-2 bg-panel border-b border-t border-border",
                  status === "changed_since_viewed" && "border-t-warn",
                )}
              >
                <code className="font-code text-[13px]">{hunk.path}</code>
                {score && (
                  <span
                    className={clsx(badge, IMPACT_CLASS[score.impact])}
                    title={score.reasons.join(", ")}
                  >
                    {score.impact} — {score.reasons[0]}
                  </span>
                )}
                {status === "changed_since_viewed" && (
                  <span className={clsx(badge, "border-warn/60 text-warn bg-warn/10")}>
                    changed since viewed
                  </span>
                )}
                <button className={chromeButton} onClick={() => onMarkViewed(hunk.id)}>
                  {status === "viewed" ? "viewed ✓" : "mark viewed"}
                </button>
                {status === "viewed" && (
                  <button className={chromeButton} onClick={() => toggleViewed(hunk.id)}>
                    {isCollapsed(hunk.id) ? "expand" : "collapse"}
                  </button>
                )}
                <button
                  className={chromeButton}
                  onClick={() => openComposer(anchorKey(hunk.id, null))}
                  title="Comment on this hunk"
                >
                  💬 comment
                  {hunkCommentCount(hunk.id) > 0 && (
                    <span className="ml-1 text-accent">{hunkCommentCount(hunk.id)}</span>
                  )}
                </button>
              </div>
            );
          }

          if (item.kind === "thread") {
            const { hunk, line } = item;
            const k = anchorKey(hunk.id, line);
            const threads = threadsAt(hunk.id, line);
            return (
              <div
                key={vi.key}
                data-index={vi.index}
                ref={virtualizer.measureElement}
                style={style}
              >
                <div
                  className={clsx(
                    viewMode === "split" && line !== null && "ml-[50%] w-[50%] box-border",
                  )}
                >
                  <CommentThread
                    flags={threads}
                    dispatch={dispatch}
                    draft={drafts[k] ?? ""}
                    onDraftChange={(v) => setDraft(k, v)}
                    onSubmit={() => submitDraft(hunk.id, line)}
                    onResolve={() => onResolve(hunk.id, line)}
                    onDispatch={(instruction) => onDispatch(hunk.id, line, instruction)}
                    onCancel={() => {
                      setDraft(k, "");
                      closeComposer(k);
                    }}
                    composerOnly={composerOpen.has(k)}
                  />
                </div>
              </div>
            );
          }

          if (item.kind === "unified") {
            const { row } = item;
            const side = row.type === "del" ? "old" : "new";
            const idx = row.type === "del" ? row.oldIdx : row.newIdx;
            return (
              <div
                key={vi.key}
                data-index={vi.index}
                ref={virtualizer.measureElement}
                style={style}
                className={clsx(
                  "group relative flex font-code text-[13px] whitespace-pre overflow-x-auto",
                  ROW_BG[row.type],
                )}
              >
                {row.type !== "del" && (
                  <button
                    className="absolute left-0 top-0 z-10 h-full w-5 cursor-pointer grid place-content-center rounded-sm bg-accent text-bg text-sm font-bold leading-none opacity-0 group-hover:opacity-100"
                    title="Comment on this line"
                    onClick={() => openComposer(anchorKey(item.hunk.id, row.rawIdx))}
                  >
                    +
                  </button>
                )}
                <span
                  className={clsx(
                    "w-12 shrink-0 text-right pr-2 select-none",
                    GUTTER_CLASS[row.type],
                  )}
                >
                  {row.oldLine ?? ""}
                </span>
                <span
                  className={clsx(
                    "w-12 shrink-0 text-right pr-2 select-none",
                    GUTTER_CLASS[row.type],
                  )}
                >
                  {row.newLine ?? ""}
                </span>
                <span className="px-2">
                  <Tokens tokens={tokensFor(item.hunk, side, idx)} plain={row.content} />
                </span>
              </div>
            );
          }

          // split
          const { left, right } = item;
          return (
            <div
              key={vi.key}
              data-index={vi.index}
              ref={virtualizer.measureElement}
              style={style}
              className="flex"
            >
              <div
                className={clsx(
                  "group relative flex flex-1 font-code text-[13px] whitespace-pre overflow-x-auto",
                  left && ROW_BG[left.type],
                )}
              >
                <span
                  className={clsx(
                    "w-12 shrink-0 text-right pr-2 select-none",
                    left ? GUTTER_CLASS[left.type] : "text-muted",
                  )}
                >
                  {left?.line ?? ""}
                </span>
                <span className="px-2">
                  {left && (
                    <Tokens tokens={tokensFor(item.hunk, "old", left.idx)} plain={left.content} />
                  )}
                </span>
              </div>
              <div className="w-px bg-border" />
              <div
                className={clsx(
                  "group relative flex flex-1 font-code text-[13px] whitespace-pre overflow-x-auto",
                  right && ROW_BG[right.type],
                )}
              >
                {right && (
                  <button
                    className="absolute left-0 top-0 z-10 h-full w-5 cursor-pointer grid place-content-center rounded-sm bg-accent text-bg text-sm font-bold leading-none opacity-0 group-hover:opacity-100 focus:opacity-100"
                    title="Comment on this line"
                    onClick={() => openComposer(anchorKey(item.hunk.id, right.rawIdx))}
                  >
                    +
                  </button>
                )}
                <span
                  className={clsx(
                    "w-12 shrink-0 text-right pr-2 select-none",
                    right ? GUTTER_CLASS[right.type] : "text-muted",
                  )}
                >
                  {right?.line ?? ""}
                </span>
                <span className="px-2">
                  {right && (
                    <Tokens tokens={tokensFor(item.hunk, "new", right.idx)} plain={right.content} />
                  )}
                </span>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
