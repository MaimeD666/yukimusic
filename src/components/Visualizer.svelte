<script lang="ts">
  import { onMount } from "svelte";
  import { native } from "../lib/bridge";
  export let active = false;
  export let large = false;
  export let enabled = true;
  export let color = "#e5dde5";
  export let trackKey: string | null = null;
  let canvas: HTMLCanvasElement;
  let values = new Float32Array(48);
  let drawn = new Float32Array(48);
  let frame = 0;
  let last = 0;
  let mounted = false;
  let visible = true;
  let reduce = false;
  let context: CanvasRenderingContext2D | null = null;
  let dpr = 1;
  let previousKey: string | null = null;
  $: if (trackKey !== previousKey) {
    previousKey = trackKey;
    values.fill(0);
    drawn.fill(0);
  }
  $: if (!active) values.fill(0);

  function render(now: number) {
    if (!mounted || !visible || !enabled) {
      frame = 0;
      return;
    }
    frame = requestAnimationFrame(render);
    if (reduce && now - last < 100) return;
    const elapsed = Math.min(100, now - last || 16.67);
    last = now;
    const ctx = context!;
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    ctx.fillStyle = color;
    const step = canvas.width / 48;
    let movement = false;
    for (let i = 0; i < 48; i++) {
      const target = active ? values[i] || 0 : 0;
      const response = target > drawn[i] ? 9 : 85;
      drawn[i] += (target - drawn[i]) * (1 - Math.exp(-elapsed / response));
      if (drawn[i] > 0.003) movement = true;
      const h = Math.max(dpr, drawn[i] * canvas.height * 0.93);
      ctx.globalAlpha = 0.3 + drawn[i] * 0.7;
      ctx.fillRect(
        i * step,
        (canvas.height - h) / 2,
        Math.max(dpr, step - 3 * dpr),
        h,
      );
    }
    if (!active && !movement) {
      cancelAnimationFrame(frame);
      frame = 0;
    }
  }
  function start() {
    if (mounted && enabled && visible && !frame)
      frame = requestAnimationFrame(render);
  }
  $: {
    active;
    enabled;
    start();
  }

  onMount(() => {
    mounted = true;
    context = canvas.getContext("2d");
    const resize = new ResizeObserver(([entry]) => {
      dpr = Math.min(window.devicePixelRatio, 2);
      canvas.width = Math.round(entry.contentRect.width * dpr);
      canvas.height = Math.round(entry.contentRect.height * dpr);
      start();
    });
    resize.observe(canvas);
    const motion = matchMedia("(prefers-reduced-motion: reduce)");
    reduce = motion.matches;
    let unlisten: (() => void) | undefined;
    if (native)
      import("@tauri-apps/api/event").then(async ({ listen }) => {
        const off = await listen<number[]>("spectrum", (e) => {
          values = Float32Array.from(e.payload);
          start();
        });
        if (!mounted) off();
        else unlisten = off;
      });
    const visibility = () => {
      visible = !document.hidden;
      if (!visible) {
        cancelAnimationFrame(frame);
        frame = 0;
      } else start();
    };
    document.addEventListener("visibilitychange", visibility);
    const observer = new IntersectionObserver(([entry]) => {
      visible = entry.isIntersecting && !document.hidden;
      if (visible) start();
      else {
        cancelAnimationFrame(frame);
        frame = 0;
      }
    });
    observer.observe(canvas);
    start();
    return () => {
      mounted = false;
      cancelAnimationFrame(frame);
      unlisten?.();
      observer.disconnect();
      resize.disconnect();
      document.removeEventListener("visibilitychange", visibility);
    };
  });
</script>

<canvas
  bind:this={canvas}
  class:large
  class="visualizer"
  aria-label="Спектр воспроизводимой музыки"
></canvas>
