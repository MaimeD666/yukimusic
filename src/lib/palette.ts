export type PlayerPalette = {
  background: string;
  surface: string;
  raised: string;
  foreground: string;
  muted: string;
  border: string;
  accent: string;
  onAccent: string;
  secondary: string;
};
type RGB = [number, number, number];
type Lab = [number, number, number];
const clamp = (v: number, min = 0, max = 1) => Math.max(min, Math.min(max, v));
const linear = (v: number) =>
  v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
const gamma = (v: number) =>
  v <= 0.0031308 ? 12.92 * v : 1.055 * v ** (1 / 2.4) - 0.055;
function toLab(rgb: RGB): Lab {
  const [r, g, b] = rgb.map((v) => linear(v / 255));
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  return [
    0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
  ];
}
function fromLab([L, a, b]: Lab): RGB {
  const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3,
    m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3,
    s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
  return [
    4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
    -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s,
  ].map((v) => Math.round(clamp(gamma(Math.max(0, v))) * 255)) as RGB;
}
const hex = (rgb: RGB) =>
  "#" + rgb.map((v) => v.toString(16).padStart(2, "0")).join("");
const rgb = (color: string) =>
  [1, 3, 5].map((i) => parseInt(color.slice(i, i + 2), 16)) as RGB;
function luminance(color: string) {
  const [r, g, b] = rgb(color).map((v) => linear(v / 255));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}
export function contrast(a: string, b: string) {
  const x = luminance(a),
    y = luminance(b);
  return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
}
function shade(base: Lab, light: number, chromaScale: number) {
  return hex(fromLab([light, base[1] * chromaScale, base[2] * chromaScale]));
}
function legible(
  base: Lab,
  bg: string,
  light: number,
  ratio: number,
  chroma = 0.16,
) {
  let result = shade(base, light, chroma);
  while (contrast(result, bg) < ratio && light < 0.99) {
    light += 0.015;
    result = shade(base, light, chroma);
  }
  return result;
}
export const neutralPalette: PlayerPalette = {
  background: "#19171c",
  surface: "#25222a",
  raised: "#34303a",
  foreground: "#f2edf5",
  muted: "#bfb6c6",
  border: "#514957",
  accent: "#e6dbea",
  onAccent: "#19121d",
  secondary: "#c5b8ce",
};
/** Weighted perceptual clusters; tiny bright pixels cannot recolor the whole player. */
export function paletteFromPixels(pixels: ArrayLike<number>): PlayerPalette {
  const bins = new Map<string, { sum: RGB; count: number }>();
  for (let i = 0; i < pixels.length; i += 4) {
    if (pixels[i + 3] < 200) continue;
    const value: RGB = [pixels[i], pixels[i + 1], pixels[i + 2]];
    const key = value.map((v) => Math.floor(v / 24)).join(",");
    const bin = bins.get(key) || { sum: [0, 0, 0] as RGB, count: 0 };
    bin.count++;
    for (let c = 0; c < 3; c++) bin.sum[c] += value[c];
    bins.set(key, bin);
  }
  const samples = [...bins.values()]
    .map((b) => ({
      lab: toLab(b.sum.map((v) => v / b.count) as RGB),
      count: b.count,
    }))
    .sort((a, b) => b.count - a.count);
  if (!samples.length) return neutralPalette;
  const clusters: { lab: Lab; count: number }[] = [];
  for (const sample of samples) {
    const near = clusters.find(
      (c) => Math.hypot(...c.lab.map((v, i) => v - sample.lab[i])) < 0.105,
    );
    if (near) {
      const n = near.count + sample.count;
      near.lab = near.lab.map(
        (v, i) => (v * near.count + sample.lab[i] * sample.count) / n,
      ) as Lab;
      near.count = n;
    } else clusters.push({ ...sample });
  }
  clusters.sort((a, b) => b.count - a.count);
  const total = samples.reduce((sum, s) => sum + s.count, 0);
  const colored = clusters.filter(
    (c) =>
      c.count / total > 0.035 &&
      c.lab[0] > 0.13 &&
      c.lab[0] < 0.94 &&
      Math.hypot(c.lab[1], c.lab[2]) > 0.035,
  );
  colored.sort(
    (a, b) =>
      b.count * (0.06 + Math.hypot(b.lab[1], b.lab[2])) -
      a.count * (0.06 + Math.hypot(a.lab[1], a.lab[2])),
  );
  const primary = colored[0]?.lab || clusters[0].lab;
  const second =
    colored.find(
      (c) => Math.hypot(c.lab[1] - primary[1], c.lab[2] - primary[2]) > 0.065,
    )?.lab || primary;
  const chroma = Math.hypot(primary[1], primary[2]);
  const scale = chroma > 0.001 ? Math.min(0.095 / chroma, 0.85) : 0;
  const base: Lab = [primary[0], primary[1] * scale, primary[2] * scale];
  const light = clamp(0.205 + primary[0] * 0.12, 0.22, 0.3);
  const background = shade(base, light, 1),
    surface = shade(base, light + 0.055, 0.85),
    raised = shade(base, light + 0.115, 0.72);
  const accent = legible(primary, surface, 0.88, 5.5, 0.65);
  return {
    background,
    surface,
    raised,
    accent,
    onAccent: contrast("#151019", accent) > 4.5 ? "#151019" : "#ffffff",
    foreground: legible(primary, raised, 0.96, 7),
    muted: legible(primary, raised, 0.78, 4.5),
    border: shade(base, light + 0.25, 0.6),
    secondary: legible(second, surface, 0.8, 4.5, 0.6),
  };
}
