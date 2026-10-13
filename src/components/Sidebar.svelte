<script lang="ts">
  import Icon from "./Icon.svelte";
  import Artwork from "./Artwork.svelte";
  import { selectionIndicator } from "../lib/motion";
  import type { Library, Playlist } from "../lib/types";
  import type { View, Theme } from "../lib/catalog";
  export let view: View;
  export let library: Library;
  export let theme: Theme;
  export let selectedPlaylist: number | undefined = undefined;
  export let onnavigate: (view: View) => void;
  export let onplaylist: (playlist: Playlist) => void;
  export let ontheme: () => void;
  const sections = [
    { view: "tracks" as const, icon: "heart", label: "Любимые" },
    { view: "playlists" as const, icon: "library", label: "Плейлисты" },
    { view: "artists" as const, icon: "artist", label: "Артисты" },
  ];
</script>

<aside class="sidebar" aria-label="Навигация">
  <button
    class="brand"
    onclick={() => onnavigate("tracks")}
    aria-label="yukicloud — любимая музыка"
  >
    <span class="brand-symbol"><Icon name="cloud" size={25} /></span><span
      >yuki<span class="brand-light">cloud</span></span
    >
  </button>
  <nav class="primary-nav" aria-label="Разделы" use:selectionIndicator={view}>
    <i class="selection-indicator" aria-hidden="true"></i>
    <button
      class:active={view === "search"}
      aria-current={view === "search" ? "page" : undefined}
      onclick={() => onnavigate("search")}
      data-tooltip="Поиск · Ctrl K"
    >
      <Icon name="search" /><span>Поиск</span>
    </button>
    <div class="nav-caption">Моя музыка</div>
    {#each sections as item}
      <button
        class:active={view === item.view}
        aria-current={view === item.view ? "page" : undefined}
        onclick={() => onnavigate(item.view)}
        data-tooltip={item.label}
      >
        <Icon name={item.icon} /><span>{item.label}</span><small
          >{library[item.view].length || ""}</small
        >
      </button>
    {/each}
  </nav>
  {#if library.playlists.length}
    <div class="sidebar-playlists">
      <div class="nav-caption">Под рукой</div>
      {#each library.playlists.slice(0, 5) as playlist (playlist.id)}
        <button
          class:current={selectedPlaylist === playlist.id}
          class="sidebar-playlist"
          data-playlist-id={playlist.id}
          data-tooltip={playlist.title}
          onclick={() => onplaylist(playlist)}
        >
          <span class="sidebar-cover"><Artwork src={playlist.artwork} /></span
          ><span>{playlist.title}</span>
        </button>
      {/each}
    </div>
  {/if}
  <div class="sidebar-bottom">
    <button
      class="settings-link"
      class:active={view === "settings"}
      aria-current={view === "settings" ? "page" : undefined}
      data-tooltip="Настройки"
      onclick={() => onnavigate("settings")}
      ><Icon name="settings" size={19} /><span>Настройки</span></button
    >
    <button
      class="theme-toggle icon-button"
      data-tooltip={theme === "dark" ? "Светлая тема" : "Тёмная тема"}
      aria-label={theme === "dark"
        ? "Включить светлую тему"
        : "Включить тёмную тему"}
      onclick={ontheme}
      ><Icon name={theme === "dark" ? "sun" : "moon"} size={19} /></button
    >
  </div>
</aside>
