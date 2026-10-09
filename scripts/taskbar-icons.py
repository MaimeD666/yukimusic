"""Native thumbnail toolbar glyphs, in light and dark system theme variants."""
import sys
from pathlib import Path
root = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(root / '.tools/python'))
from PIL import Image, ImageDraw

target = root / 'src-tauri/icons/transport'
target.mkdir(exist_ok=True)
scale = 4
for theme, color in [('dark', '#eeeeee'), ('light', '#202020')]:
    for name in ['previous', 'play', 'pause', 'next']:
        image = Image.new('RGBA', (32 * scale, 32 * scale))
        draw = ImageDraw.Draw(image)
        def polygon(points):
            draw.polygon([(x * scale, y * scale) for x, y in points], fill=color)
        if name == 'play':
            polygon([(10, 6), (25, 16), (10, 26)])
        elif name == 'pause':
            polygon([(9, 6), (13, 6), (13, 26), (9, 26)])
            polygon([(19, 6), (23, 6), (23, 26), (19, 26)])
        else:
            polygon([(8, 7), (21, 16), (8, 25)])
            polygon([(22, 7), (25, 7), (25, 25), (22, 25)])
            if name == 'previous':
                image = image.transpose(Image.Transpose.FLIP_LEFT_RIGHT)
        image.resize((32, 32), Image.Resampling.LANCZOS).save(
            target / f'{name}-{theme}.ico', format='ICO', sizes=[(32, 32)])
print('Eight embedded taskbar icons generated.')
