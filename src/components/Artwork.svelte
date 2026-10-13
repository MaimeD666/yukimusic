<script lang="ts">
  import { artworkSource } from "../lib/artwork";
  import { fade } from "svelte/transition";
  import { motionDuration } from "../lib/motion";
  import Mark from "./Mark.svelte";
  export let src: string | null = null;
  export let title = "";
  export let large = false;
  let resolved = "";
  let request = 0;
  $: load(src);
  async function load(url: string | null) {
    const id = ++request;
    const result = await artworkSource(url);
    if (request !== id) return;
    if (result) {
      const image = new Image();
      image.src = result;
      try { await image.decode(); } catch { /* The image element handles the fallback. */ }
    }
    if (request === id) resolved = result;
  }
</script>

<div class:large class="artwork">
  {#if resolved}{#key resolved}<img
      src={resolved}
      alt={title}
      loading={large ? "eager" : "lazy"}
      decoding="async"
      draggable="false"
      in:fade={{ duration: motionDuration(240) }}
      out:fade={{ duration: motionDuration(160) }}
      onerror={() => (resolved = "")}
    />{/key}
  {:else}<div class="artwork-empty" aria-label={title || "Обложка"}>
      <Mark />
    </div>{/if}
</div>
