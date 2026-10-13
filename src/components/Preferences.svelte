<script lang="ts">
  import Icon from "./Icon.svelte";
  import type { Theme } from "../lib/catalog";
  export let theme: Theme;
  export let visualizer: boolean;
  export let bass: boolean;
  export let ontheme: (theme: Theme) => void;
  export let onvisualizer: () => void;
  export let onbass: () => void;
  export let onequalizer: () => void;
  const shortcuts = [
    ["Поиск", "Ctrl K"],
    ["Пауза / воспроизведение", "Пробел"],
    ["Очередь", "Q"],
    ["Полноэкранный плеер", "F"],
    ["Мини-плеер", "M"],
    ["Назад / закрыть", "Esc"],
  ];
</script>

<div class="preferences-scroll">
  <section class="preference-section">
    <h2>Тема</h2>
    <div class="theme-choices" aria-label="Цветовая тема">
      {#each ["dark", "light"] as choice}
        <button
          class="theme-choice"
          class:selected={theme === choice}
          aria-pressed={theme === choice}
          onclick={() => ontheme(choice as Theme)}
        >
          <span class="theme-preview" class:light-preview={choice === "light"}
            ><i></i><span><b></b><b></b><b></b></span><em></em></span
          >
          <span class="theme-choice-label"
            ><Icon
              name={choice === "dark" ? "moon" : "sun"}
              size={17}
            />{choice === "dark" ? "Лунная" : "Облачная"}<span
              class="theme-radio"
              >{#if theme === choice}<Icon name="check" size={12} />{/if}</span
            ></span
          >
        </button>
      {/each}
    </div>
  </section>
  <section class="preference-section">
    <h2>Звук и движение</h2>
    <div class="setting-row">
      <span
        ><strong>Визуализация музыки</strong><small
          >Спектр звука в обычном и полноэкранном плеере</small
        ></span
      ><button
        class="switch"
        class:on={visualizer}
        role="switch"
        aria-checked={visualizer}
        aria-label="Визуализация музыки"
        onclick={onvisualizer}><span></span></button
      >
    </div>
    <div class="setting-row">
      <span
        ><strong>Отклик на бас</strong><small
          >Мягкая пульсация обложки в полном экране</small
        ></span
      ><button
        class="switch"
        class:on={bass}
        role="switch"
        aria-checked={bass}
        aria-label="Отклик на бас"
        onclick={onbass}><span></span></button
      >
    </div>
    <button class="setting-row setting-action" onclick={onequalizer}
      ><span
        ><strong>Эквалайзер</strong><small>10 полос и готовые пресеты</small
        ></span
      ><Icon name="chevron" size={18} /></button
    >
  </section>
  <section class="preference-section keyboard-section">
    <h2>Горячие клавиши</h2>
    <div class="shortcut-grid">
      {#each shortcuts as [label, key]}<div>
          <span>{label}</span><kbd>{key}</kbd>
        </div>{/each}
    </div>
  </section>
</div>
