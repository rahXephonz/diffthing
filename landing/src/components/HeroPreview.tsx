import { motion, useReducedMotion } from "framer-motion";
import {
  PREVIEW,
  PREVIEW_BAR,
  DOT,
  PREVIEW_URL,
  PREVIEW_TAG,
  SHOT_IMG,
} from "../lib/ui";

/**
 * Hero product preview: the real diffthing review UI in a browser frame.
 * Framer Motion handles a calm entrance (rise + settle) and a subtle hover
 * lift. Class names match the ported global.css so the framing matches the
 * rest of the page.
 */
export default function HeroPreview() {
  const reduce = useReducedMotion();

  return (
    <motion.div
      className={PREVIEW}
      initial={{ opacity: 0, y: reduce ? 0 : 40 }}
      whileInView={{ opacity: 1, y: 0 }}
      viewport={{ once: true, amount: 0.2 }}
      transition={{ duration: 0.8, ease: [0.2, 0.7, 0.2, 1] }}
      whileHover={reduce ? undefined : { y: -6 }}
    >
      <div className={PREVIEW_BAR}>
        <span className={DOT}></span>
        <span className={DOT}></span>
        <span className={DOT}></span>
        <span className={PREVIEW_URL}>local.diffthing.dev</span>
        <span className={PREVIEW_TAG}>28 hunks</span>
      </div>
      <img
        src="/images/preview-diffthing.png"
        alt="Diffthing review UI: a split diff with an AI-organized walkthrough of scopes and steps in the sidebar."
        className={SHOT_IMG}
        loading="eager"
        width="1702"
        height="964"
      />
    </motion.div>
  );
}
