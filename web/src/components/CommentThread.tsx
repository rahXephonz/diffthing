import { useRef, useState, type ReactNode } from "react";
import clsx from "clsx";
import {
  Bold,
  Bot,
  CircleCheck,
  Code,
  Heading,
  Italic,
  Link,
  List,
  ListOrdered,
  ListTodo,
  Quote,
  TriangleAlert,
  User,
} from "lucide-react";
import type { DispatchState } from "../libs/store";
import type { Flag, FlagEntry } from "../libs/protocol";

interface Props {
  flags: Flag[];
  dispatch: DispatchState | null;
  draft: string;
  onDraftChange: (v: string) => void;
  onSubmit: () => void; // add_flag → append reply / open thread
  onSubmitAndDispatch: () => void; // add_flag, then request_change using that comment
  onResolve: () => void; // close_flag (human only)
  onDispatch: (instruction: string) => void; // request_change
  onCancel: () => void; // close the bare composer
  composerOnly: boolean; // opened via "comment" with no thread yet
}

type Author = { label: string; icon: ReactNode; cls: string };

const AUTHOR: Record<FlagEntry["kind"], Author> = {
  human_comment: { label: "You", icon: <User size={12} />, cls: "bg-accent/20 text-accent" },
  agent_response: { label: "Agent", icon: <Bot size={12} />, cls: "bg-accent/20 text-accent" },
  agent_claim: { label: "Agent", icon: <Bot size={12} />, cls: "bg-green/20 text-green" },
  dispatch_note: {
    label: "diffthing",
    icon: <TriangleAlert size={12} />,
    cls: "bg-warn/20 text-warn",
  },
};

function Avatar({ kind }: { kind: FlagEntry["kind"] }) {
  const a = AUTHOR[kind];
  return (
    <span className={clsx("w-5 h-5 shrink-0 rounded-full grid place-content-center", a.cls)}>
      {a.icon}
    </span>
  );
}

function Comment({ entry, onAskAgent }: { entry: FlagEntry; onAskAgent?: () => void }) {
  const a = AUTHOR[entry.kind];

  return (
    <div className="border-border first:border-t-0">
      <div className="flex items-center gap-2 px-3 py-1.5">
        <Avatar kind={entry.kind} />
        <span className="text-xs font-semibold">{a.label}</span>
        {entry.kind === "agent_claim" && (
          <span className="text-[10px] px-1.5 py-0.5 rounded-full border border-green/50 text-green">
            claim · unverified
          </span>
        )}
        {entry.kind === "dispatch_note" && (
          <span className="text-[10px] text-warn">dispatch note</span>
        )}
      </div>
      <div className="px-3 pb-2">
        <MarkdownPreview source={entry.body} />
      </div>
      {onAskAgent && (
        <div className="flex px-3 pb-2">
          <button
            className="inline-flex items-center gap-1 rounded-md border border-border bg-transparent px-2 py-1 text-[11px] text-muted transition-colors hover:border-accent hover:text-accent disabled:cursor-default disabled:opacity-40"
            onClick={onAskAgent}
            title="Ask your agent about this comment"
          >
            <Bot size={12} /> Ask agent
          </button>
        </div>
      )}
      {entry.kind === "agent_claim" && (
        <p className="px-3 pb-2 -mt-1 text-[10px] text-muted italic m-0">
          Reconciliation confirms the code actually changed — you decide if it's right.
        </p>
      )}
    </div>
  );
}

