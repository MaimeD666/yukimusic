export interface Artist {
  id: number;
  name: string;
  artwork: string | null;
  trackCount: number;
  permalink: string;
  artworkVerified?: boolean;
}

export interface Track {
  id: number;
  title: string;
  artist: string;
  artistId: number;
  artwork: string | null;
  duration: number;
  permalink: string;
  available: boolean;
}

export interface Playlist {
  id: number;
  title: string;
  artist: string;
  artwork: string | null;
  trackCount: number;
  permalink: string;
}

export interface SearchResults {
  artists: Artist[];
  playlists: Playlist[];
  tracks: Track[];
  next: {
    artists: string | null;
    playlists: string | null;
    tracks: string | null;
  };
  errors: string[];
}

export interface TrackPage {
  tracks: Track[];
  next: string | null;
  total: number;
}
export interface QueueEntry {
  key: string;
  track: Track;
}
export type RepeatMode = "off" | "all" | "one";
export interface Playback {
  queue: QueueEntry[];
  currentKey: string | null;
  playing: boolean;
  loading: boolean;
  position: number;
  volume: number;
  muted: boolean;
  shuffle: boolean;
  repeat: RepeatMode;
  eq: number[];
  eqEnabled: boolean;
  error: string | null;
}

export interface Library {
  artists: Artist[];
  playlists: Playlist[];
  tracks: Track[];
}
export type Entity = Artist | Playlist | Track;
export const frequencies = [
  32, 64, 125, 250, 500, 1000, 2000, 4000, 8000, 16000,
];
export const defaultPlayback = (): Playback => ({
  queue: [],
  currentKey: null,
  playing: false,
  loading: false,
  position: 0,
  volume: 0.72,
  muted: false,
  shuffle: false,
  repeat: "off",
  eq: Array(10).fill(0),
  eqEnabled: false,
  error: null,
});
