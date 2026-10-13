<script lang="ts">
  import { onMount } from "svelte";
  import { softReveal } from "../lib/motion";
  export let name: string;
  export let kind: "artist" | "playlist";
  export let oncancel: () => void;
  export let onconfirm: () => void;
  let dialog: HTMLDialogElement;
  let cancel: HTMLButtonElement;
  onMount(() => {
    const previous = document.activeElement as HTMLElement | null;
    dialog.showModal();
    cancel.focus();
    return () => { dialog.close(); if (previous?.isConnected) previous.focus({ preventScroll: true }); };
  });
</script>

<dialog bind:this={dialog} class="confirm-removal" role="alertdialog" aria-labelledby="removal-title" aria-describedby="removal-description"
  oncancel={(event) => { event.preventDefault(); oncancel(); }}
  onkeydown={(event) => event.stopPropagation()}
  transition:softReveal={{ duration: 220, y: 6, scale: 1 }}>
  <h2 id="removal-title">Убрать {kind === "artist" ? "артиста" : "плейлист"}?</h2>
  <p id="removal-description"><strong>{name}</strong> будет удалён из коллекции. Любимые треки и очередь сохранятся.</p>
  <div class="confirm-actions">
    <button class="secondary-button" bind:this={cancel} onclick={oncancel}>Отмена</button>
    <button class="primary-button remove-button" onclick={onconfirm}>Убрать из коллекции</button>
  </div>
</dialog>