function inlineMarkdown(text: string): ReactNode[] {
  const parts: ReactNode[] = [];
  const pattern = /(`[^`]+`|\*\*[^*]+\*\*|__[^_]+__|\*[^*]+\*|_([^_]+)_|\[[^\]]+\]\([^)]+\))/g;
  let cursor = 0;
  for (const match of text.matchAll(pattern)) {
    const index = match.index ?? 0;
    if (index > cursor) parts.push(text.slice(cursor, index));
    const token = match[0];
    if (token.startsWith("`")) {
      parts.push(
        <code key={index} className="rounded bg-bg px-1 font-code text-[0.9em]">
          {token.slice(1, -1)}
        </code>,
      );
    } else if (token.startsWith("**") || token.startsWith("__")) {
      parts.push(<strong key={index}>{token.slice(2, -2)}</strong>);
    } else if (token.startsWith("[")) {
      const link = token.match(/^\[([^\]]+)\]\(([^)]+)\)$/);
      const href = link?.[2];
      const safe = href && /^(https?:\/\/|mailto:|#|\/)/i.test(href);
      parts.push(
        link && safe ? (
          <a
            key={index}
            href={href}
            target="_blank"
            rel="noreferrer"
            className="text-accent underline"
          >
            {link[1]}
          </a>
        ) : (
          token
        ),
      );
    } else {
      parts.push(<em key={index}>{token.slice(1, -1)}</em>);
    }
    cursor = index + token.length;
  }
  if (cursor < text.length) parts.push(text.slice(cursor));
  return parts;
}

