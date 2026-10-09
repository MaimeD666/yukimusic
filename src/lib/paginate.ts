import { tick } from "svelte";

type Options = {
  enabled: boolean;
  load: () => Promise<void>;
  horizontal?: boolean;
};

/** A single bounded request when the end approaches the scroll viewport. */
export function paginate(node: HTMLElement, options: Options) {
  const root = node.parentElement!;
  const distance = options.horizontal ? 280 : 448;
  let disposed = false;
  let running = false;
  let scheduled = false;
  async function check() {
    if (scheduled || disposed) return;
    scheduled = true;
    await tick();
    scheduled = false;
    if (disposed || running || !options.enabled || document.hidden) return;
    const end = node.getBoundingClientRect();
    const viewport = root.getBoundingClientRect();
    if (viewport.width === 0 || viewport.height === 0) return;
    const near = options.horizontal
      ? end.left <= viewport.right + distance
      : end.top <= viewport.bottom + distance;
    if (!near) return;
    running = true;
    try {
      await options.load();
    } finally {
      running = false;
      void check();
    }
  }
  const observer = new IntersectionObserver(
    () => {
      void check();
    },
    {
      root,
      rootMargin: options.horizontal
        ? `0px ${distance}px`
        : `${distance}px 0px`,
    },
  );
  observer.observe(node);
  const visibility = () => {
    if (!document.hidden) void check();
  };
  document.addEventListener("visibilitychange", visibility);
  return {
    update(next: Options) {
      options = next;
      void check();
    },
    destroy() {
      disposed = true;
      observer.disconnect();
      document.removeEventListener("visibilitychange", visibility);
    },
  };
}
