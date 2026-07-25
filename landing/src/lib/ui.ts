/* Shared Tailwind class strings for the landing page.
   These replace the old @layer components block in global.css: the styling is
   utility-first ("className style"), single-sourced here so repeated UI (mock
   frames, scope cards, feature rows, timeline items, …) stays DRY.

   Bare marker classes still live in the markup where JS/GSAP hooks need them:
   `reveal` (scroll batch), `pipe` (ScrollTrigger), `tab` (click handler);
   `data-stage` + the JS-toggled `.in` / `.lit` / `.scrolled` classes are
   targeted here via arbitrary variants. */

// scroll-reveal hook: marker class `reveal` + the transition; `.in` (added by GSAP) reveals.
export const REVEAL =
  "reveal opacity-0 translate-y-[18px] transition-[opacity,transform] duration-700 ease-[cubic-bezier(0.2,0.7,0.2,1)] [&.in]:opacity-100 [&.in]:translate-y-0";

export const EYEBROW =
  "inline-flex items-center gap-2 font-mono text-xs uppercase tracking-[0.18em] text-muted before:content-[''] before:h-1.5 before:w-1.5 before:rounded-[1px] before:bg-acid before:shadow-[0_0_10px_var(--color-acid)]";
export const SECTION_TITLE = "mt-[18px] mb-0 text-[clamp(2rem,5vw,3.4rem)]";
export const LEAD = "max-w-[62ch] text-[clamp(1rem,1.6vw,1.2rem)] text-muted";

const BTN_BASE =
  "inline-flex cursor-pointer items-center gap-[9px] whitespace-nowrap rounded-lg border border-line2 px-4 py-2 font-sans text-[14px] tracking-[0.02em] transition-[transform,border-color,background,box-shadow] duration-200 hover:-translate-y-px";
export const BTN_PRIMARY = `${BTN_BASE} border-acid bg-acid font-semibold text-[#0a0a0b] hover:shadow-[0_8px_30px_-8px_var(--color-acid)]`;
export const BTN_GHOST = `${BTN_BASE} border-line2 bg-transparent text-text hover:border-[var(--acid-line)] hover:bg-[var(--acid-dim)]`;
export const HERO_CTA = "mt-9 flex flex-wrap justify-center gap-3.5";

// preview / screenshot frame
export const PREVIEW =
  "preview mt-[clamp(48px,8vw,84px)] overflow-hidden rounded-[14px] border border-line text-left bg-[linear-gradient(180deg,var(--color-panel),var(--color-bg2))] shadow-[0_40px_120px_-50px_rgba(0,0,0,0.9)]";
export const PREVIEW_BAR =
  "flex items-center gap-2 border-b border-line px-4 py-3 font-mono text-xs text-faint";
export const DOT = "h-2.5 w-2.5 rounded-full bg-line2";
export const PREVIEW_URL = "ml-3 text-muted";
export const PREVIEW_TAG =
  "ml-auto rounded-full border border-[var(--acid-line)] px-2 py-0.5 text-[11px] text-acid";
export const SHOT_IMG = "shot-zoom block h-auto w-full cursor-zoom-in";

// mock frame
export const MOCK =
  "overflow-hidden rounded-[12px] border border-line bg-panel transition-[transform,border-color] duration-300 will-change-transform hover:-translate-y-1 hover:border-line2 shadow-[0_30px_80px_-50px_rgba(0,0,0,0.9)]";
export const MOCK_BAR =
  "flex items-center gap-[7px] border-b border-line px-[13px] py-2.5";
export const MOCK_DOT = "h-[9px] w-[9px] rounded-full bg-line2";
export const MOCK_T = "ml-2 font-mono text-[11px] text-faint";
export const MOCK_BODY = "p-4 font-mono text-xs";

// scope card
export const SCOPE =
  "mb-2.5 rounded-[9px] border border-line bg-panel2 px-[13px] py-3 transition-[border-color,transform] duration-200 hover:translate-x-0.5 hover:border-[var(--acid-line)]";
