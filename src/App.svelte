<script lang="ts">
  import { onMount, tick } from "svelte";
  import { fade } from "svelte/transition";
  import {
    panelSlide,
    reveal,
    selectionIndicator,
    softReveal,
    easeOut,
  } from "./lib/motion";
  import { api, native } from "./lib/bridge";
  import { palette } from "./lib/artwork";
  import { neutralPalette } from "./lib/palette";
  import {
    applyTheme,
    readTheme,
    matches,
    selectTracks,
    collectionDuration,
    type View,
    type Sort,
    type Theme,
  } from "./lib/catalog";
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
  import Sidebar from "./components/Sidebar.svelte";
  import CollectionBrowser from "./components/CollectionBrowser.svelte";
  import EmptyState from "./components/EmptyState.svelte";
  import Preferences from "./components/Preferences.svelte";
  import ConfirmRemoval from "./components/ConfirmRemoval.svelte";
  import Tooltip from "./components/Tooltip.svelte";
  import type { Menu, MenuItem } from "./lib/menu";
  let menu: Menu | null = null;
  let removal: {
    kind: "artist" | "playlist";
    id: number;
    name: string;
  } | null = null;

  let query = "";
  let searchInput: HTMLInputElement;
  let library: Library = { artists: [], playlists: [], tracks: [] };
  let state: Playback = defaultPlayback();
  let results: SearchResults | null = null;
  let searching = false;
  type CatalogContext = {
    kind: "artist" | "playlist";
    id: number;
    title: string;
    artwork: string | null;
    artworkVerified?: boolean;
  };
  let context: CatalogContext | null = null;
  let contextTracks: Track[] = [];
  let catalogLoading = false;
  let searchError = "";
  let catalogError = "";
  let notice = "";
  let panel: "queue" | "eq" | null = null;
  let mode: "dock" | "immersive" | "mini" = "dock";
  let changingMode = false;
  let theme: Theme = readTheme();
  $: applyTheme(theme);
  let view: View = "tracks";
  let searchKind: ResultKind = "tracks";
  let filter = "";
  let sort: Sort = "recent";
  const viewNames: Record<View, string> = {
    tracks: "Любимые треки",
    playlists: "Плейлисты",
    artists: "Артисты",
    search: "Поиск",
    settings: "Настройки",
  };
  type Location = {
    view: View;
    context: CatalogContext | null;
    contextTracks: Track[];
    query: string;
    results: SearchResults | null;
    searchKind: ResultKind;
    filter: string;
    sort: Sort;
    loading: boolean;
    scroll: number;
  };
  let backStack: Location[] = [];
  let forwardStack: Location[] = [];
  function location(): Location {
    return {
      view,
      context,
      contextTracks,
      query,
      results,
      searchKind,
      filter,
      sort,
      loading: catalogLoading,
      scroll:
        document.querySelector(".track-scroll, .collection-scroll")
          ?.scrollTop || 0,
    };
  }
  function rememberLocation() {
    backStack = [...backStack.slice(-29), location()];
    forwardStack = [];
  }
  function cancelLoading() {
    ++catalogRevision;
    ++searchRevision;
    clearTimeout(searchTimer);
    catalogLoading = false;
    searching = false;
    catalogError = "";
    searchError = "";
    pageLoading = { artists: false, playlists: false, tracks: false };
    pageErrors = { artists: "", playlists: "", tracks: "" };
    emptyPages = { artists: 0, playlists: 0, tracks: 0 };
    pendingPages.clear();
    loadedPages.clear();
  }
  function navigate(next: View, resume = true) {
    if (view !== next || context) {
      rememberLocation();
      cancelLoading();
      view = next;
      context = null;
      filter = "";
      sort = "recent";
      menu = null;
    }
    if (next === "search") {
      void tick().then(() => searchInput?.focus());
      if (resume) resumeSearch();
    }
  }
  function resumeSearch() {
    if (
      view === "search" &&
      !context &&
      query.trim() &&
      !results &&
      !searching
    ) {
      searching = true;
      void runSearch(++searchRevision);
    }
  }
  async function restoreLocation(next: Location) {
    cancelLoading();
    ({
      view,
      context,
      contextTracks,
      query,
      results,
      searchKind,
      filter,
      sort,
    } = next);
    menu = null;
    if (next.loading && context) void retryCatalog();
    resumeSearch();
    await tick();
    const scroll = document.querySelector(".track-scroll, .collection-scroll");
    if (scroll) scroll.scrollTop = next.scroll;
  }
  function goBack() {
    const next = backStack.at(-1);
    if (!next) return;
    forwardStack = [...forwardStack, location()];
    backStack = backStack.slice(0, -1);
    void restoreLocation(next);
  }
  function goForward() {
    const next = forwardStack.at(-1);
    if (!next) return;
    backStack = [...backStack, location()];
    forwardStack = forwardStack.slice(0, -1);
    void restoreLocation(next);
  }
  async function focusSearch() {
    if (mode !== "dock") await changeMode("dock");
    navigate("search");
    await tick();
    searchInput?.focus();
  }
  function playAll(shuffle = false) {
    if (!tracks.some((track) => track.available)) return;
    void (async () => {
      if (state.shuffle !== shuffle) await action("shuffle");
      await action("play", {
        tracks,
        index: Math.max(
          0,
          tracks.findIndex((track) => track.available),
        ),
      });
    })();
  }
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
  $: rememberView(
    query,
    results,
    context,
    contextTracks,
    ready,
    view,
    searchKind,
    filter,
    sort,
  );
  function rememberView(
    q: typeof query,
    r: typeof results,
    c: typeof context,
    t: Track[],
    loaded: boolean,
    activeView: View,
    resultKind: ResultKind,
    localFilter: string,
    localSort: Sort,
  ) {
    if (!loaded) return;
    clearTimeout(viewTimer);
    viewTimer = setTimeout(() => {
      try {
        localStorage.setItem(
          "yukicloud.view",
          JSON.stringify({
            query: q,
            results: r,
            context: c,
            tracks: t,
            view: activeView,
            searchKind: resultKind,
            filter: localFilter,
            sort: localSort,
          }),
        );
      } catch {
        /* The collection is stored separately by Rust. */
      }
    }, 400);
  }

  $: searchingView = view === "search";
  $: artists = (
    searchingView ? (results?.artists ?? []) : library.artists
  ).filter((a) => matches(a.name, filter));
  $: playlists = (
    searchingView ? (results?.playlists ?? []) : library.playlists
  ).filter((p) => matches(`${p.title} ${p.artist}`, filter));
  $: sourceTracks = context
    ? contextTracks
    : searchingView
      ? (results?.tracks ?? [])
      : library.tracks;
  $: tracks = selectTracks(sourceTracks, filter, sort);
  $: activeKind = searchingView
    ? searchKind
    : view === "artists"
      ? "artists"
      : view === "playlists"
        ? "playlists"
        : "tracks";
  $: isTrackView = !!context || activeKind === "tracks";
  $: screenTitle = context?.title || viewNames[view];
  $: itemCount =
    context || isTrackView
      ? tracks.length
      : activeKind === "artists"
        ? artists.length
        : playlists.length;
  $: liked = new Set(library.tracks.map((t) => t.id));
  $: artistIds = new Set(library.artists.map((a) => a.id));
  $: playlistIds = new Set(library.playlists.map((p) => p.id));
  $: current = state.queue.find((e) => e.key === state.currentKey)?.track;
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
    if (artistIds.has(artist.id)) {
      removal = { kind: "artist", id: artist.id, name: artist.name };
      return;
    }
    library = {
      ...library,
      artists: [...library.artists, artist],
    };
    persist();
  }
  function savePlaylist(playlist: Playlist) {
    if (playlistIds.has(playlist.id)) {
      removal = { kind: "playlist", id: playlist.id, name: playlist.title };
      return;
    }
    library = {
      ...library,
      playlists: [...library.playlists, playlist],
    };
    persist();
  }
  function confirmRemoval() {
    if (!removal) return;
    const { kind, id } = removal;
    library =
      kind === "artist"
        ? {
            ...library,
            artists: library.artists.filter((artist) => artist.id !== id),
          }
        : {
            ...library,
            playlists: library.playlists.filter(
              (playlist) => playlist.id !== id,
            ),
          };
    removal = null;
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
    if (
      ["play", "toggle", "jump", "next", "previous", "clear"].includes(name)
    ) {
      ++playbackRevision;
      pendingCollection = null;
    }
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
    if (view !== "search" || context) navigate("search", false);
    filter = "";
    results = null;
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
  async function openArtist(artist: Artist, remember = true) {
    if (remember) rememberLocation();
    if (view !== "search") view = "artists";
    filter = "";
    sort = "recent";
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
  async function openPlaylist(playlist: Playlist, remember = true) {
    if (remember) rememberLocation();
    if (view !== "search") view = "playlists";
    filter = "";
    sort = "recent";
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
  let playbackRevision = 0;
  let pendingCollection: string | null = null;
  // Playback owns its request, independently of navigation and local filters.
  async function playCollection(kind: "artist" | "playlist", id: number) {
    const key = `${kind}:${id}`;
    if (pendingCollection === key) return;
    const revision = ++playbackRevision;
    pendingCollection = key;
    const isCurrent = () => revision === playbackRevision && !disposed;
    try {
      let queue: Track[] = [];
      if (
        context?.kind === kind &&
        context.id === id &&
        !catalogLoading &&
        !catalogError
      ) {
        queue = contextTracks;
      } else if (kind === "playlist") {
        queue = (await api.playlistTracks(id)).tracks;
      } else {
        let next: string | null = null;
        const seenPages = new Set<string>();
        const seenTracks = new Set<number>();
        do {
          const page = await api.artistTracks(id, next);
          if (!isCurrent()) return;
          queue.push(
            ...page.tracks.filter((track) => {
              if (seenTracks.has(track.id)) return false;
              seenTracks.add(track.id);
              return true;
            }),
          );
          next = page.next;
          if (next && seenPages.has(next))
            throw new Error("SoundCloud повторил страницу. Попробуй ещё раз.");
          if (next) seenPages.add(next);
        } while (next);
      }
      if (!isCurrent()) return;
      const index = queue.findIndex((track) => track.available);
      if (index < 0) {
        toast("Нет доступных для воспроизведения треков");
        return;
      }
      await action("play", { tracks: queue, index });
    } catch (error) {
      if (isCurrent()) toast(errorMessage(error));
    } finally {
      if (revision === playbackRevision) pendingCollection = null;
    }
  }
  function playPlaylist(playlist: Playlist) {
    return playCollection("playlist", playlist.id);
  }
  function playArtist(artist: Artist) {
    return playCollection("artist", artist.id);
  }
  async function retryCatalog() {
    if (!context) return;
    const entity = {
      id: context.id,
      artwork: context.artwork,
      trackCount: contextTracks.length,
      permalink: "",
    };
    if (context.kind === "artist")
      await openArtist(
        {
          ...entity,
          name: context.title,
          artworkVerified: context.artworkVerified,
        },
        false,
      );
    else
      await openPlaylist(
        { ...entity, title: context.title, artist: "" },
        false,
      );
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
    if (backStack.length) goBack();
    else navigate(view);
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
    if (menu || removal) return;
    const typing = (event.target as HTMLElement)?.matches(
      "input, textarea, select, [contenteditable]",
    );
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
      event.preventDefault();
      void focusSearch();
      return;
    }
    if (event.altKey && event.key === "ArrowLeft") {
      event.preventDefault();
      goBack();
      return;
    }
    if (event.altKey && event.key === "ArrowRight") {
      event.preventDefault();
      goForward();
      return;
    }
    if (event.key === "Escape") {
      if (panel) panel = null;
      else if (mode !== "dock") void changeMode("dock");
      else if (context) closeContext();
      else if (filter) filter = "";
      else if (view === "search" && query) clearSearch();
      return;
    }
    if (typing || event.ctrlKey || event.metaKey || event.altKey) return;
    if (
      event.code === "Space" &&
      !(event.target as HTMLElement)?.closest(
        "button, select, a, [role='switch']",
      )
    ) {
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
        (visualizer && mode !== "mini") || (mode === "immersive" && bass),
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
    const artist = [...artists, ...library.artists].find(
      (a) => a.id === artistId,
    );
    const playlistId = Number(
      target.closest<HTMLElement>("[data-playlist-id]")?.dataset.playlistId,
    );
    const playlist = [...playlists, ...library.playlists].find(
      (p) => p.id === playlistId,
    );
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
          run: () => playArtist(artist),
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
            await focusSearch();
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
    const motionPreference = matchMedia("(prefers-reduced-motion: reduce)");
    const updateMotion = () => {
      motion = motionPreference.matches ? 0 : 240;
    };
    updateMotion();
    motionPreference.addEventListener("change", updateMotion);
    visualizer = localStorage.getItem("yukicloud.visualizer") !== "false";
    bass = localStorage.getItem("yukicloud.bass") === "true";
    try {
      const saved = JSON.parse(
        localStorage.getItem("yukicloud.view") || "null",
      );
      if (saved) {
        query = saved.query || "";
        results = saved.results || null;
        context = saved.context || null;
        contextTracks = saved.tracks || [];
        view =
          saved.view in viewNames
            ? saved.view
            : saved.query
              ? "search"
              : "tracks";
        searchKind = ["tracks", "artists", "playlists"].includes(
          saved.searchKind,
        )
          ? saved.searchKind
          : "tracks";
        filter = typeof saved.filter === "string" ? saved.filter : "";
        sort = ["recent", "title", "artist", "duration"].includes(saved.sort)
          ? saved.sort
          : "recent";
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
      motionPreference.removeEventListener("change", updateMotion);
    };
  });
</script>

<svelte:window onkeydown={keydown} oncontextmenu={contextmenu} />
<Tooltip />
{#if removal}<ConfirmRemoval
    name={removal.name}
    kind={removal.kind}
    oncancel={() => (removal = null)}
    onconfirm={confirmRemoval}
  />{/if}
{#if menu}{#key menu}<ContextMenu
      {menu}
      onclose={() => (menu = null)}
      onerror={(e) => toast(errorMessage(e))}
    />{/key}{/if}

<div
  class="application"
  class:mini={mode === "mini"}
  class:with-panel={!!panel}
>
  {#if mode === "dock"}
    <div class="app-body" use:reveal={{ duration: 280, y: 0 }}>
      <Sidebar
        {view}
        {library}
        {theme}
        selectedPlaylist={context?.kind === "playlist" ? context.id : undefined}
        onnavigate={navigate}
        onplaylist={openPlaylist}
        ontheme={() => (theme = theme === "dark" ? "light" : "dark")}
      />
      <div class="workspace">
        <header class="app-header" data-tauri-drag-region>
          <div class="history-controls">
            <button
              class="icon-button"
              data-tooltip="Назад · Alt ←"
              aria-label="Назад"
              disabled={!backStack.length}
              onclick={goBack}><Icon name="back" size={18} /></button
            >
            <button
              class="icon-button"
              data-tooltip="Вперёд · Alt →"
              aria-label="Вперёд"
              disabled={!forwardStack.length}
              onclick={goForward}><Icon name="arrow" size={18} /></button
            >
          </div>
          <form
            class="search"
            onsubmit={(event) => {
              event.preventDefault();
              if (!query.trim()) return;
              onSearch();
              clearTimeout(searchTimer);
              void runSearch(searchRevision);
            }}
          >
            <Icon name="search" size={18} />
            <input
              bind:this={searchInput}
              bind:value={query}
              oninput={onSearch}
              placeholder="Музыка, артисты или ссылка"
              aria-label="Поиск музыки"
              spellcheck="false"
              autocomplete="off"
            />
            {#if searching}<span class="spinner" aria-label="Поиск"
              ></span>{:else if query}<button
                type="button"
                class="icon-button"
                aria-label="Очистить поиск"
                onclick={clearSearch}><Icon name="close" size={15} /></button
              >{:else}<kbd>Ctrl K</kbd>{/if}
          </form>
          <div class="header-space" data-tauri-drag-region></div>
          <div class="window-controls">
            <button
              aria-label="Свернуть окно"
              data-tooltip="Свернуть"
              onclick={() => api.window("minimize")}
              ><Icon name="minimize" size={16} /></button
            >
            <button
              aria-label="Развернуть окно"
              data-tooltip="Развернуть"
              onclick={() => api.window("maximize")}
              ><Icon name="maximize" size={14} /></button
            >
            <button
              aria-label="Убрать в трей"
              data-tooltip="Убрать в трей"
              onclick={() => api.window("close")}
              ><Icon name="close" size={17} /></button
            >
          </div>
        </header>
        <div class="work-area">
          <main class="content-surface" aria-label={screenTitle}>
            <div
              class="view-content"
              use:reveal={{
                key: `${view}-${context?.kind || ""}-${context?.id || ""}`,
                duration: 280,
              }}
            >
              {#if !searchingView || context}<header
                  class="view-header"
                  class:detail-header={!!context}
                >
                  {#if context}<div
                      class="detail-art"
                      class:round={context.kind === "artist"}
                    >
                      <Artwork src={context.artwork} />
                    </div>
                  {:else if view !== "settings" && view !== "search"}<div
                      class="section-symbol"
                      class:heart-symbol={view === "tracks"}
                    >
                      <Icon
                        name={view === "tracks"
                          ? "heart"
                          : view === "artists"
                            ? "artist"
                            : "library"}
                        size={25}
                      />
                    </div>{/if}
                  <div class="view-heading">
                    {#if context}<span class="view-kicker"
                        >{context.kind === "artist"
                          ? "Артист"
                          : "Плейлист"}</span
                      >{/if}
                    <h1 data-tooltip={screenTitle}>{screenTitle}</h1>
                    {#if view !== "settings" && (!searchingView || context || query.trim())}<p
                      >
                        {#if isTrackView}{countLabel(
                            sourceTracks.length,
                          )}{#if sourceTracks.length}<span class="meta-dot"
                              >·</span
                            >{collectionDuration(
                              sourceTracks,
                            )}{/if}{#if catalogLoading}<span class="meta-dot"
                              >·</span
                            >Загружаю…{/if}
                        {:else}{activeKind === "artists"
                            ? library.artists.length
                            : library.playlists.length} в коллекции{/if}
                      </p>{/if}
                  </div>
                  <div class="view-actions">
                    {#if context}<button
                        class="icon-button save-context"
                        class:active={context.kind === "artist"
                          ? artistIds.has(context.id)
                          : playlistIds.has(context.id)}
                        aria-label="Сохранить в коллекцию"
                        data-tooltip="Сохранить в коллекцию"
                        onclick={saveContext}
                        ><Icon name="heart" size={19} /></button
                      >{/if}
                    {#if isTrackView && tracks.length && view !== "settings" && (!searchingView || query)}
                      <button
                        class="icon-button shuffle-all"
                        data-tooltip="Слушать вперемешку"
                        aria-label="Слушать вперемешку"
                        onclick={() => playAll(true)}
                        ><Icon name="shuffle" size={20} /></button
                      >
                      <button
                        class="primary-button listen-all"
                        disabled={!tracks.some((track) => track.available)}
                        onclick={() => playAll()}
                        ><Icon name="play" size={16} /><span>Слушать</span
                        ></button
                      >
                    {/if}
                  </div>
                </header>{/if}

              {#if view === "settings"}
                <Preferences
                  {theme}
                  {visualizer}
                  {bass}
                  ontheme={(next) => (theme = next)}
                  onvisualizer={toggleVisualizer}
                  onbass={toggleBass}
                  onequalizer={() => togglePanel("eq")}
                />
              {:else}
                {#if searchingView && !context}
                  <div
                    class="search-tabs"
                    use:selectionIndicator={searchKind}
                    role="group"
                    aria-label="Тип результатов"
                  >
                    <i class="selection-indicator" aria-hidden="true"></i>
                    {#each [{ kind: "tracks", label: "Треки" }, { kind: "artists", label: "Артисты" }, { kind: "playlists", label: "Плейлисты" }] as tab}
                      <button
                        class:active={searchKind === tab.kind}
                        aria-pressed={searchKind === tab.kind}
                        onclick={() => {
                          searchKind = tab.kind as ResultKind;
                          filter = "";
                        }}
                      >
                        {tab.label}{#if results}<span
                            >{results[tab.kind as ResultKind].length}{results
                              .next[tab.kind as ResultKind]
                              ? "+"
                              : ""}</span
                          >{/if}
                      </button>
                    {/each}
                  </div>
                {/if}
                {#if searchError && searchingView && !context}<div
                    class="inline-error"
                    role="alert"
                    transition:softReveal={{ duration: 180 }}
                  >
                    <span>{searchError}</span><button
                      onclick={() => {
                        searching = true;
                        void runSearch(++searchRevision);
                      }}>Повторить</button
                    >
                  </div>{/if}
                {#if catalogError && context}<div
                    class="inline-error"
                    role="alert"
                    transition:softReveal={{ duration: 180 }}
                  >
                    <span>{catalogError}</span><button onclick={retryCatalog}
                      >Повторить</button
                    >
                  </div>{/if}

                {#if searchingView && !query.trim() && !context}
                  <div class="search-start" use:reveal>
                    {#if library.artists.length}<div class="search-suggestions">
                        <span>Из твоей коллекции</span>
                        <div>
                          {#each library.artists.slice(0, 5) as artist}<button
                              onclick={() => openArtist(artist)}
                              ><span><Artwork src={artist.artwork} /></span
                              >{artist.name}</button
                            >{/each}
                        </div>
                      </div>{/if}
                  </div>
                {:else}
                  {#if (sourceTracks.length && isTrackView) || (!isTrackView && (activeKind === "artists" ? (searchingView ? results?.artists.length : library.artists.length) : searchingView ? results?.playlists.length : library.playlists.length))}
                    <div class="list-toolbar">
                      <label class="local-search"
                        ><Icon name="filter" size={17} /><input
                          bind:value={filter}
                          placeholder={context
                            ? "Найти в этом списке"
                            : searchingView
                              ? "Фильтр результатов"
                              : "Найти в коллекции"}
                          aria-label="Фильтр коллекции"
                        />{#if filter}<button
                            class="icon-button"
                            onclick={() => (filter = "")}
                            aria-label="Сбросить фильтр"
                            ><Icon name="close" size={14} /></button
                          >{/if}</label
                      >
                      {#if isTrackView}<label class="sort-control"
                          ><span class="sr-only">Сортировка треков</span><select
                            bind:value={sort}
                            aria-label="Сортировка треков"
                            ><option value="recent"
                              >{context || searchingView
                                ? "По порядку"
                                : "Сначала новые"}</option
                            ><option value="title">По названию</option><option
                              value="artist">По артисту</option
                            ><option value="duration">По длительности</option
                            ></select
                          ><Icon name="down" size={14} /></label
                        >
                        <button
                          class="icon-button add-all"
                          data-tooltip="Добавить всё в очередь"
                          aria-label="Добавить все треки в очередь"
                          disabled={!tracks.length}
                          onclick={() => {
                            void action("enqueue", { tracks, next: false });
                            toast("Треки добавлены в очередь");
                          }}><Icon name="plus" size={19} /></button
                        >
                      {/if}
                    </div>
                  {/if}
                  {#if (searching && !results) || (catalogLoading && !contextTracks.length)}
                    <div
                      class="track-skeletons"
                      role="status"
                      aria-label="Загрузка музыки"
                    >
                      {#each Array(10) as _}<div>
                          <i></i><span><b></b><small></small></span><em></em>
                        </div>{/each}
                    </div>
                  {:else if itemCount}
                    {#key `${view}-${context?.kind || ""}-${context?.id || ""}-${activeKind}`}
                      {#if isTrackView}<TrackList
                          {tracks}
                          currentId={current?.id}
                          playing={state.playing}
                          {liked}
                          onplay={play}
                          onlike={like}
                          onqueue={addToQueue}
                          onartist={trackArtist}
                          hasMore={searchingView &&
                            !context &&
                            !!results?.next.tracks &&
                            !searching &&
                            !pageErrors.tracks}
                          loadingMore={pageLoading.tracks}
                          onmore={() => more("tracks")}
                          resetKey={`${filter}-${sort}`}
                        />
                      {:else}<CollectionBrowser
                          kind={activeKind === "artists"
                            ? "artists"
                            : "playlists"}
                          {artists}
                          {playlists}
                          saved={activeKind === "artists"
                            ? artistIds
                            : playlistIds}
                          onartist={openArtist}
                          onplaylist={openPlaylist}
                          onsaveartist={saveArtist}
                          onsaveplaylist={savePlaylist}
                          onplayartist={playArtist}
                          onplayplaylist={playPlaylist}
                          {pendingCollection}
                          hasMore={searchingView &&
                            !!results?.next[activeKind] &&
                            !pageErrors[activeKind]}
                          loading={pageLoading[activeKind]}
                          onmore={() => more(activeKind)}
                        />{/if}
                    {/key}
                  {:else if filter}<EmptyState
                      title="В этом списке ничего не нашлось"
                      description="Попробуй другое название или убери фильтр."
                      action="Сбросить фильтр"
                      onaction={() => (filter = "")}
                    />
                  {:else if searchingView || context}<EmptyState
                      icon="search"
                      title={searchError || catalogError
                        ? "Не удалось загрузить музыку"
                        : "Пока ничего не нашлось"}
                      description={context
                        ? "Попробуй загрузить список ещё раз."
                        : "Попробуй другое название или ссылку."}
                      action={context ? "Повторить" : "Изменить запрос"}
                      onaction={() =>
                        context ? void retryCatalog() : searchInput?.focus()}
                    />
                  {:else}<EmptyState
                      icon={view === "tracks"
                        ? "heart"
                        : view === "artists"
                          ? "artist"
                          : "library"}
                      title={view === "tracks"
                        ? "Нет любимых треков"
                        : view === "artists"
                          ? "Нет сохранённых артистов"
                          : "Нет сохранённых плейлистов"}
                      description={view === "tracks"
                        ? "Добавляй треки кнопкой с сердечком."
                        : view === "artists"
                          ? "Найди артиста и добавь его в коллекцию."
                          : "Найди плейлист и добавь его в коллекцию."}
                      action="Найти музыку"
                      onaction={focusSearch}
                    />{/if}
                  {#if searchingView && !context && pageErrors[activeKind]}<button
                      class="pagination-retry"
                      data-tooltip={pageErrors[activeKind]}
                      onclick={() => retryPage(activeKind)}
                      >Не удалось загрузить ещё. Повторить</button
                    >{/if}
                {/if}
              {/if}
            </div>
          </main>
          {#if panel}<aside
              class="side-panel"
              class:eq-side={panel === "eq"}
              transition:panelSlide
              aria-label={panel === "queue" ? "Очередь" : "Настройка звука"}
            >
              <div
                class="panel-content"
                use:reveal={{ key: panel, duration: 220, y: 5 }}
              >
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
            </aside>{/if}
        </div>
      </div>
    </div>
  {/if}
  <Player
    {state}
    {theme}
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
    onartist={trackArtist}
  />
  {#if panel && mode !== "dock"}<div
      class="panel-scrim"
      transition:fade={{ duration: motion, easing: easeOut }}
    >
      <button
        class="scrim-dismiss"
        aria-label="Закрыть панель"
        onclick={() => (panel = null)}
      ></button>
      <div
        class="floating-panel"
        transition:softReveal={{ duration: 260, y: 12 }}
      >
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
  {#if state.error}<div
      class="playback-error"
      role="alert"
      transition:softReveal
    >
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
      transition:softReveal={{ y: 6, duration: 220, scale: 1 }}
    >
      <Icon name="check" size={16} />{notice}
    </div>{/if}
</div>
