import { createHighlighter, type Highlighter, type ThemedToken } from "shiki";
import { createJavaScriptRegexEngine } from "shiki/engine/javascript";
import htbTheme from "./htbShikiTheme.json";

let highlighterPromise: Promise<Highlighter> | null = null;

/**
 * No langs preloaded — each Shiki grammar is its own chunk (some, like
 * cpp/emacs-lisp, are 600-800KB) and eagerly loading all ~30 supported
 * langs on startup would defeat the point of a local-first tool. Languages
 * load on demand via ensureLang() as files with new extensions appear.
 */
export function getHighlighter(): Promise<Highlighter> {
  if (!highlighterPromise) {
    highlighterPromise = createHighlighter({
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      themes: [htbTheme as any],
      langs: ["text"],
      // Pure-JS regex engine, not the default WASM (oniguruma) one. The daemon
      // serves the SPA under a strict CSP (`script-src 'self'`, no
      // 'wasm-unsafe-eval'), which blocks WebAssembly.instantiate — the WASM
      // engine would reject createHighlighter and every diff would fall back to
      // plain monochrome text. The JS engine needs no WASM, so highlighting
      // works without loosening the CSP.
      engine: createJavaScriptRegexEngine(),
    });
  }
  return highlighterPromise;
}

/** Loads a language grammar if not already loaded. Idempotent, cheap to
 *  call repeatedly — Shiki no-ops if it's already loaded. */
export async function ensureLang(highlighter: Highlighter, lang: string): Promise<void> {
  if (lang === "text" || highlighter.getLoadedLanguages().includes(lang as never)) return;
  try {
    await highlighter.loadLanguage(lang as never);
  } catch {
    // Unknown/invalid lang id — falls back to "text" at tokenize time.
  }
}

const cache = new Map<string, ThemedToken[][]>();

/**
 * Cached by hunk id + side — hunk id is a content hash, so the same hunk
 * never needs re-tokenizing across regenerate/reorder/reconcile. Callers
 * must ensureLang() first; this falls back to plain "text" tokenizing
 * (still gets HTB theme colors for punctuation-less plain runs) if the
 * language isn't loaded.
 */
export function tokenizeSide(
  highlighter: Highlighter,
  hunkId: string,
  side: "old" | "new",
  code: string,
  lang: string,
): ThemedToken[][] {
  const key = `${hunkId}:${side}`;
  const cached = cache.get(key);
  if (cached) return cached;

  const safeLang = highlighter.getLoadedLanguages().includes(lang as never) ? lang : "text";
  const { tokens } = highlighter.codeToTokens(code, {
    lang: safeLang as never,
    theme: "hackthebox",
  });
  cache.set(key, tokens);
  return tokens;
}