export const S_EYEBROW =
  "font-mono text-[10px] uppercase tracking-[0.16em] text-faint";
export const S_TITLE =
  "my-1 mb-2 flex items-center gap-2 text-sm font-semibold";
export const S_FILES = "flex flex-col gap-[5px]";
export const S_FILE = "flex items-center gap-2 font-mono text-xs text-muted";
export const S_FILE_IC = "h-[13px] w-[13px] shrink-0 rounded-[3px] bg-line2";
export const CNT = "ml-auto text-[11px]";

// inline atoms
export const CHIP =
  "rounded-full border border-line2 px-[7px] py-0.5 font-mono text-[10px] text-muted";
export const CHIP_HI = `${CHIP} border-[rgba(255,107,107,0.4)] text-del`;
export const NUM = "font-mono text-xs text-acid-ink";
export const ADD = "text-add";
export const DEL = "text-del";
export const ACID = "text-acid-ink";

// walkthrough feature row
export const FEATURE =
  "feature grid grid-cols-2 items-center gap-[clamp(28px,6vw,72px)] border-t border-line py-[clamp(48px,8vw,90px)] max-[900px]:grid-cols-1 max-[900px]:gap-7";
export const F_TEXT_FLIP = "order-2 max-[900px]:order-none";
export const F_H3 = "mb-3.5 text-[clamp(1.5rem,3vw,2.1rem)]";
export const F_P = "mx-0 mb-[18px] mt-0 max-w-[46ch] text-muted";
export const F_EYEBROW = `${EYEBROW} mb-4`;
export const F_POINTS = "m-0 flex list-none flex-col gap-2.5 p-0";
export const F_LI = "flex items-start gap-2.5 text-[0.95rem] text-text";
export const F_LI_SVG = "mt-[3px] shrink-0 text-acid-ink";

// review-focus + approval mock bodies
export const FOCUS_ROW = "mb-3 leading-relaxed text-muted";
export const FOCUS_LABEL =
  "mb-1.5 block text-[10px] uppercase tracking-[0.14em] text-faint";
export const PROGRESS = "mt-1 h-1.5 overflow-hidden rounded-full bg-line";
export const PROGRESS_BAR = "block h-full w-[66%] rounded-full bg-acid";
export const APPR_FILE =
  "flex items-center gap-[9px] border-b border-line px-1 py-2 text-muted";
export const TICK = "text-acid-ink";
export const TICK_PENDING = "text-faint";
export const BIGBTN =
  "mt-3.5 w-full rounded-lg border border-[var(--acid-line)] bg-[var(--acid-dim)] py-[11px] text-center font-mono text-xs text-acid-ink";

// diff mock
export const DIFF_SPLIT = "grid grid-cols-2";
const DIFF_LINE = "flex gap-3 whitespace-pre py-px";
export const DIFF_CTX = DIFF_LINE;
export const DIFF_LN = "w-6 select-none text-right text-faint";
export const DIFF_ADD = `${DIFF_LINE} bg-[var(--add-bg)] text-add`;
export const DIFF_DEL = `${DIFF_LINE} bg-[var(--del-bg)] text-del`;
export const GAP_HATCH =
  "min-h-[18px] rounded-[2px] bg-[repeating-linear-gradient(-45deg,transparent,transparent_4px,rgba(125,125,133,0.28)_4px,rgba(125,125,133,0.28)_6px)]";

// pipeline bento tile (marker: none; GSAP adds `.lit`, targeted via group)
export const STAGE =
  "group relative flex min-h-[180px] flex-col gap-3 rounded-[16px] border border-line bg-panel p-6 transition-[border-color,transform] duration-300 hover:-translate-y-1 hover:border-line2";
export const SPAN2 = "col-span-2 max-[820px]:col-span-1";
export const IDX = "font-mono text-[11px] text-faint";
export const ICO =
  "grid h-[38px] w-[38px] place-items-center rounded-[10px] border border-line2 text-acid-ink transition-[border-color,box-shadow] duration-300 group-[.lit]:border-[var(--acid-line)] group-[.lit]:shadow-[0_0_24px_-6px_var(--color-acid)]";
