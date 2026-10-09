import { api, native } from "./bridge";
import {
  neutralPalette,
  paletteFromPixels,
  type PlayerPalette,
} from "./palette";
const cache = new Map<string, Promise<string>>();

export function artworkSource(url: string | null): Promise<string> {
  if (!url) return Promise.resolve("");
  if (!cache.has(url)) {
    if (cache.size > 80) cache.delete(cache.keys().next().value!);
    cache.set(
      url,
      native
        ? api.artwork(url).catch(() => {
            cache.delete(url);
            return "";
          })
        : Promise.resolve(url),
    );
  }
  return cache.get(url)!;
}

export async function palette(url: string | null): Promise<PlayerPalette> {
  const source = await artworkSource(url);
  if (!source) return neutralPalette;
  return new Promise((resolve) => {
    const img = new Image();
    img.crossOrigin = "anonymous";
    img.onload = () => {
      try {
        const canvas = document.createElement("canvas");
        canvas.width = canvas.height = 64;
        const ctx = canvas.getContext("2d")!;
        ctx.drawImage(img, 0, 0, 64, 64);
        resolve(paletteFromPixels(ctx.getImageData(0, 0, 64, 64).data));
      } catch {
        resolve(neutralPalette);
      }
    };
    img.onerror = () => resolve(neutralPalette);
    img.src = source;
  });
}
