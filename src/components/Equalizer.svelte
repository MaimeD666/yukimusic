<script lang="ts">
  import Icon from "./Icon.svelte";
  import { frequencies } from "../lib/types";
  export let bands: number[];
  export let enabled: boolean;
  export let onchange: (bands: number[], enabled: boolean) => void;
  export let onclose: () => void;
  const presets: Record<string, number[]> = {
    Ровно: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    Низкие: [5, 4, 3, 1, 0, 0, 0, 0, 0, 0],
    Воздух: [0, 0, -1, -1, 0, 1, 2, 3, 4, 3],
    Вокал: [-2, -1, 0, 1, 3, 3, 2, 0, -1, -1],
  };
  function update(index: number, value: number) {
    const next = [...bands];
    next[index] = value;
    onchange(next, true);
  }
</script>

<section class="eq-panel" aria-label="Эквалайзер">
  <div class="panel-heading">
    <div>
      <span class="eyebrow">НАСТРОЙКА ЗВУКА</span>
      <h2>Эквалайзер<span class="heading-star">✦</span></h2>
    </div>
    <button
      class="icon-button"
      aria-label="Закрыть эквалайзер"
      onclick={onclose}><Icon name="close" /></button
    >
  </div>
  <div class="eq-presets">
    {#each Object.entries(presets) as [name, preset]}<button
        class:chosen={JSON.stringify(preset) === JSON.stringify(bands)}
        onclick={() => onchange([...preset], true)}>{name}</button
      >{/each}<button
      class="eq-switch"
      class:chosen={enabled}
      aria-pressed={enabled}
      onclick={() => onchange(bands, !enabled)}
      >{enabled ? "Включён" : "Выключен"}</button
    >
  </div>
  <div class="eq-bands">
    {#each frequencies as frequency, i}<label
        ><span class="eq-gain">{bands[i] > 0 ? "+" : ""}{bands[i]}</span><input
          type="range"
          min="-12"
          max="12"
          step="1"
          value={bands[i]}
          aria-label={`${frequency} Гц`}
          oninput={(e) => update(i, +e.currentTarget.value)}
          ondblclick={() => update(i, 0)}
        /><span>{frequency >= 1000 ? `${frequency / 1000}k` : frequency}</span
        ></label
      >{/each}
  </div>
  <p class="panel-note">
    Двойной клик — сброс полосы. Запас громкости компенсируется автоматически.
  </p>
</section>