export const STAGE_H3 = "mt-auto text-[20px]";
export const STAGE_P = "m-0 text-[0.85rem] leading-[1.5] text-muted";

// security card
export const SEC_CARD =
  "bg-bg px-[26px] py-[30px] transition-[background] duration-200 hover:bg-panel";
export const SEC_ICO =
  "mb-4 grid h-9 w-9 place-items-center rounded-[9px] border border-line2 text-acid";
export const SEC_H3 = "mb-2 text-[1.05rem]";
export const SEC_P = "m-0 text-[0.9rem] text-muted";

// comparison
const CMP_GRID = "grid grid-cols-[1.4fr_1fr_1fr] max-[720px]:grid-cols-1";
export const CMP_HEAD = CMP_GRID;
export const CMP_ROW = CMP_GRID;
const CMP_HEAD_CELL =
  "border-b border-line px-[22px] py-[18px] font-mono text-xs uppercase tracking-[0.08em] text-muted";
export const CMP_CRIT_HEAD = CMP_HEAD_CELL;
export const CMP_TRAD_HEAD = CMP_HEAD_CELL;
export const CMP_YOU_HEAD = `${CMP_HEAD_CELL} bg-[var(--acid-dim)] text-acid-ink`;
const CMP_CELL =
  "flex items-center gap-2 border-b border-line px-[22px] py-4 text-[0.92rem]";
export const CMP_CRIT = `${CMP_CELL} font-mono text-xs text-text max-[720px]:bg-panel`;
export const CMP_TRAD = `${CMP_CELL} text-muted`;
export const CMP_YOU = `${CMP_CELL} bg-[rgba(181,255,61,0.04)] text-text`;
export const CMP_LAST = "border-b-0"; // append to last row's cells
export const CMP_YOU_SVG = "shrink-0 text-acid-ink";

// install terminal + CLI chips
export const TAB =
  "tab cursor-pointer rounded-[7px] border border-line px-[13px] py-2 font-sans text-[14px] text-muted transition-[color,border-color,background] duration-150 aria-selected:border-[var(--acid-line)] aria-selected:bg-[var(--acid-dim)] aria-selected:text-acid";
export const TERM_BODY =
  "min-h-[132px] p-5 font-mono text-[13px] leading-[1.9]";
export const TERM_CMT = "text-faint";
export const TERM_PR = "text-acid";
export const TERM_CMD = "text-text";
export const TERM_OUT = "text-muted";
export const COPY =
  "ml-auto cursor-pointer rounded-md border border-line px-[9px] py-[3px] font-sans text-[11px] text-faint hover:border-[var(--acid-line)] hover:text-acid";
export const CLI =
  "flex items-center justify-between gap-2 rounded-lg border border-line px-[13px] py-3 font-sans text-[14px] transition-[border-color,transform] duration-200 hover:-translate-y-0.5 hover:border-[var(--acid-line)]";

// roadmap timeline
export const TIMELINE = "flex flex-col gap-5 border-l border-line pl-[22px]";
const TL_BASE =
  "relative before:content-[''] before:absolute before:-left-[27px] before:top-[5px] before:h-[9px] before:w-[9px] before:rounded-full";
export const TL = `${TL_BASE} before:border before:border-line2 before:bg-bg`;
export const TL_DONE = `${TL_BASE} before:border before:border-acid before:bg-acid before:shadow-[0_0_12px_-2px_var(--color-acid)]`;
export const TL_H4 =
  "mb-[3px] mt-0 text-[0.98rem] font-semibold tracking-[-0.01em]";
export const TL_P = "m-0 text-[0.86rem] text-muted";
export const TL_VER = "font-mono text-[11px] text-faint";
export const ROAD_COL_H3 =
  "mb-5 flex items-center gap-[9px] font-mono text-xs uppercase tracking-[0.12em] text-muted";
