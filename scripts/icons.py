"""One centered glyph for the app icon and vector artwork placeholder."""
import sys
from pathlib import Path
root=Path(__file__).resolve().parent.parent
sys.path.insert(0,str(root/'.tools/python'))
from PIL import Image,ImageDraw,ImageFont
from fontTools.ttLib import TTFont
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.boundsPen import BoundsPen
font_path=root/'public/fonts/UnifrakturCook-Bold.ttf'
font=TTFont(font_path);glyphs=font.getGlyphSet();glyph=glyphs[font.getBestCmap()[ord('y')]]
bounds=BoundsPen(glyphs);glyph.draw(bounds);x0,y0,x1,y1=bounds.bounds
pen=SVGPathPen(glyphs);glyph.draw(pen)
pad=(y1-y0)*.02
svg=f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{x0-pad:g} {y0-pad:g} {x1-x0+pad*2:g} {y1-y0+pad*2:g}" aria-hidden="true"><path fill="currentColor" transform="translate(0 {y0+y1:g}) scale(1 -1)" d="{pen.getCommands()}"/></svg>\n'
(root/'src/components/Mark.svelte').write_text('<script lang="ts"></script>\n'+svg,encoding='utf8')
size=1024
image=Image.new('RGBA',(size,size),(22,18,27,255))
glyph_font=ImageFont.truetype(str(font_path),760)
ink=Image.new('L',(size*2,size*2));draw=ImageDraw.Draw(ink);draw.text((size//2,0),'y',font=glyph_font,fill=255)
ink=ink.crop(ink.getbbox())
image.paste((232,223,238,255),((size-ink.width)//2,(size-ink.height)//2),ink)
image.resize((256,256),Image.Resampling.LANCZOS).save(root/'src-tauri/icons/icon.png')
image.save(root/'src-tauri/icons/icon.ico',sizes=[(16,16),(24,24),(32,32),(48,48),(64,64),(128,128),(256,256)])
print('Glyph ink is centered in both assets.')
