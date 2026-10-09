<script lang="ts">
  import { artworkSource } from "../lib/artwork";
  import Mark from "./Mark.svelte";
  export let src: string | null = null;
  export let title = "";
  export let large = false;
  let resolved = "";
  let request = 0;
  $: load(src);
  async function load(url: string | null) {
    const id = ++request;
    resolved = "";
    const result = await artworkSource(url);
    if (request === id) resolved = result;
  }
</script>

<div class:large class="artwork">
  {#if resolved}<img
      src={resolved}
      alt={title}
      loading={large ? "eager" : "lazy"}
      decoding="async"
      draggable="false"
      onerror={() => (resolved = "")}
    />
  {:else}<div class="artwork-empty" aria-label={title || "Обложка"}>
      <Mark />
    </div>{/if}
</div>
