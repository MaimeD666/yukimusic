<script lang="ts">
  import { reveal } from "../lib/motion";
  import { bassReaction } from "../lib/bassReaction";
  import {
    neutralPalette,
    paletteForTheme,
    type PlayerPalette,
  } from "../lib/palette";
  import Artwork from "./Artwork.svelte";
  import Icon from "./Icon.svelte";
  import Visualizer from "./Visualizer.svelte";
  import { time } from "../lib/format";
  import type { Playback, Track } from "../lib/types";
  export let state: Playback;
  export let mode: "dock" | "immersive" | "mini" = "dock";
  export let palette: PlayerPalette = neutralPalette;
  export let theme: "dark" | "light" = "dark";
  $: colors = mode === "dock" ? paletteForTheme(palette, theme) : palette;
  export let motion = 180;
  export let visualizer = true;
  export let bass = false;
  export let onbass: () => void;
  export let liked = false;
  export let panel: string | null = null;
  export let onaction: (action: string, value?: unknown) => void;
  export let onmode: (mode: "dock" | "immersive" | "mini") => void;
  export let onpanel: (panel: string) => void;
  export let onlike: (track: Track) => void;
  export let onvisualizer: () => void;
  export let onartist: (track: Track) => void;
  $: track = state.queue.find((e) => e.key === state.currentKey)?.track;
  $: duration = track?.duration || 0;
  let seek: number | null = null;
  let dragging = false;
  let pendingSeek = false;
  let seekKey: string | null = null;
  let seekSnapshot: Playback | null = null;
  $: if (seekKey !== state.currentKey) {
    seekKey = state.currentKey;
    seek = null;
    pendingSeek = false;
    dragging = false;
  }
  $: if (
    pendingSeek &&
    !dragging &&
    state !== seekSnapshot &&
    seek !== null &&
    (Math.abs(state.position - seek) < 0.05 ||
      (seek >= duration && state.position === 0) ||
      state.error)
  ) {
    pendingSeek = false;
    seek = null;
  }
  function commitSeek(event: Event) {
    seek = +(event.target as HTMLInputElement).value;
    seekSnapshot = state;
    pendingSeek = true;
    dragging = false;
    onaction("seek", seek);
  }
</script>

<section
  class="player"
  use:bassReaction={{
    enabled: bass && mode === "immersive",
    active: state.playing,
    trackKey: state.currentKey,
    reduced: motion === 0,
  }}
  class:bass-mode={bass && mode === "immersive"}
  class:immersive={mode === "immersive"}
  class:mini-player={mode === "mini"}
  class:idle={!track}
  style:--accent={colors.accent}
  style:--player-bg={colors.background}
  style:--player-surface={colors.surface}
  style:--player-raised={colors.raised}
  style:--player-text={colors.foreground}
  style:--player-muted={colors.muted}
  style:--player-border={colors.border}
  style:--player-on-accent={colors.onAccent}
  style:--player-secondary={colors.secondary}
  use:reveal={{ key: mode, initial: false, y: 0, duration: 300 }}
  aria-label="Плеер"
