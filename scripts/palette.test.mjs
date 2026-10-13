import assert from 'node:assert/strict';
import { paletteFromPixels, paletteForTheme, contrast } from '../src/lib/palette.ts';
const pixels = (colors) => Uint8ClampedArray.from(colors.flatMap(rgb => Array.from({length:64},()=>[...rgb,255]).flat()));
for (const colors of [ [[0,0,0]], [[255,255,255]], [[120,120,120]], [[250,5,5]], [[5,20,250]], [[0,245,80]], [[255,255,0]], [[20,0,20],[200,20,120]], [[5,5,5],[5,5,5],[170,20,10]] ]) {
  const palette=paletteFromPixels(pixels(colors));
  assert(contrast(palette.foreground,palette.raised)>=7,JSON.stringify(palette));
  assert(contrast(palette.muted,palette.raised)>=4.5,JSON.stringify(palette));
  assert(contrast(palette.onAccent,palette.accent)>=4.5,JSON.stringify(palette));
  const light=paletteForTheme(palette,'light');
  for (const bg of [light.background,light.surface,light.raised]) {
    assert(contrast(light.foreground,bg)>=7,JSON.stringify(light));
    assert(contrast(light.muted,bg)>=4.5,JSON.stringify(light));
    assert(contrast(light.accent,bg)>=4.5,JSON.stringify(light));
  }
  assert(contrast(light.onAccent,light.accent)>=4.5,JSON.stringify(light));
}
const red=paletteFromPixels(pixels([[200,20,20]])).background;
const blue=paletteFromPixels(pixels([[20,40,200]])).background;
assert.notEqual(red,blue);
assert.notEqual(paletteForTheme(paletteFromPixels(pixels([[200,20,20]])),'light').background, paletteForTheme(paletteFromPixels(pixels([[20,40,200]])),'light').background);
assert(parseInt(red.slice(1,3),16)>parseInt(red.slice(5,7),16));
assert(parseInt(blue.slice(5,7),16)>parseInt(blue.slice(1,3),16));
for(const palette of [paletteFromPixels(pixels([[0,0,0]])),paletteFromPixels(pixels([[255,255,255]]))]) {
 const rgb=[1,3,5].map(i=>parseInt(palette.background.slice(i,i+2),16));
 assert(Math.max(...rgb)-Math.min(...rgb)<3,'Grayscale artwork must stay neutral');
}
console.log('Palette contrast and hue checks passed.');