function MarkdownPreview({ source }: { source: string }) {
  if (!source.trim()) return <p className="m-0 text-sm text-muted">Nothing to preview</p>;
  const blocks: ReactNode[] = [];
  const lines = source.replace(/\r\n?/g, "\n").split("\n");
  let inCode = false;
  let code: string[] = [];
  lines.forEach((line, index) => {
    if (line.startsWith("```")) {
      if (inCode) {
        blocks.push(
          <pre
            key={`code-${index}`}
            className="my-2 overflow-x-auto rounded-md bg-bg p-2 font-code text-xs"
          >
            <code>{code.join("\n")}</code>
          </pre>,
        );
        code = [];
      }
      inCode = !inCode;
      return;
    }
    if (inCode) {
      code.push(line);
      return;
    }
    const heading = line.match(/^(#{1,3})\s+(.+)$/);
    const task = line.match(/^\s*[-*]\s+\[([ xX])\]\s+(.+)$/);
    const bullet = line.match(/^\s*[-*]\s+(.+)$/);
    const ordered = line.match(/^\s*(\d+)\.\s+(.+)$/);

    if (heading)
      blocks.push(
        <div
          key={index}
          className={clsx("font-semibold", heading[1].length === 1 ? "text-lg" : "text-base")}
        >
          {inlineMarkdown(heading[2])}
        </div>,
      );
    else if (line.startsWith("> "))
      blocks.push(
        <blockquote key={index} className="my-1 border-l-2 border-muted pl-3 text-muted">
          {inlineMarkdown(line.slice(2))}
        </blockquote>,
      );
    else if (task)
      blocks.push(
        <div key={index} className="flex gap-2">
          <input type="checkbox" checked={task[1].toLowerCase() === "x"} readOnly />{" "}
          <span>{inlineMarkdown(task[2])}</span>
        </div>,
      );
    else if (bullet)
      blocks.push(
        <div key={index} className="pl-4 before:mr-2 before:content-['•']">
          {inlineMarkdown(bullet[1])}
        </div>,
      );
    else if (ordered)
      blocks.push(
        <div key={index} className="pl-4">
          <span className="mr-2 text-muted">{ordered[1]}.</span>
          {inlineMarkdown(ordered[2])}
        </div>,
      );
    else if (line === "") blocks.push(<div key={index} className="h-2" />);
    else
      blocks.push(
        <p key={index} className="m-0 min-h-5">
          {inlineMarkdown(line)}
        </p>,
      );
  });
  if (code.length)
    blocks.push(
      <pre key="code-tail" className="my-2 overflow-x-auto rounded-md bg-bg p-2 font-code text-xs">
        <code>{code.join("\n")}</code>
      </pre>,
    );
  return <div className="text-sm leading-relaxed wrap-break-word">{blocks}</div>;
}

function Composer({
  draft,
  onDraftChange,
  onSubmit,
  onSubmitAndDispatch,
  onCancel,
  placeholder,
  showCancel,
  dispatching,
}: {
  draft: string;
  onDraftChange: (v: string) => void;
  onSubmit: () => void;
  onSubmitAndDispatch: () => void;
  onCancel: () => void;
  placeholder: string;
  showCancel: boolean;
  dispatching: boolean;
}) {
  const [mode, setMode] = useState<"write" | "preview">("write");
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const editSelection = (before: string, after = "", fallback = "text", linePrefix = false) => {
    const textarea = textareaRef.current;
    if (!textarea) return;
    const start = textarea.selectionStart;
    const end = textarea.selectionEnd;
    const selected = draft.slice(start, end) || fallback;
    let replacement: string;
    if (linePrefix) {
      replacement = selected
        .split("\n")
        .map((line) => `${before}${line}`)
        .join("\n");
    } else {
      replacement = `${before}${selected}${after}`;
    }
    onDraftChange(`${draft.slice(0, start)}${replacement}${draft.slice(end)}`);
    requestAnimationFrame(() => {
      textarea.focus();
      textarea.setSelectionRange(start + before.length, start + replacement.length - after.length);
    });
  };
  const tools: [string, ReactNode, string, string, string, boolean][] = [
    ["Heading", <Heading size={14} />, "### ", "", "Heading", true],
    ["Bold", <Bold size={14} />, "**", "**", "bold text", false],
    ["Italic", <Italic size={14} />, "_", "_", "italic text", false],
    ["Quote", <Quote size={14} />, "> ", "", "quote", true],
    ["Code", <Code size={14} />, "`", "`", "code", false],
    ["Link", <Link size={14} />, "[", "](https://)", "link text", false],
    ["Bulleted list", <List size={14} />, "- ", "", "list item", true],
    ["Numbered list", <ListOrdered size={14} />, "1. ", "", "list item", true],
    ["Task list", <ListTodo size={14} />, "- [ ] ", "", "task", true],
  ];
  return (
    <div className="flex flex-col gap-2 px-3 py-2">
      <div className="overflow-hidden rounded-md border border-border bg-bg">
        <div className="flex items-center border-b border-border bg-panel/60">
          {(["write", "preview"] as const).map((tab) => (
            <button
              key={tab}
              onClick={() => setMode(tab)}
              className={clsx(
                "cursor-pointer border-0 border-r border-border bg-transparent px-3 py-2 text-sm capitalize",
                mode === tab ? "bg-bg text-text font-medium" : "text-muted",
              )}
            >
              {tab}
            </button>
          ))}
          {mode === "write" && (
            <div className="ml-auto flex items-center px-1">
              {tools.map(([title, label, before, after, fallback, linePrefix]) => (
                <button
                  key={title}
                  type="button"
                  title={title}
                  onClick={() => editSelection(before, after, fallback, linePrefix)}
                  className="grid h-8 min-w-8 cursor-pointer place-content-center border-0 bg-transparent px-2 text-muted hover:text-text"
                >
                  {label}
                </button>
              ))}
            </div>
          )}
        </div>
        {mode === "write" ? (
          <textarea
            ref={textareaRef}
            value={draft}
            onChange={(e) => onDraftChange(e.target.value)}
            placeholder={placeholder}
            rows={4}
            onKeyDown={(e) => {
              if ((e.metaKey || e.ctrlKey) && e.key === "Enter") onSubmit();
            }}
            className="block w-full resize-y border-0 bg-bg px-3 py-2 text-sm placeholder:text-muted outline-none"
          />
        ) : (
          <div className="min-h-24 px-3 py-2">
            <MarkdownPreview source={draft} />
          </div>
        )}
      </div>
      <div className="flex gap-1.5 justify-end">
        {showCancel && (
          <button
            className="text-xs bg-transparent border border-border rounded-md px-2.5 py-1 cursor-pointer text-muted hover:border-border"
            onClick={onCancel}
          >
            Cancel
          </button>
        )}
        <button
          className="text-xs bg-green/15 border border-green/50 rounded-md px-2.5 py-1 cursor-pointer text-green hover:bg-green/25 disabled:opacity-40"
          disabled={draft.trim() === ""}
          onClick={onSubmit}
        >
          Comment
        </button>
        <button
          className="text-xs bg-accent/15 border border-accent/50 rounded-md px-2.5 py-1 cursor-pointer text-accent hover:bg-accent/25 disabled:opacity-40"
          disabled={draft.trim() === "" || dispatching}
          onClick={onSubmitAndDispatch}
          title="Posts this comment, then asks your agent to act on it"
        >
          {dispatching ? "Agent busy…" : "Ask agent"}
        </button>
      </div>
    </div>
  );
}

export default function CommentThread({
  flags,
  dispatch,
  draft,
  onDraftChange,
  onSubmit,
  onSubmitAndDispatch,
  onResolve,
  onDispatch,
  onCancel,
  composerOnly,
}: Props) {
  // Dispatch itself is fire-and-forget. Keep the composer disabled between
  // the click and the daemon's first status event so it immediately reflects
  // the normal "Agent busy" flow and cannot submit the same prompt twice.
  const [dispatchStarting, setDispatchStarting] = useState(false);
  const running = dispatch?.status === "running" || (dispatchStarting && dispatch === null);
  const startDispatch = (request: () => void) => {
    setDispatchStarting(true);
    request();
  };
  const submitAndDispatch = () => {
    startDispatch(onSubmitAndDispatch);
  };

  if (flags.length === 0) {
    if (!composerOnly) return null;
    return (
      <div className="px-4 py-3 bg-panel/40 border-b border-border">
        <div className="rounded-md border border-border bg-panel">
          <Composer
            draft={draft}
            onDraftChange={onDraftChange}
            onSubmit={onSubmit}
            onSubmitAndDispatch={submitAndDispatch}
            onCancel={onCancel}
            placeholder="Leave a comment on this hunk…"
            showCancel
            dispatching={running}
          />
        </div>
      </div>
    );
  }

  return (
    <div className="px-4 py-3 bg-panel/40 border-b border-border flex flex-col gap-3">
      {flags.map((flag, fi) => {
        const resolved = !flag.open;
        const latestHumanIndex = flag.thread.reduce(
          (latest, entry, index) => (entry.kind === "human_comment" ? index : latest),
          -1,
        );
        const agentRespondedToLatestHuman = flag.thread
          .slice(latestHumanIndex + 1)
          .some((entry) => entry.kind === "agent_response" || entry.kind === "agent_claim");
        return (
          <div
            key={fi}
            className={clsx(
              "rounded-md border bg-panel overflow-hidden",
              resolved ? "border-border/60 opacity-70" : "border-border",
            )}
          >
            {resolved && (
              <div className="flex items-center gap-1.5 px-3 py-1.5 bg-green/5 border-b border-border text-xs text-green">
                <CircleCheck size={13} /> Resolved
              </div>
            )}
            {flag.thread.map((e, i) => (
              <Comment
                key={i}
                entry={e}
                onAskAgent={
                  !resolved && !running && i === latestHumanIndex && !agentRespondedToLatestHuman
                    ? () => startDispatch(() => onDispatch(e.body))
                    : undefined
                }
              />
            ))}

            {!resolved && (
              <div className="border-t border-border bg-panel/60">
                {flag.addressed_claim && (
                  <div className="px-3 py-1.5 text-[11px] text-accent border-b border-border">
                    Agent claims this is addressed — review the change, then resolve.
                  </div>
                )}
                <Composer
                  draft={draft}
                  onDraftChange={onDraftChange}
                  onSubmit={onSubmit}
                  onSubmitAndDispatch={submitAndDispatch}
                  onCancel={onCancel}
                  placeholder="Reply…"
                  showCancel={false}
                  dispatching={running}
                />
                <div className="flex items-center gap-1.5 px-3 py-2 border-t border-border">
                  <button
                    className="text-xs bg-transparent border border-border rounded-md px-2.5 py-1 cursor-pointer text-muted hover:border-green hover:text-green"
                    onClick={onResolve}
                    title="Resolving is always your call"
                  >
                    Resolve
                  </button>
                  {dispatch && dispatch.status !== "running" && (
                    <span
                      className={clsx(
                        "text-[11px] ml-auto truncate",
                        dispatch.status === "done" && "text-green",
                        dispatch.status === "scope_violation" && "text-warn",
                        (dispatch.status === "failed" ||
                          dispatch.status === "timed_out_reverted") &&
                          "text-highest",
                      )}
                      title={dispatch.detail ?? undefined}
                    >
                      {dispatch.status.replace(/_/g, " ")}
                    </span>
                  )}
                </div>
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
