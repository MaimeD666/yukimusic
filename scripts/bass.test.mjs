import assert from 'node:assert/strict';
import { bassEnergy, BassEnvelope } from '../src/lib/bass.ts';

assert.equal(bassEnergy(Array(48).fill(0)), 0, 'silence must be still');
assert.equal(bassEnergy([...Array(12).fill(0), ...Array(36).fill(1)]), 0, 'treble cannot trigger bass');
assert.equal(bassEnergy(Array(48).fill(NaN)), 0);
const kick = bassEnergy([...Array(12).fill(0.9), ...Array(36).fill(0)]);
assert(kick > 0.7 && kick <= 1, 'a strong bass hit must visibly respond');
const envelope = new BassEnvelope();
assert(envelope.update(kick, 16.67) > kick * 0.85, 'attack fits within one 60 Hz frame');
for (let i = 0; i < 90; i++) envelope.update(0, 16.67);
assert.equal(envelope.level, 0, 'pause/silence must settle completely');
function release(fps) {
  const e = new BassEnvelope(); e.level = 1;
  for (let i = 0; i < fps; i++) e.update(0, 1000 / fps);
  return e.level;
}
assert(Math.abs(release(60) - release(165)) < 0.001, 'release must not depend on monitor refresh');
console.log('Bass: frequency isolation, attack, silence and refresh-rate checks passed.');
