import clsx from "clsx";
import type { ReactNode } from "react";

export default function Landing({
  tone,
  status,
  children,
}: {
  tone: "wait" | "error" | "ended";
  status: string;
  children: ReactNode;
}) {
  const dot =
    tone === "error" ? "bg-highest" : tone === "ended" ? "bg-warn" : "bg-accent animate-pulse";
  return (
    <div className="min-h-screen grid place-content-center px-6">
      <div className="flex w-[min(30rem,70vw)] flex-col items-center gap-7 text-center">
        <div className="flex items-center gap-2 text-2xl font-semibold tracking-tight text-text">
          <img
            src="/images/diffthing-logo.png"
            alt="diffthing-logo"
            aria-hidden="true"
            className="h-8 w-8 shrink-0 object-contain"
          />
          diffthing
        </div>
        <div className="flex items-center gap-2 text-sm text-muted">
          <span className={clsx("h-2 w-2 rounded-full", dot)} />
          {status}
        </div>
        <div className="space-y-3 text-sm leading-relaxed text-muted">{children}</div>
        <div className="w-full text-left">
          <div className="mb-1.5 text-xs uppercase tracking-wide text-muted/70">
            Run in your project
          </div>
          <div className="flex items-center gap-2 rounded-lg border border-border bg-panel px-4 py-3 font-mono text-sm">
            <span className="select-none text-muted">$</span>
            <span className="text-text">npx diffthing</span>
          </div>
        </div>
        <div className="text-xs text-muted/70">AI organizes the diff. Only you review.</div>
      </div>
    </div>
  );
}
