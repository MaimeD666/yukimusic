import type {
  Artist,
  Library,
  Playback,
  Playlist,
  SearchResults,
  Track,
  TrackPage,
} from "./types";
import { defaultPlayback } from "./types";
import { enqueue, entries, moveEntry, removeEntry } from "./queue";

export const native =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
let preview = defaultPlayback();
let latest = defaultPlayback();
let previewLibrary: Library = { artists: [], playlists: [], tracks: [] };
const subscribers = new Set<(state: Playback) => void>();

async function call<T>(
  command: string,
  args: Record<string, unknown> = {},
): Promise<T> {
  if (!native)
    throw new Error("Поиск и звук доступны в настольном приложении.");
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(command, args);
}

export const api = {
  search: (query: string) => call<SearchResults>("search", { query }),
  searchMore: (kind: string, url: string) =>
    call<SearchResults>("search_more", { kind, url }),
  artistTracks: (id: number, next: string | null = null) =>
    call<TrackPage>("artist_tracks", { id, next }),
  artistProfile: (id: number) => call<Artist>("artist_profile", { id }),
  playlistTracks: (id: number) => call<TrackPage>("playlist_tracks", { id }),
  artwork: (url: string) => call<string>("artwork", { url }),
  resolve: (url: string) =>
    call<{ kind: string; entity: Artist | Playlist | Track }>("resolve", {
      url,
    }),
  async library(): Promise<Library> {
    if (native) return call<Library>("get_library");
    try {
      previewLibrary = JSON.parse(
        localStorage.getItem("yukicloud.library") || "{}",
      );
    } catch {
      /* empty library */
    }
    return {
      artists: previewLibrary.artists || [],
      playlists: previewLibrary.playlists || [],
      tracks: previewLibrary.tracks || [],
    };
  },
  async saveLibrary(library: Library) {
    if (native) return call<void>("save_library", { library });
    previewLibrary = library;
    localStorage.setItem("yukicloud.library", JSON.stringify(library));
  },
  async snapshot(): Promise<Playback> {
    latest = native ? await call<Playback>("snapshot") : preview;
    return latest;
  },
  async action(action: string, value: unknown = null) {
    if (native) return call<void>("player_action", { action, value });
    // The browser is a layout preview. Never simulate successful audio playback.
    if (["play", "toggle", "jump"].includes(action))
      throw new Error(
        "Открой yukicloud как настольное приложение, чтобы слушать музыку.",
      );
    if (action === "enqueue") {
      const v = value as { tracks: Track[]; next: boolean };
      preview = enqueue(preview, v.tracks, v.next);
    }
    if (action === "remove") preview = removeEntry(preview, value as string);
    if (action === "move") {
      const v = value as { from: string; to: string };
      preview.queue = moveEntry(preview.queue, v.from, v.to);
    }
    if (action === "clear")
      preview.queue = preview.queue.filter((e) => e.key === preview.currentKey);
    if (action === "volume") preview.volume = Number(value);
    if (action === "mute") preview.muted = !preview.muted;
    if (action === "shuffle") preview.shuffle = !preview.shuffle;
    if (action === "repeat") preview.repeat = value as Playback["repeat"];
    if (action === "eq") {
      const v = value as { bands: number[]; enabled: boolean };
      preview.eq = v.bands;
      preview.eqEnabled = v.enabled;
    }
    preview = { ...preview, queue: [...preview.queue] };
    subscribers.forEach((fn) => fn(preview));
  },
  async subscribe(
    fn: (state: Playback) => void,
    notice?: (message: string) => void,
  ) {
    if (native) {
      const { listen } = await import("@tauri-apps/api/event");
      const offState = await listen<Playback>("playback", (e) => {
        latest = e.payload;
        fn(latest);
      });
      const offTime = await listen<
        Pick<Playback, "position" | "playing" | "loading">
      >("timeline", (e) => {
        latest = { ...latest, ...e.payload };
        fn(latest);
      });
      const offNotice = await listen<string>("notice", (e) =>
        notice?.(e.payload),
      );
      return () => {
        offState();
        offTime();
        offNotice();
      };
    }
    subscribers.add(fn);
    return () => {
      subscribers.delete(fn);
    };
  },
  async window(action: string) {
    if (native) return call<void>("window_action", { action });
    if (action === "fullscreen") {
      if (document.fullscreenElement) await document.exitFullscreen();
      else await document.documentElement.requestFullscreen();
    }
  },
  async visualization(visible: boolean, enabled: boolean) {
    if (native) await call<void>("visualization", { visible, enabled });
  },
};

export function previewQueue(tracks: Track[]) {
  preview.queue = entries(tracks);
}
