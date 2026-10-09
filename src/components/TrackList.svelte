<script lang="ts">
  import Icon from "./Icon.svelte";
  import Artwork from "./Artwork.svelte";
  import { time } from "../lib/format";
  import type { Track } from "../lib/types";
  import { paginate } from "../lib/paginate";
  export let tracks: Track[] = [];
  export let currentId: number | undefined = undefined;
  export let playing = false;
  export let liked = new Set<number>();
  export let onplay: (track: Track, index: number) => void;
  export let onlike: (track: Track) => void;
  export let onqueue: (track: Track, next: boolean) => void;
  export let onartist: (track: Track) => void;
  export let hasMore = false;
  export let loadingMore = false;
  export let onmore: () => Promise<void> = async () => {};
  let scrollTop = 0;
  let height = 400;
  const rowHeight = 56;
  $: start = Math.max(
    0,
    Math.min(
      Math.floor(scrollTop / rowHeight) - 5,
      tracks.length - Math.ceil(height / rowHeight),
    ),
  );
  $: end = Math.min(tracks.length, start + Math.ceil(height / rowHeight) + 12);
  $: visible = tracks.slice(start, end);
</script>

<div
  class="track-table"
  role="table"
  aria-label="Треки"
  aria-rowcount={tracks.length}
>
  <div class="track-heading" role="row">
    <span>№</span><span>ТРЕК / АРТИСТ</span><span>ВРЕМЯ</span><span></span>
  </div>
  <div
    class="track-scroll"
    bind:clientHeight={height}
    onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}
  >
    <div style:height={`${start * rowHeight}px`}></div>
    {#each visible as track, i (`${track.id}-${start + i}`)}
      <div
        class="track-row"
        data-track-id={track.id}
        class:current={currentId === track.id}
        role="row"
        tabindex="-1"
        aria-rowindex={start + i + 1}
        draggable="true"
        ondragstart={(e) =>
          e.dataTransfer?.setData(
            "application/yukicloud-track",
            JSON.stringify(track),
          )}
      >
        <button
          class="track-number"
          title={`Включить ${track.title}`}
          aria-label={`Включить ${track.title}`}
          onclick={() => onplay(track, start + i)}
          disabled={!track.available}
        >
          {#if currentId === track.id && playing}<Icon
              name="wave"
              size={16}
            />{:else}<span>{String(start + i + 1).padStart(2, "0")}</span><Icon
              name="play"
              size={14}
            />{/if}
        </button>
        <div class="track-info">
          <button
            class="track-cover"
            aria-label={`Включить ${track.title}`}
            onclick={() => onplay(track, start + i)}
            disabled={!track.available}
            ><Artwork src={track.artwork} title="" /></button
          >
          <div>
            <button
              class="track-title"
              onclick={() => onplay(track, start + i)}
              disabled={!track.available}>{track.title}</button
            ><button class="artist-link" onclick={() => onartist(track)}
              >{track.artist}</button
            >
          </div>
        </div>
        <span class="track-duration">{time(track.duration)}</span>
        <div class="track-actions">
          <button
            class="icon-button"
            class:active={liked.has(track.id)}
            aria-label={liked.has(track.id)
              ? "Убрать из коллекции"
              : "В коллекцию"}
            title="В коллекцию"
            onclick={() => onlike(track)}
            ><Icon name="heart" size={16} /></button
          >
          <button
            class="icon-button"
            aria-label="Играть следующим"
            title="Играть следующим"
            onclick={() => onqueue(track, true)}
            ><Icon name="next" size={16} /></button
          >
          <button
            class="icon-button"
            aria-label="В конец очереди"
            title="В конец очереди"
            onclick={() => onqueue(track, false)}
            ><Icon name="plus" size={17} /></button
          >
        </div>
      </div>
    {/each}
    <div
      style:height={`${Math.max(0, tracks.length - end) * rowHeight}px`}
    ></div>
    {#if hasMore || loadingMore}<div
        class="load-marker"
        class:loading={loadingMore}
        use:paginate={{ enabled: hasMore && !loadingMore, load: onmore }}
      >
        {#if loadingMore}<span class="spinner" aria-label="Загрузка треков"
          ></span>{/if}
      </div>{/if}
  </div>
</div>
