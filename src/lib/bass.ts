// The first 12 logarithmic FFT bands cover roughly 35–160 Hz.
// Convert their display dB scale back to amplitude: treble and silence cannot pulse the cover.
export function bassEnergy(bands: readonly number[]) {
  let power = 0;
  let peak = 0;
  const count = Math.min(12, bands.length);
  for (let i = 0; i < count; i++) {
    const value = Number.isFinite(bands[i])
      ? Math.max(0, Math.min(1, bands[i]))
      : 0;
    const amplitude = Math.pow(10, (value * 60 - 70) / 20);
    power += amplitude * amplitude;
    peak = Math.max(peak, amplitude);
  }
  const energy = count ? Math.sqrt(power / count) * 0.65 + peak * 0.35 : 0;
  return Math.pow(Math.max(0, Math.min(1, (energy - 0.004) * 5)), 0.72);
}

export class BassEnvelope {
  level = 0;
  update(target: number, elapsed: number) {
    // Time-based response remains identical on 60 / 144 / 165 Hz displays.
    const response = target > this.level ? 8 : 115;
    this.level +=
      (target - this.level) *
      (1 - Math.exp(-Math.min(100, elapsed) / response));
    if (this.level < 0.001) this.level = 0;
    return this.level;
  }
}
