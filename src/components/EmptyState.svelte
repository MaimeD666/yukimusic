<script lang="ts">
  import Icon from "./Icon.svelte";
  import { reveal } from "../lib/motion";
  export let icon = "search";
  export let title: string;
  export let description: string;
  export let action = "";
  export let onaction: () => void = () => {};
</script>

<div class="empty-state" use:reveal={{ key: title }}>
  <span class="empty-symbol"><Icon name={icon} size={29} /></span>
  <h2>{title}</h2>
  <p>{description}</p>
  {#if action}<button class="primary-button" onclick={onaction}
      >{action}<Icon name="arrow" size={16} /></button
    >{/if}
</div>
