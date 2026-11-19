"""Bake the Horizon Night GX2 scene, font masks, and fixed-size glyph records."""
import argparse
from itertools import chain
from math import isqrt
from pathlib import Path
from random import Random
import struct

from PIL import Image, ImageDraw, ImageFont


HERE = Path(__file__).resolve().parent
WIDTH = 512
HEIGHT = 400
SCENE = (256, 144)
FONT_SPECS = (
    ("Geist-Regular.ttf", 36),
    ("BricolageGrotesqueDisplay-Bold.ttf", 64),
    ("GeistMono-Regular.ttf", 25),
)
DISPLAY_CHARS = frozenset("PlayProfilesVersion&resourcesSettingsAgentControlPreparingsessionDemosessionCouldnotstartClosingHorizon")


def skyline(draw, runs, color):
    x = 0
    for width, y in runs:
        draw.rectangle((x, y, x + width - 1, SCENE[1] - 1), fill=color)
        x += width
    assert x == SCENE[0]


def scene():
    image = Image.new("RGBA", SCENE)
    draw = ImageDraw.Draw(image)
    # Night sky stops and sunset colors follow horizon-theme/src/night.rs and
    # launcher/src/sky.rs. This composition uses one fixed GamePad-sized grid.
    bands = ((0, "#07061a"), (29, "#140f38"), (55, "#2a1656"),
             (79, "#5b2a78"), (102, "#b0477d"), (117, "#f08a5d"))
    ends = chain((y for y, _ in bands[1:]), (SCENE[1],))
    for (start, color), end in zip(bands, ends):
        draw.rectangle((0, start, SCENE[0] - 1, end - 1), fill=color)

    rng = Random(0x484F52495A4F4E)
    for _ in range(108):
        x, y = rng.randrange(256), rng.randrange(63)
        if x < 145 and 27 <= y < 49:
            continue
        draw.point((x, y), fill=rng.choice(("#473975", "#7564a0", "#b2a4d8", "#493b74")))
    for x, y in ((27, 17), (72, 30), (109, 12), (164, 24), (233, 33), (225, 54)):
        draw.line((x - 1, y, x + 1, y), fill="#a094c8")
        draw.line((x, y - 1, x, y + 1), fill="#cfc9ea")
        draw.point((x, y), fill="#f4f2ff")

    for x, y, width, color, edge in (
        (24, 61, 35, "#382468", "#523581"),
        (112, 70, 45, "#432576", "#603387"),
        (202, 67, 36, "#432576", "#61378b"),
        (40, 89, 53, "#69367c", "#89508f"),
        (159, 97, 60, "#7d3e83", "#a25591"),
        (219, 88, 41, "#69347d", "#8b4a8b"),
        (6, 108, 37, "#aa567e", "#cb7191"),
        (89, 114, 41, "#b75d87", "#de879d"),
        (226, 108, 32, "#a1517f", "#cc7392"),
    ):
        draw.rectangle((x + 7, y - 1, x + width - 10, y), fill=color)
        draw.rectangle((x + 3, y, x + width - 4, y + 2), fill=color)
        draw.line((x, y + 2, x + width - 1, y + 2), fill=color)
        draw.line((x + 6, y + 3, x + width - 7, y + 3), fill=edge)
        draw.line((x + 11, y + 4, x + width - 13, y + 4), fill=color)

    # Flat-bottomed, striped sun behind the skyline.
    sun_x, sun_y, radius = 198, 126, 21
    for y in range(sun_y - radius, sun_y + 1):
        if y in (116, 119, 122, 125):
            continue
        half = isqrt(radius * radius - (sun_y - y) ** 2)
        color = "#ffe28c" if y < 110 else "#ffbf66" if y < 119 else "#ff944f"
        draw.line((sun_x - half, y, sun_x + half, y), fill=color)

    skyline(draw, ((18, 127), (18, 124), (17, 121), (14, 123), (21, 127),
                   (19, 130), (23, 127), (13, 124), (15, 125), (21, 129),
                   (18, 130), (10, 127), (12, 124), (18, 123), (19, 126)), "#3a1f5e")
    skyline(draw, ((16, 132), (9, 130), (18, 132), (11, 135), (17, 138),
                   (18, 136), (17, 134), (14, 135), (12, 137), (20, 139),
                   (17, 137), (22, 134), (17, 135), (21, 132), (7, 134), (20, 136)), "#150f2e")
    for x, y in ((15, 132), (27, 132), (42, 132), (105, 134), (173, 137), (221, 132), (237, 136)):
        draw.rectangle((x, y - 6, x + 1, y), fill="#150f2e")
        draw.line((x - 2, y - 4, x + 3, y - 4), fill="#150f2e")
        draw.line((x - 3, y - 2, x + 4, y - 2), fill="#150f2e")
    return image


