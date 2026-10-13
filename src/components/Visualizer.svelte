<script lang="ts">
  import { onMount } from "svelte";
  import { native } from "../lib/bridge";
  export let active = false;
  export let large = false;
  export let enabled = true;
  export let color = "var(--accent)";
  export let trackKey: string | null = null;
  let canvas: HTMLCanvasElement;
  const values = new Float32Array(48);
  const drawn = new Float32Array(48);
  let frame = 0;
  let last = 0;
  let lastSignal = 0;
  let mounted = false;
  let intersecting = true;
  let reduce = false;
  let context: CanvasRenderingContext2D | null = null;
  let width = 0,
    height = 0;
  let previousKey: string | null = null;
  $: if (trackKey !== previousKey) {
    previousKey = trackKey;
    values.fill(0);
    drawn.fill(0);
    start();
  }
  $: {
    active;
    enabled;
    if (!active) values.fill(0);
    start();
  }

  function curve(ctx: CanvasRenderingContext2D, points: [number, number][]) {
    ctx.moveTo(...points[0]);
    for (let i = 1; i < points.length; i++) {
      const previous = points[i - 1];
      const current = points[i];
      const middle = (previous[0] + current[0]) / 2;
      // Pass through every frequency band; rounded joins must not erase peaks.
      ctx.bezierCurveTo(middle, previous[1], middle, current[1], ...current);
    }
  }

  function render(now: number) {
    frame = 0;
    if (!mounted || !context || !intersecting || document.hidden) return;
    const ctx = context;
    ctx.clearRect(0, 0, width, height);
    if (!width || !height) return;
    const elapsed = Math.min(64, Math.max(1, now - last || 16.67));
    last = now;
    let movement = false;
    for (let i = 0; i < drawn.length; i++) {
      const target =
        active && enabled && !reduce && now - lastSignal < 250
          ? values[i] ** 1.6
          : 0;
      drawn[i] +=
        (target - drawn[i]) *
        (1 - Math.exp(-elapsed / (target > drawn[i] ? 24 : 150)));
      if (drawn[i] < 0.002) drawn[i] = 0;
      if (drawn[i] > 0) movement = true;
    }
    const ink = getComputedStyle(canvas).color;
    ctx.strokeStyle = ink;
    ctx.lineWidth = large ? 1.5 : 1;
    ctx.lineCap = "round";
    const baseline = height - 1;
    const top: [number, number][] = [];
    for (let i = 0; i < drawn.length; i++) {
      const x = (i / (drawn.length - 1)) * width;
      top.push([x, baseline - drawn[i] * (height - 3)]);
    }
    if (movement) {
      ctx.beginPath();
      curve(ctx, top);
      ctx.lineTo(width, baseline);
      ctx.lineTo(0, baseline);
      ctx.closePath();
      const fill = ctx.createLinearGradient(0, 0, 0, height);
      fill.addColorStop(0, ink);
      fill.addColorStop(1, "transparent");
      ctx.fillStyle = fill;
      ctx.globalAlpha = large ? 0.3 : 0.38;
      ctx.fill();
    }
    ctx.beginPath();
    curve(ctx, top);
    ctx.globalAlpha = movement ? 0.85 : 0.18;
    ctx.stroke();
    if (movement) {
      frame = requestAnimationFrame(render);
    }
    ctx.globalAlpha = 1;
  }
  function start() {
    if (mounted && !frame && intersecting && !document.hidden)
      frame = requestAnimationFrame(render);
  }

  onMount(() => {
    mounted = true;
    context = canvas.getContext("2d");
    const resize = new ResizeObserver(([entry]) => {
      const dpr = Math.min(window.devicePixelRatio, 2);
      width = entry.contentRect.width;
      height = entry.contentRect.height;
      canvas.width = Math.max(1, Math.round(width * dpr));
      canvas.height = Math.max(1, Math.round(height * dpr));
      context?.setTransform(dpr, 0, 0, dpr, 0, 0);
      start();
    });
    resize.observe(canvas);
    const preference = matchMedia("(prefers-reduced-motion: reduce)");
    const updateMotion = () => {
      reduce = preference.matches;
      if (reduce) drawn.fill(0);
      start();
    };
    updateMotion();
    preference.addEventListener("change", updateMotion);
    const theme = new MutationObserver(start);
    theme.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["data-theme"],
    });
    const player = canvas.closest(".player");
    if (player)
      theme.observe(player, { attributes: true, attributeFilter: ["style"] });
    const paletteChanged = (event: Event) => {
      if (
        ["--accent", "--player-secondary"].includes(
          (event as TransitionEvent).propertyName,
        )
      )
        start();
    };
    document.documentElement.addEventListener("transitionend", paletteChanged);
    let unlisten: (() => void) | undefined;
    if (native)
      import("@tauri-apps/api/event").then(async ({ listen }) => {
        const off = await listen<number[]>("spectrum", (event) => {
          if (!active || !enabled || reduce) return;
          for (let i = 0; i < values.length; i++)
            values[i] = Math.max(0, Math.min(1, Number(event.payload[i]) || 0));
          lastSignal = performance.now();
          start();
        });
        if (!mounted) off();
        else unlisten = off;
      });
    const visibility = () => {
      if (document.hidden || !intersecting) {
        cancelAnimationFrame(frame);
        frame = 0;
      } else start();
    };
    document.addEventListener("visibilitychange", visibility);
    const observer = new IntersectionObserver(([entry]) => {
      intersecting = entry.isIntersecting;
      visibility();
    });
    observer.observe(canvas);
    start();
    return () => {
      mounted = false;
      cancelAnimationFrame(frame);
      unlisten?.();
      observer.disconnect();
      resize.disconnect();
      theme.disconnect();
      document.removeEventListener("visibilitychange", visibility);
      preference.removeEventListener("change", updateMotion);
      document.documentElement.removeEventListener(
        "transitionend",
        paletteChanged,
      );
    };
  });
</script>

<canvas
  bind:this={canvas}
  class:large
  class="visualizer"
  style:color
  aria-label="Визуализация звука"
></canvas>
