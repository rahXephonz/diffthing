import type { ReactNode } from "react";

export const badge = "text-[11px] px-2 py-0.5 rounded-full border border-border text-muted";

export const chromeButton =
  "bg-transparent border border-border text-text rounded-md px-2.5 py-1 cursor-pointer hover:border-accent disabled:opacity-40 disabled:cursor-default disabled:hover:border-border";

export function Counts({ added, removed }: { added: number; removed: number }) {
  return (
    <span className="tabular-nums whitespace-nowrap">
      <span className="text-green-400">+{added}</span>{" "}
      <span className="text-highest">-{removed}</span>
    </span>
  );
}

/// Wrap every case-insensitive occurrence of `query` in a brand-green mark.
/// Plain text in, spans out — no regex on user input (query is escaped by
/// using indexOf, not a pattern), so search strings can't break rendering.
export function Highlight({ text, query }: { text: string; query: string }) {
  if (!query) return <>{text}</>;
  const lower = text.toLowerCase();
  const needle = query.toLowerCase();
  const parts: ReactNode[] = [];
  let from = 0;
  for (let at = lower.indexOf(needle, from); at !== -1; at = lower.indexOf(needle, from)) {
    if (at > from) parts.push(text.slice(from, at));
    parts.push(
      <mark
        key={`${at}-${needle}`}
        className="bg-green/20 text-green rounded-[2px] px-[1px] -mx-[1px]"
      >
        {text.slice(at, at + needle.length)}
      </mark>,
    );
    from = at + needle.length;
  }
  if (parts.length === 0) return <>{text}</>;
  if (from < text.length) parts.push(text.slice(from));
  return <>{parts}</>;
}

export function Kbd({ children }: { children: ReactNode }) {
  return (
    <code className="font-mono text-[0.85em] text-text bg-panel border border-border rounded px-1.5 py-0.5">
      {children}
    </code>
  );
}
