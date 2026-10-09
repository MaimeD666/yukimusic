<script lang="ts">
  import { onMount } from "svelte";
  import { fade, fly } from "svelte/transition";
  import { api, native } from "./lib/bridge";
  import { palette } from "./lib/artwork";
  import { neutralPalette } from "./lib/palette";
  import { paginate } from "./lib/paginate";
  import { defaultPlayback } from "./lib/types";
  import type {
    Artist,
    Library,
    Playback,
    Playlist,
    SearchResults,
    Track,
  } from "./lib/types";
  import { countLabel } from "./lib/format";
  import Icon from "./components/Icon.svelte";
  import Artwork from "./components/Artwork.svelte";
  import TrackList from "./components/TrackList.svelte";
  import Player from "./components/Player.svelte";
  import Equalizer from "./components/Equalizer.svelte";
  import Queue from "./components/Queue.svelte";
  import ContextMenu from "./components/ContextMenu.svelte";
  import type { Menu, MenuItem } from "./lib/menu";
  let menu: Menu | null = null;

  let query = "";
  let searchInput: HTMLInputElement;
  let library: Library = { artists: [], playlists: [], tracks: [] };
  let state: Playback = defaultPlayback();
  let results: SearchResults | null = null;
  let searching = false;
  let context: {
    kind: "artist" | "playlist";
    id: number;
    title: string;
    artwork: string | null;
    artworkVerified?: boolean;
  } | null = null;
  let contextTracks: Track[] = [];
  let catalogLoading = false;
  let searchError = "";
  let catalogError = "";
  let notice = "";
  let panel: "queue" | "eq" | null = null;
  let mode: "dock" | "immersive" | "mini" = "dock";
  let changingMode = false;
  const accent = "#e5dde8";
  let playerPalette = neutralPalette;
  let visualizer = true;
  let bass = false;
  $: if (ready) {
    mode;
    bass;
    visualizer;
    visualizationVisibility();
  }
  let searchRevision = 0;
  let catalogRevision = 0;
  let colorRevision = 0;
  let paletteUrl: string | null | undefined;
  let noticeTimer: ReturnType<typeof setTimeout>;
  let searchTimer: ReturnType<typeof setTimeout>;
  let eqTimer: ReturnType<typeof setTimeout>;
  let motion = 240;
  let disposed = false;
  let saving: Promise<void> = Promise.resolve();
  let viewTimer: ReturnType<typeof setTimeout>;
  let ready = false;
  type ResultKind = "artists" | "playlists" | "tracks";
  let pageLoading = { artists: false, playlists: false, tracks: false };
  let pageErrors = { artists: "", playlists: "", tracks: "" };
  let emptyPages = { artists: 0, playlists: 0, tracks: 0 };
  const pendingPages = new Set<string>();
  const loadedPages = new Set<string>();
  $: rememberView(query, results, context, contextTracks, ready);
  function rememberView(
    q: typeof query,
    r: typeof results,
    c: typeof context,
    t: Track[],
    loaded: boolean,
  ) {
    if (!loaded) return;
    clearTimeout(viewTimer);
    viewTimer = setTimeout(() => {
      try {
        localStorage.setItem(
          "yukicloud.view",
          JSON.stringify({ query: q, results: r, context: c, tracks: t }),
        );
      } catch {
        /* The collection is stored separately by Rust. */
      }
    }, 400);
  }

  $: artists = results?.artists ?? library.artists;
  $: playlists = results?.playlists ?? library.playlists;
  $: tracks = context ? contextTracks : (results?.tracks ?? library.tracks);
  $: liked = new Set(library.tracks.map((t) => t.id));
  $: artistIds = new Set(library.artists.map((a) => a.id));
  $: playlistIds = new Set(library.playlists.map((p) => p.id));
  $: current = state.queue.find((e) => e.key === state.currentKey)?.track;
  $: compactCollection =
    !results &&
    !context &&
    library.artists.length <= 6 &&
    library.playlists.length <= 4 &&
    library.tracks.length <= 8;
  $: updatePalette(current?.artwork || null);
  async function updatePalette(url: string | null) {
    if (paletteUrl === url) return;
    paletteUrl = url;
    const revision = ++colorRevision;
    const next = await palette(url);
    if (revision === colorRevision) playerPalette = next;
  }

  function toast(message: string) {
    notice = message;
    clearTimeout(noticeTimer);
    noticeTimer = setTimeout(() => (notice = ""), 4200);
  }
  function errorMessage(error: unknown) {
    return error instanceof Error ? error.message : String(error);
  }
  function persist() {
    const snapshot = structuredClone(library);
    saving = saving
      .catch(() => {})
      .then(() => api.saveLibrary(snapshot))
      .catch((e) =>
        toast(`Не удалось сохранить коллекцию: ${errorMessage(e)}`),
      );
  }
  function like(track: Track) {
    library = {
      ...library,
      tracks: liked.has(track.id)
        ? library.tracks.filter((t) => t.id !== track.id)
        : [track, ...library.tracks],
    };
    persist();
  }
  function saveArtist(artist: Artist) {
    library = {
      ...library,
      artists: artistIds.has(artist.id)
        ? library.artists.filter((a) => a.id !== artist.id)
        : [...library.artists, artist],
    };
    persist();
  }
  function savePlaylist(playlist: Playlist) {
    library = {
      ...library,
      playlists: playlistIds.has(playlist.id)
        ? library.playlists.filter((p) => p.id !== playlist.id)
        : [...library.playlists, playlist],
    };
    persist();
  }
  function saveContext() {
    if (!context) return;
    const shared = {
      id: context.id,
      artwork: context.artwork,
      trackCount: contextTracks.length,
      permalink: "",
    };
    if (context.kind === "artist")
      saveArtist({
        ...shared,
        name: context.title,
        artworkVerified: context.artworkVerified,
      });
    else
      savePlaylist({
        ...shared,
        title: context.title,
        artist: contextTracks[0]?.artist || "",
      });
  }
  async function action(name: string, value: unknown = null) {
    try {
      await api.action(name, value);
    } catch (e) {
      toast(errorMessage(e));
    }
  }
  function play(track: Track, index: number) {
    if (current?.id === track.id) void action("toggle");
    else void action("play", { tracks, index });
  }
  function addToQueue(track: Track, next: boolean) {
    void action("enqueue", { tracks: [track], next });
    toast(next ? "Трек будет следующим" : "Трек добавлен в конец очереди");
  }
  function onSearch() {
    emptyPages = { artists: 0, playlists: 0, tracks: 0 };
    pageLoading = { artists: false, playlists: false, tracks: false };
    pageErrors = { artists: "", playlists: "", tracks: "" };
    pendingPages.clear();
    loadedPages.clear();
    clearTimeout(searchTimer);
    const revision = ++searchRevision;
    context = null;
    ++catalogRevision;
    catalogLoading = false;
    searchError = "";
    if (!query.trim()) {
      results = null;
      searching = false;
      return;
    }
    searching = true;
    searchTimer = setTimeout(() => runSearch(revision), 280);
  }
  async function runSearch(revision: number) {
    try {
      if (
        /^https:\/\/(?:www\.)?(?:soundcloud\.com|on\.soundcloud\.com)\//i.test(
          query.trim(),
        )
      ) {
        const resolved = await api.resolve(query.trim());
        if (revision !== searchRevision) return;
        if (resolved.kind === "user")
          await openArtist(resolved.entity as Artist);
        else if (resolved.kind === "playlist")
          await openPlaylist(resolved.entity as Playlist);
        else
          results = {
            artists: [],
            playlists: [],
            tracks: [resolved.entity as Track],
            next: { artists: null, playlists: null, tracks: null },
            errors: [],
          };
      } else {
        const found = await api.search(query.trim());
        if (revision !== searchRevision) return;
        results = found;
        searchError = found.errors.join(" ");
      }
    } catch (e) {
      if (revision === searchRevision) searchError = errorMessage(e);
    } finally {
      if (revision === searchRevision) searching = false;
    }
  }
  function clearSearch() {
    query = "";
    onSearch();
    searchInput?.focus();
  }
  async function more(kind: ResultKind) {
    const url = results?.next[kind];
    if (!url || searching || pageLoading[kind] || pageErrors[kind]) return;
    const revision = searchRevision;
    const key = `${revision}:${kind}:${url}`;
    if (pendingPages.has(key) || loadedPages.has(key)) return;
    pendingPages.add(key);
    pageLoading = { ...pageLoading, [kind]: true };
    try {
      const page = await api.searchMore(kind, url);
      if (revision === searchRevision && results) {
        loadedPages.add(key);
        const seen = new Set(results[kind].map((item) => item.id));
        const added = page[kind].filter((item) => {
          if (seen.has(item.id)) return false;
          seen.add(item.id);
          return true;
        });
        const next = page.next[kind];
        emptyPages[kind] = added.length ? 0 : emptyPages[kind] + 1;
        if (next && emptyPages[kind] >= 3)
          pageErrors = {
            ...pageErrors,
            [kind]:
              "SoundCloud вернул несколько страниц без новых результатов.",
          };
        results = {
          ...results,
          [kind]: [...results[kind], ...added],
          next: {
            ...results.next,
            [kind]:
              next && !loadedPages.has(`${revision}:${kind}:${next}`)
                ? next
                : null,
          },
        };
      }
    } catch (e) {
      if (revision === searchRevision)
        pageErrors = { ...pageErrors, [kind]: errorMessage(e) };
    } finally {
      pendingPages.delete(key);
      if (revision === searchRevision)
        pageLoading = { ...pageLoading, [kind]: false };
    }
  }
  async function retryPage(kind: ResultKind) {
    emptyPages[kind] = 0;
    pageErrors = { ...pageErrors, [kind]: "" };
    await more(kind);
  }
  async function openArtist(artist: Artist) {
    const revision = ++catalogRevision;
    context = {
      kind: "artist",
      id: artist.id,
      title: artist.name,
      artwork: artist.artworkVerified ? artist.artwork : null,
      artworkVerified: artist.artworkVerified,
    };
    if (!artist.artworkVerified && native)
      void verifiedArtist(artist)
        .then((profile) => {
          if (revision === catalogRevision && context)
            context = {
              ...context,
              artwork: profile.artwork,
              artworkVerified: true,
            };
        })
        .catch(() => {});
    contextTracks = [];
    catalogLoading = true;
    catalogError = "";
    try {
      let next: string | null = null;
      const seenPages = new Set<string>();
      const seen = new Set<number>();
      do {
        const page = await api.artistTracks(artist.id, next);
        if (revision !== catalogRevision || disposed) return;
        contextTracks = [
          ...contextTracks,
          ...page.tracks.filter((t) => {
            if (seen.has(t.id)) return false;
            seen.add(t.id);
            return true;
          }),
        ];
        next = page.next;
        if (next && seenPages.has(next))
          throw new Error(
            "SoundCloud повторил страницу. Загруженные треки сохранены.",
          );
        if (next) seenPages.add(next);
      } while (next);
    } catch (e) {
      if (revision === catalogRevision) catalogError = errorMessage(e);
    } finally {
      if (revision === catalogRevision) catalogLoading = false;
    }
  }
  async function openPlaylist(playlist: Playlist) {
    const revision = ++catalogRevision;
    context = {
      kind: "playlist",
      id: playlist.id,
      title: playlist.title,
      artwork: playlist.artwork,
    };
    contextTracks = [];
    catalogLoading = true;
    catalogError = "";
    try {
      const page = await api.playlistTracks(playlist.id);
      if (revision === catalogRevision) contextTracks = page.tracks;
    } catch (e) {
      if (revision === catalogRevision) catalogError = errorMessage(e);
    } finally {
      if (revision === catalogRevision) catalogLoading = false;
    }
  }
  async function playPlaylist(playlist: Playlist) {
    await openPlaylist(playlist);
    if (
      context?.kind === "playlist" &&
      context.id === playlist.id &&
      contextTracks.length &&
      !catalogError
    ) {
      await action("play", {
        tracks: contextTracks,
        index: Math.max(
          0,
          contextTracks.findIndex((t) => t.available),
        ),
      });
    }
  }
  function trackArtist(track: Track) {
    void openArtist({
      id: track.artistId,
      name: track.artist,
      artwork: track.artwork,
      trackCount: 0,
      permalink: "",
    });
  }
  const artistProfiles = new Map<number, Promise<Artist>>();
  async function verifiedArtist(artist: Artist): Promise<Artist> {
    if (!artistProfiles.has(artist.id))
      artistProfiles.set(
        artist.id,
        api.artistProfile(artist.id).catch((error) => {
          artistProfiles.delete(artist.id);
          throw error;
        }),
      );
    const profile = await artistProfiles.get(artist.id)!;
    const resolved = { ...profile, artwork: profile.artwork || artist.artwork };
    if (library.artists.some((a) => a.id === artist.id && !a.artworkVerified)) {
      library = {
        ...library,
        artists: library.artists.map((a) =>
          a.id === artist.id ? resolved : a,
        ),
      };
      persist();
    }
    return resolved;
  }
  function closeContext() {
    ++catalogRevision;
    context = null;
    catalogLoading = false;
    catalogError = "";
  }
  async function changeMode(next: typeof mode) {
    if (next === mode || changingMode) return;
    if (next !== "dock" && !current) return;
    const previous = mode;
    changingMode = true;
    menu = null;
    try {
      await api.window(
        next === "mini"
          ? "mini"
          : next === "immersive"
            ? "immersive"
            : "restore",
      );
      mode = next;
      panel = null;
    } catch (e) {
      mode = previous;
      toast(errorMessage(e));
    } finally {
      changingMode = false;
    }
  }
  function togglePanel(name: string) {
    if (mode === "mini") {
      void changeMode("dock").then(() => (panel = name as typeof panel));
    } else panel = panel === name ? null : (name as typeof panel);
  }
  function setEq(bands: number[], enabled: boolean) {
    state = { ...state, eq: bands, eqEnabled: enabled };
    clearTimeout(eqTimer);
    eqTimer = setTimeout(() => action("eq", { bands, enabled }), 70);
  }
  function keydown(event: KeyboardEvent) {
    if (menu) return;
    const typing = (event.target as HTMLElement)?.matches(
      "input, textarea, [contenteditable]",
    );
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
      event.preventDefault();
      if (mode !== "dock")
        void changeMode("dock").then(() => searchInput?.focus());
      else searchInput?.focus();
      return;
    }
    if (event.key === "Escape") {
      if (panel) panel = null;
      else if (mode !== "dock") void changeMode("dock");
      else if (context) closeContext();
      else if (query) clearSearch();
      return;
    }
    if (typing || event.ctrlKey || event.metaKey || event.altKey) return;
    if (event.code === "Space") {
      event.preventDefault();
      void action("toggle");
    }
    if (event.key.toLowerCase() === "q") togglePanel("queue");
    if (!event.repeat && event.key.toLowerCase() === "f")
      void changeMode(mode === "immersive" ? "dock" : "immersive");
    if (!event.repeat && event.key.toLowerCase() === "m")
      void changeMode(mode === "mini" ? "dock" : "mini");
    if (
      !event.repeat &&
      mode === "immersive" &&
      event.key.toLowerCase() === "b"
    )
      toggleBass();
    if (event.key === "ArrowRight")
      void action(event.shiftKey ? "next" : "seek", state.position + 5);
    if (event.key === "ArrowLeft")
      void action(
        event.shiftKey ? "previous" : "seek",
        Math.max(0, state.position - 5),
      );
  }
  function visualizationVisibility() {
    void api
      .visualization(
        !document.hidden,
        visualizer || (bass && mode === "immersive"),
      )
      .catch(() => {});
  }
  function toggleBass() {
    bass = !bass;
    localStorage.setItem("yukicloud.bass", String(bass));
  }
  function toggleVisualizer() {
    visualizer = !visualizer;
    localStorage.setItem("yukicloud.visualizer", String(visualizer));
    visualizationVisibility();
  }
  function contextmenu(event: MouseEvent) {
    event.preventDefault();
    const target = event.target instanceof Element ? event.target : null;
    if (!target || target.closest(".app-context-menu")) return;
    const items: MenuItem[] = [];
    const copy = async (text: string) => {
      await navigator.clipboard.writeText(text);
      toast("Ссылка скопирована");
    };
    const input = target.closest<HTMLInputElement>(
      'input:not([type="range"]), textarea',
    );
    const queued = state.queue.find(
      (e) =>
        e.key ===
        target.closest<HTMLElement>("[data-queue-key]")?.dataset.queueKey,
    );
    const trackId = Number(
      target.closest<HTMLElement>("[data-track-id]")?.dataset.trackId,
    );
    const track =
      queued?.track ||
      tracks.find((t) => t.id === trackId) ||
      (target.closest(".player-art, .player-track") ? current : undefined);
    const artistId = Number(
      target.closest<HTMLElement>("[data-artist-id]")?.dataset.artistId,
    );
    const artist = artists.find((a) => a.id === artistId);
    const playlistId = Number(
      target.closest<HTMLElement>("[data-playlist-id]")?.dataset.playlistId,
    );
    const playlist = playlists.find((p) => p.id === playlistId);
    if (input) {
      const start = input.selectionStart || 0,
        end = input.selectionEnd || 0;
      const selected = input.value.slice(start, end);
      items.push(
        {
          label: "Вырезать",
          icon: "trash",
          disabled: !selected,
          run: async () => {
            await navigator.clipboard.writeText(selected);
            input.focus();
            input.setRangeText("", start, end, "end");
            input.dispatchEvent(new Event("input", { bubbles: true }));
          },
        },
        {
          label: "Копировать",
          icon: "link",
          disabled: !selected,
          run: () => navigator.clipboard.writeText(selected),
        },
        {
          label: "Вставить",
          icon: "plus",
          run: async () => {
            const text = await navigator.clipboard.readText();
            input.focus();
            input.setRangeText(text, start, end, "end");
            input.dispatchEvent(new Event("input", { bubbles: true }));
          },
        },
        {
          label: "Выделить всё",
          separator: true,
          run: () => {
            input.focus();
            input.select();
          },
        },
      );
    } else if (track) {
      const playingThis = queued
        ? queued.key === state.currentKey
        : track.id === current?.id;
      items.push(
        {
          label:
            playingThis && (state.playing || state.loading)
              ? "Пауза"
              : "Воспроизвести",
          icon: playingThis && state.playing ? "pause" : "play",
          disabled: !track.available,
          run: () => {
            if (playingThis) return action("toggle");
            if (queued) return action("jump", queued.key);
            const index = tracks.findIndex((t) => t.id === track.id);
            return action("play", {
              tracks: index >= 0 ? tracks : [track],
              index: Math.max(0, index),
            });
          },
        },
        {
          label: "Играть следующим",
          icon: "next",
          disabled: !track.available,
          run: () => addToQueue(track, true),
        },
        {
          label: "В конец очереди",
          icon: "plus",
          disabled: !track.available,
          run: () => addToQueue(track, false),
        },
        {
          label: liked.has(track.id) ? "Убрать из любимых" : "В любимые",
          icon: "heart",
          separator: true,
          run: () => like(track),
        },
        {
          label: "Открыть артиста",
          icon: "arrow",
          run: async () => {
            if (mode !== "dock") await changeMode("dock");
            panel = null;
            trackArtist(track);
          },
        },
        {
          label: "Скопировать ссылку",
          icon: "link",
          disabled: !track.permalink,
          run: () => copy(track.permalink),
        },
      );
      if (queued)
        items.push({
          label: "Удалить из очереди",
          icon: "trash",
          separator: true,
          run: () => action("remove", queued.key),
        });
    } else if (artist) {
      items.push(
        { label: "Открыть артиста", run: () => openArtist(artist) },
        {
          label: "Слушать",
          icon: "play",
          run: async () => {
            await openArtist(artist);
            if (
              context?.kind === "artist" &&
              context.id === artist.id &&
              contextTracks.length
            )
              await action("play", {
                tracks: contextTracks,
                index: Math.max(
                  0,
                  contextTracks.findIndex((t) => t.available),
                ),
              });
          },
        },
        {
          label: artistIds.has(artist.id)
            ? "Убрать из коллекции"
            : "В коллекцию",
          icon: "heart",
          separator: true,
          run: () => saveArtist(artist),
        },
        {
          label: "Скопировать ссылку",
          icon: "link",
          disabled: !artist.permalink,
          run: () => copy(artist.permalink),
        },
      );
    } else if (playlist) {
      items.push(
        { label: "Открыть плейлист", run: () => openPlaylist(playlist) },
        { label: "Слушать", icon: "play", run: () => playPlaylist(playlist) },
        {
          label: playlistIds.has(playlist.id)
            ? "Убрать из коллекции"
            : "В коллекцию",
          icon: "heart",
          separator: true,
          run: () => savePlaylist(playlist),
        },
        {
          label: "Скопировать ссылку",
          icon: "link",
          disabled: !playlist.permalink,
          run: () => copy(playlist.permalink),
        },
      );
    } else {
      items.push(
        {
          label: "Найти музыку",
          icon: "search",
          run: async () => {
            if (mode !== "dock") await changeMode("dock");
            searchInput?.focus();
          },
        },
        { label: "Очередь", icon: "queue", run: () => togglePanel("queue") },
        { label: "Эквалайзер", icon: "eq", run: () => togglePanel("eq") },
      );
      if (current)
        items.push({
          label:
            mode === "dock" ? "Полноэкранный плеер" : "Вернуться к коллекции",
          icon: mode === "dock" ? "expand" : "collapse",
          separator: true,
          run: () => changeMode(mode === "dock" ? "immersive" : "dock"),
        });
      if (mode === "immersive")
        items.push({
          label: bass ? "Выключить бас-режим" : "Включить бас-режим",
          icon: "bass",
          run: toggleBass,
        });
    }
    const rect = target.getBoundingClientRect();
    menu = {
      x: event.clientX || rect.left + 12,
      y: event.clientY || rect.bottom,
      items,
    };
  }

  onMount(() => {
    let off: (() => void) | undefined;
    motion = matchMedia("(prefers-reduced-motion: reduce)").matches ? 0 : 240;
    visualizer = localStorage.getItem("yukicloud.visualizer") !== "false";
    bass = localStorage.getItem("yukicloud.bass") === "true";
    try {
      const view = JSON.parse(localStorage.getItem("yukicloud.view") || "null");
      if (view) {
        query = view.query || "";
        results = view.results;
        context = view.context;
        contextTracks = view.tracks || [];
      }
    } catch {
      /* Fresh view. */
    }
    ready = true;
    // Subscribe first, then request a snapshot so no action is lost during mount.
    void (async () => {
      off = await api.subscribe((next) => {
        state = next;
      }, toast);
      if (disposed) {
        off();
        return;
      }
      state = await api.snapshot();
      library = await api.library();
      if (native) {
        const legacy = library.artists
          .filter((a) => !a.artworkVerified)
          .slice(0, 12);
        for (let worker = 0; worker < 2; worker++)
          void (async () => {
            while (legacy.length && !disposed) {
              const artist = legacy.shift()!;
              try {
                await verifiedArtist(artist);
              } catch {
                /* Retry when opened. */
              }
            }
          })();
      }
      visualizationVisibility();
    })().catch((e) => toast(errorMessage(e)));
    document.addEventListener("visibilitychange", visualizationVisibility);
    return () => {
      disposed = true;
      off?.();
      clearTimeout(searchTimer);
      clearTimeout(noticeTimer);
      clearTimeout(eqTimer);
      clearTimeout(viewTimer);
      ++catalogRevision;
      ++searchRevision;
      document.removeEventListener("visibilitychange", visualizationVisibility);
    };
  });
