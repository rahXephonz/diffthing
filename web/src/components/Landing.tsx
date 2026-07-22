import clsx from "clsx";
import type { ReactNode } from "react";
import type { BrowserHelp } from "../libs/connection";

/** Render inline `backtick` spans as <code>, everything else as text. */
function withCode(text: string): ReactNode[] {
  return text.split(/(`[^`]+`)/g).map((part, i) =>
    part.startsWith("`") && part.endsWith("`") ? (
      <code
        key={i}
        className="font-mono text-[0.85em] text-text bg-panel border border-border rounded px-1 py-0.5"
      >
        {part.slice(1, -1)}
      </code>
    ) : (
      <span key={i}>{part}</span>
    ),
  );
}

export default function Landing({
  tone,
  status,
  children,
  daemonPort,
  help,
}: {
  tone: "wait" | "error" | "ended";
  status: string;
  children?: ReactNode;
  /** Shown while connecting so the reader sees which daemon we're dialing. */
  daemonPort?: number | null;
  /** Per-browser remediation card for the "browser blocked" diagnosis. */
  help?: BrowserHelp | null;
}) {
  const dot =
    tone === "error" ? "bg-highest" : tone === "ended" ? "bg-warn" : "bg-accent animate-pulse";
  return (
    <div className="min-h-screen grid place-content-center px-6">
      <div className="flex w-[min(34rem,80vw)] flex-col items-center gap-7 text-center">
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
          {daemonPort ? <span className="text-muted/60">· daemon on :{daemonPort}</span> : null}
        </div>
        {children && <div className="space-y-3 text-sm leading-relaxed text-muted">{children}</div>}

        {help && (
          <div className="w-full rounded-lg border border-border bg-panel/60 px-5 py-4 text-left">
            <div className="mb-2 text-sm font-medium text-text">{help.label}</div>
            <ol className="space-y-1.5 text-sm leading-relaxed text-muted">
              {help.steps.map((step, i) => (
                <li key={i} className="flex gap-2">
                  <span className="select-none text-muted/50">{i + 1}.</span>
                  <span>{withCode(step)}</span>
                </li>
              ))}
            </ol>
          </div>
        )}

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
