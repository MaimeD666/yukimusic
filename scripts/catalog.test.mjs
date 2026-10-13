import assert from 'node:assert/strict';
import { selectTracks, matches, collectionDuration } from '../src/lib/catalog.ts';
const tracks = [
  { id: 1, title: 'Облака', artist: 'Луна', duration: 180 },
  { id: 2, title: 'Air', artist: 'Yuki', duration: 120 },
  { id: 3, title: 'Moon', artist: 'Yuki', duration: 240 },
];
assert.deepEqual(selectTracks(tracks, '  YUKI ', 'duration').map(t => t.id), [2, 3]);
assert.deepEqual(selectTracks(tracks, 'ЛУНА', 'recent').map(t => t.id), [1]);
assert.deepEqual(selectTracks(tracks, '', 'duration').map(t => t.id), [2, 1, 3]);
assert.deepEqual(tracks.map(t => t.id), [1, 2, 3], 'Sorting must preserve the saved library order');
assert.deepEqual(selectTracks(tracks, 'missing', 'title'), []);
assert.equal(matches('Плейлист', ' плей '), true);
assert.equal(collectionDuration(tracks), '9 мин');
assert.equal(collectionDuration([{duration: 3660}]), '1 ч 1 мин');
assert.equal(collectionDuration([]), '0 мин');
console.log('Catalog filtering, order preservation, and duration checks passed.');
