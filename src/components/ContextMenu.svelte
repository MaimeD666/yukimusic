<script lang="ts">
  import { onMount } from "svelte";
  import { softReveal } from "../lib/motion";
  import Icon from "./Icon.svelte";
  import type { Menu } from "../lib/menu";
  export let menu: Menu;
  export let onclose: () => void;
  export let onerror: (error: unknown) => void;
  let element: HTMLDivElement;
  let left = menu.x;
  let top = menu.y;
  const previous = document.activeElement as HTMLElement | null;
  function close(restore = false) {
    onclose();
    if (restore) previous?.focus({ preventScroll: true });
  }
  function buttons() {
    return [
      ...element.querySelectorAll<HTMLButtonElement>("button:not(:disabled)"),
    ];
  }
  function keydown(event: KeyboardEvent) {
    event.stopPropagation();
    const choices = buttons();
    const index = choices.indexOf(document.activeElement as HTMLButtonElement);
    if (["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) {
      event.preventDefault();
      const next =
        event.key === "Home"
          ? 0
          : event.key === "End"
            ? choices.length - 1
            : (index + (event.key === "ArrowDown" ? 1 : -1) + choices.length) %
              choices.length;
      choices[next]?.focus();
    } else if (event.key === "Escape" || event.key === "Tab") {
      event.preventDefault();
      close(true);
    }
  }
  onMount(() => {
    const rect = element.getBoundingClientRect();
    left = Math.max(8, Math.min(menu.x, innerWidth - rect.width - 8));
    top = Math.max(8, Math.min(menu.y, innerHeight - rect.height - 8));
    buttons()[0]?.focus({ preventScroll: true });
    const dismiss = (e: Event) => {
      if (!element.contains(e.target as Node)) close();
    };
    const resize = () => close();
    document.addEventListener("pointerdown", dismiss, true);
    document.addEventListener("scroll", dismiss, true);
    window.addEventListener("blur", resize);
    window.addEventListener("resize", resize);
    return () => {
      document.removeEventListener("pointerdown", dismiss, true);
      document.removeEventListener("scroll", dismiss, true);
      window.removeEventListener("blur", resize);
      window.removeEventListener("resize", resize);
    };
  });
</script>

<div
  class="app-context-menu"
  bind:this={element}
  role="menu"
  tabindex="-1"
  aria-label="Действия"
  style:left={`${left}px`}
  style:top={`${top}px`}
  onkeydown={keydown}
  transition:softReveal={{ duration: 150, y: 4, scale: 0.97 }}
>
  {#each menu.items as item}
    {#if item.separator}<div role="separator" class="menu-separator"></div>{/if}
    <button
      role="menuitem"
      disabled={item.disabled}
      onclick={() => {
        close(true);
        try {
          Promise.resolve(item.run()).catch(onerror);
        } catch (e) {
          onerror(e);
        }
      }}
      ><Icon name={item.icon || "arrow"} size={16} /><span>{item.label}</span
      ></button
    >
  {/each}
</div>

<style>
  .app-context-menu {
    position: fixed;
    z-index: 1000;
    width: 240px;
    max-width: calc(100vw - 16px);
    max-height: calc(100vh - 16px);
    overflow-y: auto;
    padding: 5px;
    border: 1px solid var(--line);
    border-radius: 14px;
    background: var(--raised);
    color: var(--text);
    box-shadow: 0 8px 28px #0005;
  }
  button {
    display: flex;
    gap: 12px;
    align-items: center;
    width: 100%;
    min-height: 35px;
    padding: 7px 10px;
    color: inherit;
    text-align: left;
    font-size: 12px;
    border-radius: 9px;
    line-height: 19px;
  }
  button:hover:not(:disabled),
  button:focus-visible {
    background: var(--hover);
    color: var(--text);
    outline: none;
  }
  button:disabled {
    opacity: 0.35;
  }
  .menu-separator {
    height: 1px;
    margin: 5px 7px;
    background: var(--line);
  }
</style>
