<script lang="ts">
  import type { Artist, Playlist } from "../lib/types";
  import Icon from "./Icon.svelte";
  import Artwork from "./Artwork.svelte";
  import { countLabel } from "../lib/format";
  import { paginate } from "../lib/paginate";
  import { reveal, motionDuration, easeOut } from "../lib/motion";
  import { flip } from "svelte/animate";
  export let kind: "artists" | "playlists";
  export let artists: Artist[] = [];
  export let playlists: Playlist[] = [];
  export let saved = new Set<number>();
  export let onartist: (artist: Artist) => void;
  export let onplaylist: (playlist: Playlist) => void;
  export let onsaveartist: (artist: Artist) => void;
  export let onsaveplaylist: (playlist: Playlist) => void;
  export let onplayartist: (artist: Artist) => void;
  export let onplayplaylist: (playlist: Playlist) => void;
  export let pendingCollection: string | null = null;
  export let hasMore = false;
  export let loading = false;
  export let onmore: () => Promise<void> = async () => {};
</script>

<div
  class="collection-scroll"
  use:reveal={{ key: kind }}
  aria-label={kind === "artists" ? "Артисты" : "Плейлисты"}
>
  <div class="collection-cards" class:artist-cards={kind === "artists"}>
    {#if kind === "artists"}
      {#each artists as artist (artist.id)}
        <article
          class="collection-card artist-item"
          data-artist-id={artist.id}
          animate:flip={{ duration: motionDuration(280), easing: easeOut }}
        >
          <button
            class="collection-open artist-main"
            onclick={() => onartist(artist)}
            data-tooltip={`Открыть ${artist.name}`}
          >
            <span class="collection-art artist-portrait"
              ><Artwork src={artist.artwork} /></span
            >
            <strong>{artist.name}</strong><small
              >Артист{artist.trackCount
                ? ` · ${countLabel(artist.trackCount)}`
                : ""}</small
            >
          </button>
          {#if !saved.has(artist.id)}<div class="card-collection-action">
              <button
                class="icon-button card-save"
                aria-label={`Сохранить ${artist.name}`}
                onclick={() => onsaveartist(artist)}
                ><Icon name="plus" size={17} /></button
              >
            </div>{/if}
          <div class="card-actions">
            <button
              class="card-play"
              aria-label={`Слушать ${artist.name}`}
              aria-busy={pendingCollection === `artist:${artist.id}`}
              onclick={() => onplayartist(artist)}
              >{#if pendingCollection === `artist:${artist.id}`}<span
                  class="spinner"
                  aria-hidden="true"
                ></span>{:else}<Icon name="play" size={18} />{/if}</button
            >
          </div>
        </article>
      {/each}
    {:else}
      {#each playlists as playlist (playlist.id)}
        <article
          class="collection-card playlist-tile"
          data-playlist-id={playlist.id}
          animate:flip={{ duration: motionDuration(280), easing: easeOut }}
        >
          <button
            class="collection-open playlist-open"
            onclick={() => onplaylist(playlist)}
            data-tooltip={`Открыть ${playlist.title}`}
          >
            <span class="collection-art playlist-image"
              ><Artwork src={playlist.artwork} /></span
            >
            <strong>{playlist.title}</strong><small
              >{countLabel(playlist.trackCount)} · {playlist.artist}</small
            >
          </button>
          {#if !saved.has(playlist.id)}<div class="card-collection-action">
              <button
                class="icon-button card-save"
                aria-label={`Сохранить ${playlist.title}`}
                onclick={() => onsaveplaylist(playlist)}
                ><Icon name="plus" size={17} /></button
              >
            </div>{/if}
          <div class="card-actions">
            <button
              class="card-play"
              aria-label={`Слушать ${playlist.title}`}
              aria-busy={pendingCollection === `playlist:${playlist.id}`}
              onclick={() => onplayplaylist(playlist)}
              >{#if pendingCollection === `playlist:${playlist.id}`}<span
                  class="spinner"
                  aria-hidden="true"
                ></span>{:else}<Icon name="play" size={18} />{/if}</button
            >
          </div>
        </article>
      {/each}
    {/if}
  </div>
  {#if hasMore || loading}<div
      class="load-marker"
      class:loading
      use:paginate={{ enabled: hasMore && !loading, load: onmore }}
    >
      {#if loading}<span class="spinner" aria-label="Загрузка"></span>{/if}
    </div>{/if}
</div>
