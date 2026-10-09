import { native } from "./bridge";
import { bassEnergy, BassEnvelope } from "./bass";

type Options = {
  enabled: boolean;
  active: boolean;
  trackKey: string | null;
  reduced: boolean;
};
export function bassReaction(node: HTMLElement, initial: Options) {
  let options = initial;
  let disposed = false;
  let unlisten: (() => void) | undefined;
  let subscribing = false;
  let frame = 0;
  let last = 0;
  let received = 0;
  let target = 0;
  const envelope = new BassEnvelope();
  let art: HTMLElement | null = null;
  let glow: HTMLElement | null = null;
  const running = () =>
    options.enabled && options.active && !options.reduced && !document.hidden;
  function reset() {
    cancelAnimationFrame(frame);
    frame = 0;
    target = envelope.level = 0;
    art?.style.removeProperty("transform");
    art?.style.removeProperty("will-change");
    glow?.style.removeProperty("opacity");
    glow?.style.removeProperty("transform");
  }
  function draw(now: number) {
    frame = 0;
    if (!options.enabled || options.reduced || document.hidden) {
      reset();
      return;
    }
    const level = envelope.update(
      running() && now - received < 150 ? target : 0,
      Math.min(50, now - last || 16.67),
    );
    last = now;
    if (art) art.style.transform = `scale(${1 + level * 0.048})`;
    if (glow) {
      glow.style.opacity = String(level * 0.72);
      glow.style.transform = `translate(-50%, -50%) scale(${0.9 + level * 0.18})`;
    }
    if (level > 0 || running()) frame = requestAnimationFrame(draw);
    else reset();
  }
  function start() {
    if (running() && !frame) {
      art = node.querySelector(".player-art");
      glow = node.querySelector(".bass-glow");
      if (art) art.style.willChange = "transform";
      last = performance.now();
      frame = requestAnimationFrame(draw);
    }
  }
  async function sync() {
    if (!running()) {
      unlisten?.();
      unlisten = undefined;
      target = 0;
      if (!options.enabled || options.reduced || document.hidden) reset();
      return;
    }
    start();
    if (!native || unlisten || subscribing) return;
    subscribing = true;
    const { listen } = await import("@tauri-apps/api/event");
    const off = await listen<number[]>("spectrum", ({ payload }) => {
      target = bassEnergy(payload);
      received = performance.now();
    });
    subscribing = false;
    if (disposed || !running()) off();
    else unlisten = off;
  }
  const visibility = () => void sync();
  document.addEventListener("visibilitychange", visibility);
  void sync();
  return {
    update(next: Options) {
      if (next.trackKey !== options.trackKey) reset();
      options = next;
      void sync();
    },
    destroy() {
      disposed = true;
      reset();
      unlisten?.();
      document.removeEventListener("visibilitychange", visibility);
    },
  };
}
