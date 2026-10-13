<script lang="ts">
  import { onMount, tick } from "svelte";
  import { softReveal } from "../lib/motion";
  let text = "", left = 0, top = 0;
  let element: HTMLDivElement;
  onMount(() => {
    let timer: ReturnType<typeof setTimeout>;
    let target: HTMLElement | null = null;
    let description: string | null = null;
    let revision = 0;
    const hide = () => {
      ++revision;
      clearTimeout(timer);
      if (target) {
        if (description) target.setAttribute("aria-describedby", description);
        else target.removeAttribute("aria-describedby");
      }
      target = null;
      text = "";
    };
    const enter = (event: Event) => {
      const candidate = (event.target as Element)?.closest<HTMLElement>(
        ".icon-button, .play-button, .card-play, .track-number, .with-panel .primary-nav button",
      );
      if (!candidate || candidate.matches(":disabled") || !candidate.getAttribute("aria-label")) return;
      if (event.type === "focusin" && !candidate.matches(":focus-visible")) return;
      if (candidate === target) return;
      hide();
      target = candidate;
      description = target.getAttribute("aria-describedby");
      const current = revision;
      timer = setTimeout(async () => {
        if (current !== revision || !candidate.isConnected) return;
        const rect = candidate.getBoundingClientRect();
        text = candidate.dataset.tooltip || candidate.getAttribute("aria-label") || "";
        left = rect.left + rect.width / 2;
        top = rect.top - 8;
        candidate.setAttribute("aria-describedby", "app-tooltip");
        await tick();
        if (current !== revision || !element) return;
        left = Math.max(8, Math.min(innerWidth - element.offsetWidth - 8, left - element.offsetWidth / 2));
        top = rect.top >= element.offsetHeight + 16 ? rect.top - element.offsetHeight - 8 : rect.bottom + 8;
      }, event.type === "focusin" ? 400 : 700);
    };
    const leave = (event: Event) => {
      if (!target?.contains((event as MouseEvent).relatedTarget as Node | null)) hide();
    };
    document.addEventListener("pointerover", enter);
    document.addEventListener("focusin", enter);
    document.addEventListener("pointerout", leave);
    document.addEventListener("focusout", leave);
    for (const event of ["pointerdown", "keydown", "scroll"]) document.addEventListener(event, hide, true);
    window.addEventListener("blur", hide);
    return () => {
      hide();
      document.removeEventListener("pointerover", enter);
      document.removeEventListener("focusin", enter);
      document.removeEventListener("pointerout", leave);
      document.removeEventListener("focusout", leave);
      for (const event of ["pointerdown", "keydown", "scroll"]) document.removeEventListener(event, hide, true);
      window.removeEventListener("blur", hide);
    };
  });
</script>

{#if text}<div id="app-tooltip" class="app-tooltip" bind:this={element} role="tooltip" style:left={`${left}px`} style:top={`${top}px`} transition:softReveal={{ duration: 140, y: 0, scale: 1 }}>{text}</div>{/if}
