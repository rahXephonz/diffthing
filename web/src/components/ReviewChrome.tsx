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

export function Kbd({ children }: { children: ReactNode }) {
  return (
    <code className="font-mono text-[0.85em] text-text bg-panel border border-border rounded px-1.5 py-0.5">
      {children}
    </code>
  );
}
