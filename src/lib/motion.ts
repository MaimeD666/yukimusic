import { tick } from "svelte";

const easing = "cubic-bezier(.22, 1, .36, 1)";
// The same curve is used by CSS, Web Animations and Svelte transitions.
export function easeOut(t: number) {
  let low = 0, high = 1, u = t;
  for (let i = 0; i < 14; i++) {
    const x = 3 * (1 - u) ** 2 * u * 0.22 + 3 * (1 - u) * u ** 2 * 0.36 + u ** 3;
    if (Math.abs(x - t) < 0.00001) break;
    if (x < t) low = u; else high = u;
    u = (low + high) / 2;
  }
  return 1 - (1 - u) ** 3;
}
export const motionDuration = (duration = 240) =>
  matchMedia("(prefers-reduced-motion: reduce)").matches ? 0 : duration;

type Reveal = {
  key?: unknown;
  duration?: number;
  y?: number;
  scale?: number;
  initial?: boolean;
};

/** Animate a content change without replacing its DOM, focus or scroll position. */
export function reveal(node: HTMLElement | SVGElement, options: Reveal = {}) {
  let previous = options.key;
  let animation: Animation | undefined;
  let revision = 0;
  const preference = matchMedia("(prefers-reduced-motion: reduce)");
  const stop = () => animation?.cancel();
  async function play(next: Reveal) {
    const current = ++revision;
    stop();
    await tick();
    if (current !== revision || preference.matches || !node.isConnected) return;
    animation = node.animate(
      [
        { opacity: 0, transform: `translateY(${next.y ?? 7}px) scale(${next.scale ?? 1})` },
        { opacity: 1, transform: "translateY(0) scale(1)" },
      ],
      { duration: next.duration ?? 260, easing },
    );
  }
  if (options.initial !== false) void play(options);
  preference.addEventListener("change", stop);
  return {
    update(next: Reveal) {
      if (next.key !== previous) {
        previous = next.key;
        void play(next);
      }
    },
    destroy() {
      ++revision;
      stop();
      preference.removeEventListener("change", stop);
    },
  };
}

export function softReveal(node: Element, options: Reveal = {}) {
  const transform = getComputedStyle(node).transform;
  return {
    duration: motionDuration(options.duration ?? 220),
    easing: easeOut,
    css: (t: number) =>
      `opacity:${t};transform:${transform === "none" ? "" : transform} translateY(${(1 - t) * (options.y ?? 6)}px) scale(${1 - (1 - t) * (1 - (options.scale ?? 0.98))})`,
  };
}

/** Reserve the panel's space progressively, so the list does not jump sideways. */
export function panelSlide(node: HTMLElement) {
  const width = node.getBoundingClientRect().width;
  return {
    duration: motionDuration(280),
    easing: easeOut,
    css: (t: number) =>
      `width:${width * t}px;min-width:0;margin-left:${10 * t}px;opacity:${Math.min(1, t * 2)};border-width:${t}px;`,
  };
}

/** One moving highlight for a navigation or segmented control. */
export function selectionIndicator(node: HTMLElement, key: unknown) {
  let frame = 0;
  let disposed = false;
  let observed: Element | null = null;
  const resize = new ResizeObserver(schedule);
  resize.observe(node);
  async function schedule() {
    await tick();
    if (disposed) return;
    cancelAnimationFrame(frame);
    frame = requestAnimationFrame(() => {
      const selected = node.querySelector<HTMLElement>(
        'button[aria-current="page"], button[aria-pressed="true"]',
      );
      node.dataset.selectionActive = String(!!selected);
      if (!selected) return;
      if (observed !== selected) {
        if (observed) resize.unobserve(observed);
        resize.observe(selected);
        observed = selected;
      }
      node.style.setProperty("--selection-x", `${selected.offsetLeft}px`);
      node.style.setProperty("--selection-y", `${selected.offsetTop}px`);
      node.style.setProperty("--selection-width", `${selected.offsetWidth}px`);
      node.style.setProperty("--selection-height", `${selected.offsetHeight}px`);
    });
  }
  void schedule();
  return {
    update(next: unknown) {
      if (next !== key) { key = next; void schedule(); }
    },
    destroy() { disposed = true; cancelAnimationFrame(frame); resize.disconnect(); },
  };
}