</script>

<svelte:window onkeydown={keydown} oncontextmenu={contextmenu} />
{#if menu}{#key menu}<ContextMenu
      {menu}
      onclose={() => (menu = null)}
      onerror={(e) => toast(errorMessage(e))}
    />{/key}{/if}
<div class="application" class:mini={mode === "mini"} style:--accent={accent}>
  {#if mode === "dock"}
    <header class="library-header" data-tauri-drag-region>
      <button
        class="wordmark"
        aria-label="yukicloud — коллекция"
        title="Вернуться к коллекции"
        onclick={() => {
          clearSearch();
          closeContext();
        }}>yuki<span>cloud</span></button
      >
      <form
        class="search"
        onsubmit={(e) => {
          e.preventDefault();
          if (!query.trim()) return;
          clearTimeout(searchTimer);
          searching = true;
          void runSearch(++searchRevision);
        }}
      >
        <Icon name="search" size={19} />
        <input
          bind:this={searchInput}
          bind:value={query}
          oninput={onSearch}
          placeholder="Артисты, треки, плейлисты"
          aria-label="Поиск музыки"
          spellcheck="false"
          autocomplete="off"
        />
        {#if searching}<span class="spinner" aria-label="Поиск"
          ></span>{:else if query}<button
            type="button"
            class="icon-button"
            aria-label="Очистить поиск"
            onclick={clearSearch}><Icon name="close" size={17} /></button
          >{:else}<kbd>Ctrl K</kbd>{/if}
      </form>
      {#if results || context}<button
          class="collection-return"
          onclick={() => {
            clearSearch();
            closeContext();
          }}
          title="Вернуться к сохранённой музыке"
          ><Icon name="heart" size={16} /><span>Коллекция</span></button
        >{/if}
      <div class="header-space" data-tauri-drag-region></div>
      <div class="window-controls">
        <button
          aria-label="Свернуть окно"
          onclick={() => api.window("minimize")}
          ><Icon name="minimize" size={16} /></button
        ><button
          aria-label="Развернуть окно"
          onclick={() => api.window("maximize")}
          ><Icon name="maximize" size={14} /></button
        ><button aria-label="Убрать в трей" onclick={() => api.window("close")}
          ><Icon name="close" size={17} /></button
        >
      </div>
    </header>
    <main
      class="library-screen"
      class:compact-collection={compactCollection}
      class:small-playlist-collection={!results &&
        library.playlists.length <= 4}
      class:few-playlists={!results && library.playlists.length <= 1}
      style:--saved-tracks-height={`${Math.max(3, library.tracks.length) * 56 + 28}px`}
    >
      {#if searchError}<div class="inline-error" role="alert">
          <span>{searchError}</span><button
            onclick={() => {
              searching = true;
              void runSearch(++searchRevision);
            }}>Повторить</button
          >
        </div>{/if}
      <section
        class="artists-collection"
        class:empty={artists.length === 0}
        aria-label="Артисты"
      >
        <div class="collection-heading">
          <h2>Артисты <small>{artists.length || ""}</small></h2>
          {#if pageErrors.artists}<button
              class="pagination-retry"
              title={pageErrors.artists}
              onclick={() => retryPage("artists")}>Повторить загрузку</button
            >{/if}
        </div>
        <div class="artist-strip">
          {#each artists as artist (artist.id)}
            <div
              class="artist-item"
              data-artist-id={artist.id}
              class:selected={context?.kind === "artist" &&
                context.id === artist.id}
            >
              <button
                class="artist-main"
                title={artist.name}
                onclick={() => openArtist(artist)}
                ><div class="artist-portrait">
                  <Artwork src={artist.artwork} />
                </div>
                <span>{artist.name}</span></button
              >
              <button
                class="artist-save"
                class:active={artistIds.has(artist.id)}
                aria-pressed={artistIds.has(artist.id)}
                title={artistIds.has(artist.id)
                  ? "Убрать из коллекции"
                  : "Сохранить артиста"}
                aria-label={artistIds.has(artist.id)
                  ? `Убрать ${artist.name} из коллекции`
                  : `Сохранить ${artist.name}`}
                onclick={() => saveArtist(artist)}
                ><Icon
                  name={artistIds.has(artist.id) ? "heart" : "plus"}
                  size={14}
                /></button
              >
            </div>
          {:else}<button
              class="artist-empty"
              onclick={() => searchInput.focus()}
              ><Icon name="plus" size={20} /><span
                >{results ? "Артисты не найдены" : "Найти артиста"}</span
              ></button
            >{/each}
          <div
            class="load-marker horizontal"
            use:paginate={{
              enabled:
                !!results?.next.artists &&
                !searching &&
                !pageLoading.artists &&
                !pageErrors.artists,
              load: () => more("artists"),
              horizontal: true,
            }}
          >
            {#if pageLoading.artists}<span class="spinner"></span>{/if}
          </div>
        </div>
      </section>
      <div class="collection-grid">
        <section class="playlist-collection" aria-label="Плейлисты">
          <div class="collection-heading">
            <h2>Плейлисты <small>{playlists.length || ""}</small></h2>
          </div>
          <div class="playlist-scroll">
            <div class="playlist-grid">
              {#each playlists as playlist (playlist.id)}
                <article
                  class="playlist-tile"
                  data-playlist-id={playlist.id}
                  class:selected={context?.kind === "playlist" &&
                    context.id === playlist.id}
                >
                  <button
                    class="playlist-open"
                    onclick={() => openPlaylist(playlist)}
                    title={playlist.title}
                    ><div class="playlist-image">
                      <Artwork src={playlist.artwork} />
                    </div>
                    <strong>{playlist.title}</strong><span
                      >{countLabel(playlist.trackCount)}</span
                    ></button
                  >
                  <div class="playlist-tile-actions">
                    <button
                      class="tile-save icon-button"
                      class:active={playlistIds.has(playlist.id)}
                      aria-label={playlistIds.has(playlist.id)
                        ? `Убрать ${playlist.title} из коллекции`
                        : `Сохранить ${playlist.title}`}
                      title={playlistIds.has(playlist.id)
                        ? "Убрать из коллекции"
                        : "В коллекцию"}
                      onclick={() => savePlaylist(playlist)}
                      ><Icon name="heart" size={16} /></button
                    ><button
                      class="tile-play icon-button"
                      aria-label={`Слушать ${playlist.title}`}
                      title="Слушать плейлист"
                      onclick={() => playPlaylist(playlist)}
                      ><Icon name="play" size={17} /></button
                    >
                  </div>
                </article>
              {:else}<div class="collection-empty">
                  <Icon name="queue" size={27} />
                  <p>
                    {results
                      ? "Плейлисты не найдены"
                      : "Сохраняй плейлисты из поиска — они появятся здесь."}
                  </p>
                  {#if !results}<button
                      class="quiet-button"
                      onclick={() => searchInput.focus()}
                      >Найти плейлист <Icon name="arrow" size={15} /></button
                    >{/if}
                </div>{/each}
            </div>
            <div
              class="load-marker"
              class:loading={pageLoading.playlists}
              use:paginate={{
                enabled:
                  !!results?.next.playlists &&
                  !searching &&
                  !pageLoading.playlists &&
                  !pageErrors.playlists,
                load: () => more("playlists"),
              }}
            >
              {#if pageLoading.playlists}<span class="spinner"></span>{/if}
            </div>
            {#if pageErrors.playlists}<button
                class="pagination-retry"
                title={pageErrors.playlists}
                onclick={() => retryPage("playlists")}
                >Повторить загрузку</button
              >{/if}
          </div>
        </section>
        <section class="track-collection" aria-label="Список музыки">
          <div class="track-collection-heading" class:has-context={!!context}>
            {#if context}<button
                class="context-back icon-button"
                title="Вернуться к результатам"
                aria-label="Вернуться к результатам"
                onclick={closeContext}><Icon name="back" size={20} /></button
              >
              <div class="context-art">
                <Artwork src={context.artwork} />
              </div>{/if}
            <div class="track-heading-copy">
              <h2 title={context?.title}>
                {context?.title || (results ? "Треки" : "Любимые треки")}
              </h2>
              <span
                >{catalogLoading
                  ? "Загружаю треки…"
                  : countLabel(tracks.length)}</span
              >
            </div>
            <div class="catalog-actions">
              {#if context}<button
                  class="icon-button"
                  class:active={context.kind === "artist"
                    ? artistIds.has(context.id)
                    : playlistIds.has(context.id)}
                  aria-label="Сохранить в коллекцию"
                  title="Сохранить в коллекцию"
                  onclick={saveContext}><Icon name="heart" size={18} /></button
                >{/if}
              {#if tracks.length}<button
                  class="icon-button"
                  title="Добавить всё в очередь"
                  aria-label="Добавить все треки в очередь"
                  onclick={() => {
                    void action("enqueue", { tracks, next: false });
                    toast("Треки добавлены в очередь");
                  }}><Icon name="plus" size={19} /></button
                ><button
                  class="catalog-play"
                  disabled={catalogLoading}
                  onclick={() =>
                    action("play", {
                      tracks,
                      index: Math.max(
                        0,
                        tracks.findIndex((t) => t.available),
                      ),
                    })}
                  ><Icon name="play" size={15} /><span>Слушать</span></button
                >{/if}
            </div>
          </div>
          {#if catalogError}<div class="inline-error" role="alert">
              {catalogError}
            </div>{/if}
          {#if tracks.length}<TrackList
              {tracks}
              currentId={current?.id}
              playing={state.playing}
              {liked}
              onplay={play}
              onlike={like}
              onqueue={addToQueue}
              onartist={trackArtist}
              hasMore={!context &&
                !!results?.next.tracks &&
                !searching &&
                !pageErrors.tracks}
              loadingMore={pageLoading.tracks}
              onmore={() => more("tracks")}
            />{#if !context && pageErrors.tracks}<button
                class="pagination-retry"
                title={pageErrors.tracks}
                onclick={() => retryPage("tracks")}>Повторить загрузку</button
              >{/if}
          {:else if searching || catalogLoading}<div
              class="track-skeletons"
              aria-label="Загрузка треков"
            >
              {#each Array(8) as _}<div>
                  <i></i><span></span><small></small>
                </div>{/each}
            </div>
          {:else}<div class="collection-empty tracks-empty">
              <Icon name={results || context ? "search" : "heart"} size={30} />
              <h3>
                {results || context
                  ? "Треки не найдены"
                  : "Любимая музыка будет здесь"}
              </h3>
              <p>
                {results || context
                  ? "Попробуй изменить запрос."
                  : "Нажми на сердце рядом с треком, чтобы сохранить его."}
              </p>
              <button class="quiet-button" onclick={() => searchInput.focus()}
                >Открыть поиск <Icon name="arrow" size={15} /></button
              >
            </div>{/if}
        </section>
      </div>
    </main>
  {/if}
  {#if current}<Player
      {state}
      {mode}
      palette={playerPalette}
      {motion}
      {visualizer}
      {bass}
      onbass={toggleBass}
      {panel}
      liked={current ? liked.has(current.id) : false}
      onaction={action}
      onmode={changeMode}
      onpanel={togglePanel}
      onlike={like}
      onvisualizer={toggleVisualizer}
    />{/if}
  {#if panel}<div class="panel-scrim" transition:fade={{ duration: motion }}>
      <button
        class="scrim-dismiss"
        aria-label="Закрыть панель"
        onclick={() => (panel = null)}
      ></button>
      <div class="floating-panel" transition:fly={{ y: 16, duration: motion }}>
        {#if panel === "queue"}<Queue
            {state}
            onaction={action}
            onclose={() => (panel = null)}
          />{:else}<Equalizer
            bands={state.eq}
            enabled={state.eqEnabled}
            onchange={setEq}
            onclose={() => (panel = null)}
          />{/if}
      </div>
    </div>{/if}
  {#if state.error}<div class="playback-error" role="alert">
      <span>{state.error}</span><button onclick={() => action("toggle")}
        >Повторить</button
      ><button
        aria-label="Закрыть сообщение"
        onclick={() => action("dismiss_error")}
        ><Icon name="close" size={14} /></button
      >
    </div>{/if}
  {#if notice}<div
      class="toast"
      role="status"
      transition:fly={{ y: 10, duration: motion }}
    >
      {notice}
    </div>{/if}
</div>
