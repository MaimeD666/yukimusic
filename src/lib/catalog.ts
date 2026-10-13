import type { Track } from "./types";

export type View = "tracks" | "playlists" | "artists" | "search" | "settings";
export type Sort = "recent" | "title" | "artist" | "duration";
export type Theme = "dark" | "light";

export function matches(text: string, query: string): boolean {
  return text.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase());
}

/** Sorting never changes the library's saved order or the playback queue. */
export function selectTracks(
  tracks: Track[],
  query: string,
  sort: Sort,
): Track[] {
  const selected = tracks.filter((track) =>
    matches(`${track.title} ${track.artist}`, query),
  );
  if (sort === "title")
    selected.sort((a, b) => a.title.localeCompare(b.title, "ru"));
  if (sort === "artist")
    selected.sort(
      (a, b) =>
        a.artist.localeCompare(b.artist, "ru") ||
        a.title.localeCompare(b.title, "ru"),
    );
  if (sort === "duration") selected.sort((a, b) => a.duration - b.duration);
  return selected;
}

export function collectionDuration(tracks: Track[]): string {
  const minutes = Math.round(
    tracks.reduce((sum, track) => sum + track.duration, 0) / 60,
  );
  return minutes >= 60
    ? `${Math.floor(minutes / 60)} ч ${minutes % 60} мин`
    : `${minutes} мин`;
}

export function readTheme(): Theme {
  try {
    return localStorage.getItem("yukicloud.theme") === "light"
      ? "light"
      : "dark";
  } catch {
    return "dark";
  }
}

let themeTimer: ReturnType<typeof setTimeout>;
export function applyTheme(theme: Theme) {
  if (
    document.documentElement.dataset.theme &&
    document.documentElement.dataset.theme !== theme
  ) {
    clearTimeout(themeTimer);
    document.documentElement.classList.add("theme-changing");
    themeTimer = setTimeout(
      () => document.documentElement.classList.remove("theme-changing"),
      320,
    );
  }
  document.documentElement.dataset.theme = theme;
  try {
    localStorage.setItem("yukicloud.theme", theme);
  } catch {
    /* The theme still works without storage. */
  }
}
