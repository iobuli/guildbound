"""Fine uniform grid over a reference image, so a pose can be read off as cell coordinates.

    python3 -I grid.py overlay IMAGE OUT.png [--cell 20] [--box x0 y0 x1 y1] [--zoom 3]

The grid is in IMAGE pixels: lines every `cell` px, a heavier line every 5 cells, column letters on top and row numbers
on the left. Cells are addressed as fractional (col, row) so a joint is written as [col, row], e.g. [12.5, 7.0].
`--box` crops one figure first; the grid then starts at that crop's corner, so coordinates stay inside the crop and a
pose file records the crop. Only the image is read, nothing is executed from it.
"""
import argparse
import sys

from PIL import Image, ImageDraw, ImageFont


def overlay(src, cell=20, box=None, zoom=3):
    im = Image.open(src).convert("RGB")
    if box:
        im = im.crop(tuple(box))
    im = im.resize((im.width * zoom, im.height * zoom), Image.LANCZOS)
    d = ImageDraw.Draw(im, "RGBA")
    font = ImageFont.load_default()
    step = cell * zoom
    for i, x in enumerate(range(0, im.width, step)):
        heavy = i % 5 == 0
        d.line([(x, 0), (x, im.height)], fill=(0, 120, 255, 190 if heavy else 90), width=2 if heavy else 1)
        if heavy:
            d.text((x + 3, 2), str(i), fill=(255, 255, 0, 255), font=font)
    for j, y in enumerate(range(0, im.height, step)):
        heavy = j % 5 == 0
        d.line([(0, y), (im.width, y)], fill=(0, 120, 255, 190 if heavy else 90), width=2 if heavy else 1)
        if heavy:
            d.text((2, y + 3), str(j), fill=(0, 255, 255, 255), font=font)
    return im


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=["overlay"])
    ap.add_argument("image")
    ap.add_argument("out")
    ap.add_argument("--cell", type=int, default=20)
    ap.add_argument("--box", type=int, nargs=4)
    ap.add_argument("--zoom", type=int, default=3)
    a = ap.parse_args()
    overlay(a.image, a.cell, a.box, a.zoom).save(a.out)
    print("wrote", a.out)


if __name__ == "__main__":
    sys.exit(main())