def bake(source):
    fonts = source / "crates/horizon-theme/fonts"
    atlas = Image.new("RGBA", (WIDTH, HEIGHT), (255, 255, 255, 0))
    atlas.paste(scene(), (0, 0))
    icon = Image.open(source / "crates/horizon-theme/icons/horizon-64.png").convert("RGBA")
    icon = icon.resize((40, 40), Image.Resampling.LANCZOS)
    atlas.paste(icon, (264, 0))
    atlas.putpixel((309, 0), (255, 255, 255, 255))

    # Reusable antialiased quarter-corner, with the solid interior toward
    # the lower right. The renderer mirrors it for the other corners.
    corner = Image.new("L", (64, 64))
    ImageDraw.Draw(corner).ellipse((0, 0, 127, 127), fill=255)
    corner = corner.resize((16, 16), Image.Resampling.LANCZOS)
    atlas.paste(Image.merge("RGBA", (Image.new("L", corner.size, 255),) * 3 + (corner,)), (312, 0))

    records = [bytes(10)] * (len(FONT_SPECS) * 95)
    glyphs = []
    for face, (name, size) in enumerate(FONT_SPECS):
        font = ImageFont.truetype(fonts / name, size, layout_engine=ImageFont.Layout.BASIC)
        for index, code in enumerate(range(32, 127)):
            char = chr(code)
            if face == 1 and char != " " and char not in DISPLAY_CHARS:
                continue
            left, top, right, bottom = font.getbbox(char, anchor="ls")
            width, height = right - left, bottom - top
            mask = Image.new("L", (width, height))
            if width and height:
                ImageDraw.Draw(mask).text((-left, -top), char, font=font, anchor="ls", fill=255)
            advance = int(round(font.getlength(char) * 64))
            glyphs.append((face * 95 + index, mask, left, top, advance))

    # Fill the unused space beside the sky first, then continue across the
    # full atlas below it. Tall glyphs go first to keep each shelf compact.
    pending = sorted(glyphs, key=lambda item: (-item[1].height, -item[1].width, item[0]))
    for begin_x, begin_y, end_y in ((260, 46, 143), (1, 146, HEIGHT)):
        x, y, shelf = begin_x, begin_y, 0
        while pending:
            index, mask, left, top, advance = pending[0]
            w, h = mask.size
            if x + w + 1 >= WIDTH:
                x, y, shelf = begin_x, y + shelf + 2, 0
            if y + h + 1 >= end_y:
                break
            records[index] = struct.pack(">HHBBbbH", x, y, w, h, left, top, advance)
            if w and h:
                white = Image.new("L", mask.size, 255)
                atlas.paste(Image.merge("RGBA", (white, white, white, mask)), (x, y))
            x += w + 2
            shelf = max(shelf, h)
            pending.pop(0)
    if pending:
        raise SystemExit(f"{len(pending)} glyphs did not fit in the atlas")

    (HERE / "night-atlas.rgba").write_bytes(atlas.tobytes())
    (HERE / "night-fonts.bin").write_bytes(b"".join(records))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--horizon-source", required=True, type=Path)
    bake(parser.parse_args().horizon_source)
