import type { Playback, QueueEntry, Track } from "./types.ts";

let serial = 0;
export function entries(tracks: Track[]): QueueEntry[] {
  return tracks.map((track) => ({ key: `${Date.now()}-${++serial}`, track }));
}

export function moveEntry(queue: QueueEntry[], source: string, target: string) {
  const from = queue.findIndex((e) => e.key === source);
  const to = queue.findIndex((e) => e.key === target);
  if (from < 0 || to < 0 || from === to) return queue;
  const next = [...queue];
  next.splice(to, 0, next.splice(from, 1)[0]);
  return next;
}

export function enqueue(
  state: Playback,
  tracks: Track[],
  next: boolean,
): Playback {
  const queue = [...state.queue];
  const at = next
    ? Math.max(0, queue.findIndex((e) => e.key === state.currentKey) + 1)
    : queue.length;
  queue.splice(at, 0, ...entries(tracks));
  return { ...state, queue };
}

export function removeEntry(state: Playback, key: string): Playback {
  const index = state.queue.findIndex((e) => e.key === key);
  const queue = state.queue.filter((e) => e.key !== key);
  if (state.currentKey !== key) return { ...state, queue };
  const currentKey = queue[Math.min(index, queue.length - 1)]?.key ?? null;
  return {
    ...state,
    queue,
    currentKey,
    position: 0,
    playing: currentKey !== null && state.playing,
  };
}