>
  {#if mode === "immersive"}<div
      class="bass-glow"
      aria-hidden="true"
    ></div>{/if}
  {#if mode !== "dock"}
    <div class="player-top" data-tauri-drag-region>
      <span class="eyebrow">yukicloud</span>
      <div>
        {#if mode === "immersive"}<button
            class="bass-toggle"
            class:active={bass}
            aria-label="Бас-режим"
            aria-pressed={bass}
            data-tooltip="Визуальный отклик баса · B"
            onclick={onbass}
            ><Icon name="bass" size={18} /><span>Бас</span></button
          >{/if}
        <button
          class="icon-button"
          aria-label="Переключить визуалайзер"
          aria-pressed={visualizer}
          class:active={visualizer}
          onclick={onvisualizer}><Icon name="wave" size={18} /></button
        ><button
          class="icon-button"
          aria-label="Вернуться к коллекции"
          onclick={() => onmode("dock")}
          ><Icon name="collapse" size={18} /></button
        >
      </div>
    </div>
  {/if}
  <button
    class="player-art"
    data-tooltip="Открыть полноэкранный плеер"
    aria-label="Открыть обложку"
    disabled={!track}
    onclick={() => onmode(mode === "immersive" ? "dock" : "immersive")}
  >
    <Artwork
      src={track?.artwork || null}
      title={track?.title || ""}
      large={mode === "immersive"}
    />
  </button>
  <div
    class="player-track"
    use:reveal={{ key: track?.id, initial: false, y: 3, duration: 220 }}
  >
    <div class="player-track-heading">
      <h2 data-tooltip={track?.title}>{track?.title || "Ничего не играет"}</h2>
      <button
        class="icon-button player-like"
        class:active={liked}
        aria-label={liked
          ? "Убрать текущий трек из любимых"
          : "Сохранить текущий трек"}
        aria-pressed={liked}
        disabled={!track}
        onclick={() => track && onlike(track)}
        ><Icon name="heart" size={18} /></button
      >
    </div>
    <p>
      {#if state.loading}<span class="loading-status" role="status"
          >Загрузка…</span
        >{/if}{#if track}<button
          class="player-artist"
          onclick={async () => {
            if (mode !== "dock") await onmode("dock");
            if (track) onartist(track);
          }}>{track.artist}</button
        >{:else}Выбери трек в коллекции или поиске{/if}
    </p>
    {#if mode === "dock"}<div
        class="dock-spectrum"
        class:spectrum-hidden={!visualizer}
        aria-hidden={!visualizer}
      >
        <Visualizer
          active={state.playing}
          enabled={visualizer}
          color="var(--accent)"
          trackKey={state.currentKey}
        />
      </div>{/if}
  </div>
  <div class="player-center">
    <div class="transport">
      <button
        class="icon-button secondary-transport"
        class:active={state.shuffle}
        data-tooltip="Перемешать очередь"
        aria-label="Перемешать очередь"
        aria-pressed={state.shuffle}
        onclick={() => onaction("shuffle")}
        ><Icon name="shuffle" size={18} /></button
      >
      <button
        class="icon-button"
        data-tooltip="Предыдущий трек"
        aria-label="Предыдущий трек"
        disabled={!track}
        onclick={() => onaction("previous")}
        ><Icon name="previous" size={20} /></button
      >
      <button
        class="play-button"
        class:loading={state.loading}
        data-tooltip={state.loading
          ? "Загрузка трека · Нажми, чтобы приостановить"
          : "Воспроизведение / пауза · Пробел"}
        aria-label={state.playing || state.loading ? "Пауза" : "Воспроизвести"}
        aria-busy={state.loading}
        disabled={!track}
        onclick={() => onaction("toggle")}
        >{#if state.loading}<span class="play-spinner" aria-hidden="true"
          ></span>{:else}<Icon
            name={state.playing ? "pause" : "play"}
            size={23}
          />{/if}</button
      >
      <button
        class="icon-button"
        data-tooltip="Следующий трек"
        aria-label="Следующий трек"
        disabled={!track}
        onclick={() => onaction("next")}><Icon name="next" size={20} /></button
      >
      <button
        class="icon-button secondary-transport repeat-button"
        class:active={state.repeat !== "off"}
        data-tooltip={`Повтор: ${state.repeat === "off" ? "выключен" : state.repeat === "all" ? "очередь" : "трек"}`}
        aria-label="Режим повтора"
        onclick={() =>
          onaction(
            "repeat",
            state.repeat === "off"
              ? "all"
              : state.repeat === "all"
                ? "one"
                : "off",
          )}
        ><Icon name="repeat" size={18} />{#if state.repeat === "one"}<small
            >1</small
          >{/if}</button
      >
    </div>
    <div class="timeline">
      <span>{time(seek ?? state.position)}</span><input
        aria-label="Позиция воспроизведения"
        type="range"
        min="0"
        max={Math.max(1, duration)}
        step=".1"
        value={seek ?? state.position}
        disabled={!track}
        style:--fill={`${Math.min(100, ((seek ?? state.position) / Math.max(1, duration)) * 100)}%`}
        oninput={(e) => {
          dragging = true;
          seek = +e.currentTarget.value;
        }}
        onchange={commitSeek}
      /><span>{time(duration)}</span>
    </div>
  </div>
  {#if mode === "immersive"}<div
      class="player-spectrum"
      class:spectrum-hidden={!visualizer}
      aria-hidden={!visualizer}
    >
      <Visualizer
        active={state.playing}
        enabled={visualizer}
        large={mode === "immersive"}
        color="var(--player-secondary)"
        trackKey={state.currentKey}
      />
    </div>{/if}
  <div class="player-options">
    <div class="volume-control">
      <button
        class="icon-button"
        aria-label={state.muted ? "Включить звук" : "Выключить звук"}
        onclick={() => onaction("mute")}
        ><Icon
          name={state.muted || state.volume === 0 ? "mute" : "volume"}
          size={18}
        /></button
      ><input
        type="range"
        aria-label="Громкость приложения"
        data-tooltip={`Громкость ${Math.round(state.volume * 100)}%`}
        min="0"
        max="1"
        step=".01"
        value={state.muted ? 0 : state.volume}
        style:--fill={`${state.muted ? 0 : state.volume * 100}%`}
        oninput={(e) => onaction("volume", +e.currentTarget.value)}
      />
    </div>
    <button
      class="icon-button"
      class:active={panel === "eq"}
      data-tooltip="Эквалайзер"
      aria-label="Эквалайзер"
      onclick={() => onpanel("eq")}><Icon name="eq" size={19} /></button
    >
    <button
      class="icon-button"
      class:active={panel === "queue"}
      data-tooltip="Очередь · Q"
      aria-label="Открыть очередь"
      onclick={() => onpanel("queue")}><Icon name="queue" size={20} /></button
    >
    {#if mode === "dock"}<button
        class="icon-button player-mode"
        data-tooltip="Мини-плеер · M"
        aria-label="Мини-плеер"
        disabled={!track}
        onclick={() => onmode("mini")}><Icon name="mini" size={19} /></button
      ><button
        class="icon-button player-mode"
        data-tooltip="Полноэкранный плеер · F"
        aria-label="Полноэкранный плеер"
        disabled={!track}
        onclick={() => onmode("immersive")}
        ><Icon name="expand" size={19} /></button
      >{/if}
  </div>
</section>
