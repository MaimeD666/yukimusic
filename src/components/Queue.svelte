<script lang="ts">
  import Icon from "./Icon.svelte";
  import Artwork from "./Artwork.svelte";
  import { time } from "../lib/format";
  import type { Playback, Track } from "../lib/types";
  export let state: Playback;
  export let onaction: (action: string, value?: unknown) => void;
  export let onclose: () => void;
  let dragKey = "";
  let overKey = "";
  let scrollTop = 0;
  let height = 430;
  $: start = Math.max(
    0,
    Math.min(
      Math.floor(scrollTop / 60) - 4,
      state.queue.length - Math.ceil(height / 60),
    ),
  );
  $: end = Math.min(state.queue.length, start + Math.ceil(height / 60) + 10);
  function drop(event: DragEvent, key: string) {
    event.preventDefault();
    event.stopPropagation();
    overKey = "";
    const serialized = event.dataTransfer?.getData(
      "application/yukicloud-track",
    );
    if (serialized) {
      try {
        onaction("enqueue", {
          tracks: [JSON.parse(serialized) as Track],
          next: false,
        });
      } catch {
        /* invalid drop */
      }
    } else if (dragKey) onaction("move", { from: dragKey, to: key });
    dragKey = "";
  }
</script>

<section class="queue-panel" aria-label="Очередь воспроизведения">
  <div class="panel-heading">
    <div>
      <span class="eyebrow"
        >СЛУШАТЬ ДАЛЬШЕ / {String(state.queue.length).padStart(2, "0")}</span
      >
      <h2>Очередь<span class="heading-star">✦</span></h2>
    </div>
    <button class="icon-button" aria-label="Закрыть очередь" onclick={onclose}
      ><Icon name="close" /></button
    >
  </div>
  {#if state.queue.length}<div class="queue-tools">
      <span>Перетащи, чтобы изменить порядок</span><button
        onclick={() => onaction("clear")}>Очистить</button
      >
    </div>{/if}
  <div
    class="queue-list"
    role="group"
    aria-label="Треки в очереди"
    bind:clientHeight={height}
    onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}
    ondragover={(e) => e.preventDefault()}
    ondrop={(e) => drop(e, state.queue.at(-1)?.key || "")}
  >
    <div style:height={`${start * 60}px`}></div>
    {#each state.queue.slice(start, end) as entry, offset (entry.key)}
      {@const index = start + offset}
      <div
        class="queue-item"
        data-queue-key={entry.key}
        class:current={entry.key === state.currentKey}
        class:drop-target={overKey === entry.key}
        draggable="true"
        role="group"
        aria-label={entry.track.title}
        ondragstart={() => (dragKey = entry.key)}
        ondragend={() => {
          dragKey = "";
          overKey = "";
        }}
        ondragover={(e) => {
          e.preventDefault();
          overKey = entry.key;
        }}
        ondrop={(e) => drop(e, entry.key)}
      >
        <span class="queue-grip"><Icon name="grip" size={14} /></span><button
          class="queue-track"
          disabled={!entry.track.available}
          onclick={() => onaction("jump", entry.key)}
          ><div class="queue-art"><Artwork src={entry.track.artwork} /></div>
          <span
            ><strong>{entry.track.title}</strong><small
              >{entry.track.available
                ? entry.track.artist
                : "Недоступен"}</small
            ></span
          ></button
        ><small>{time(entry.track.duration)}</small>
        <div class="queue-item-actions">
          <button
            class="icon-button"
            aria-label="Поднять в очереди"
            disabled={index === 0}
            onclick={() =>
              onaction("move", {
                from: entry.key,
                to: state.queue[index - 1]?.key,
              })}><span class="arrow-up">↑</span></button
          ><button
            class="icon-button"
            aria-label="Опустить в очереди"
            disabled={index === state.queue.length - 1}
            onclick={() =>
              onaction("move", {
                from: entry.key,
                to: state.queue[index + 1]?.key,
              })}>↓</button
          ><button
            class="icon-button"
            aria-label={`Удалить ${entry.track.title} из очереди`}
            onclick={() => onaction("remove", entry.key)}
            ><Icon name="close" size={15} /></button
          >
        </div>
      </div>
    {:else}<div class="empty-small">
        <Icon name="queue" size={32} />
        <h3>Здесь начинается следующий трек</h3>
        <p>Добавляй музыку кнопкой + или перетаскивай треки.</p>
      </div>{/each}
    <div style:height={`${Math.max(0, state.queue.length - end) * 60}px`}></div>
  </div>
</section>
