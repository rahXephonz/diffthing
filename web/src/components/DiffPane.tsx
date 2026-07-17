import { useEffect, useMemo, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import clsx from "clsx";
import type { Highlighter, ThemedToken } from "shiki";
import { buildHunkModel, type SplitCell, type UnifiedRow } from "../libs/diffRows";
import { ensureLang, getHighlighter, tokenizeSide } from "../libs/highlighter";
import { langFromPath } from "../libs/langFromPath";
import type { Hunk, ImpactScore } from "../libs/protocol";

export type ViewMode = "unified" | "split";
export type HunkStatus = "unviewed" | "viewed" | "changed_since_viewed";

interface Props {
  hunks: Hunk[];
  scores: Record<string, ImpactScore>;
  statusOf: (id: string) => HunkStatus;
  onMarkViewed: (id: string) => void;
  onFlag: (id: string, comment: string) => void;
  viewMode: ViewMode;
}

const IMPACT_CLASS: Record<string, string> = {
  highest: "border-highest text-highest",
  high: "border-high text-high",
  medium: "border-medium text-medium",
  low: "text-low",
};

const badge = "text-[11px] px-2 py-0.5 rounded-full border border-border text-muted";
const chromeButton =
  "bg-transparent border border-border text-text rounded-md px-2.5 py-1 cursor-pointer hover:border-accent";

const HEADER_HEIGHT = 41;
const LINE_HEIGHT = 20;

type FlatRow =
  | { kind: "header"; hunk: Hunk }
  | { kind: "unified"; hunk: Hunk; row: UnifiedRow }
  | { kind: "split"; hunk: Hunk; left: SplitCell | null; right: SplitCell | null };

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
  viewMode,
}: Props) {
  const [highlighter, setHighlighter] = useState<Highlighter | null>(null);
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
      const model = models.get(hunk.id)!;
      if (viewMode === "unified") {
        for (const row of model.unified) rows.push({ kind: "unified", hunk, row });
      } else {
        for (const r of model.split)
          rows.push({ kind: "split", hunk, left: r.left, right: r.right });
      }
    }
    return rows;
  }, [hunks, models, viewMode]);

  const scrollRef = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: flatRows.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: (i) => (flatRows[i].kind === "header" ? HEADER_HEIGHT : LINE_HEIGHT),
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
                  <span className={`${badge} border-warn text-warn`}>changed since viewed</span>
                )}
                <button className={chromeButton} onClick={() => onMarkViewed(hunk.id)}>
                  {status === "viewed" ? "viewed ✓" : "mark viewed"}
                </button>
                <button
                  className={chromeButton}
                  onClick={() => {
                    const comment = prompt("Flag comment:");
                    if (comment) onFlag(hunk.id, comment);
                  }}
                >
                  flag
                </button>
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
                  "flex font-code text-[13px] whitespace-pre overflow-x-auto",
                  ROW_BG[row.type],
                )}
              >
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
                  "flex-1 flex font-code text-[13px] whitespace-pre overflow-x-auto",
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
                  "flex-1 flex font-code text-[13px] whitespace-pre overflow-x-auto",
                  right && ROW_BG[right.type],
                )}
              >
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
