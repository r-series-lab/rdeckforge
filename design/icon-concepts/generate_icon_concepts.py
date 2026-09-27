from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter, ImageFont


ROOT = Path(__file__).resolve().parent
R_SERIES_ROOT = ROOT.parents[2]
SIZE = 1024
TILE = (30, 30, 994, 994)
RADIUS = 155

FONT_BLACK = "/Library/Fonts/Swis721 Blk BT Black.ttf"
FONT_BOLD = "/System/Library/Fonts/Supplemental/Arial Bold.ttf"

WHITE = (248, 250, 252, 255)
SOFT_WHITE = (236, 241, 248, 255)
SILVER = (201, 209, 222, 255)
SILVER_DARK = (158, 169, 187, 255)
BLUE = (107, 159, 255, 255)
BLUE_SOFT = (151, 190, 255, 255)


@dataclass(frozen=True)
class Concept:
    key: str
    title: str
    subtitle: str
    renderer: str


CONCEPTS = [
    Concept("A", "D Stack", "Safest family mark: bold D plus forged slide layers.", "a"),
    Concept("B", "Strike Deck", "Cold-blue forge strike, still keeping the D dominant.", "b"),
    Concept("C", "Press Mold", "Deck plates compress into the D; more tool-like.", "c"),
    Concept("D", "Forge F", "Stronger Forge reading, built from presentation bars.", "d"),
    Concept("E", "Monolith", "Minimal carved D with quiet deck strata.", "e"),
]


def font(size: int, path: str = FONT_BLACK) -> ImageFont.FreeTypeFont:
    return ImageFont.truetype(path, size)


def rounded_tile() -> tuple[Image.Image, Image.Image]:
    img = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    mask = Image.new("L", (SIZE, SIZE), 0)
    md = ImageDraw.Draw(mask)
    md.rounded_rectangle(TILE, radius=RADIUS, fill=255)

    grad = Image.new("RGBA", (SIZE, SIZE), (5, 7, 12, 255))
    px = grad.load()
    for y in range(SIZE):
        for x in range(SIZE):
            dx = (x - 360) / SIZE
            dy = (y - 280) / SIZE
            light = max(0, 1 - (dx * dx + dy * dy) * 4.5)
            edge = (x + y) / (SIZE * 2)
            r = int(5 + light * 16 + edge * 4)
            g = int(7 + light * 17 + edge * 4)
            b = int(12 + light * 22 + edge * 5)
            px[x, y] = (r, g, b, 255)

    img.alpha_composite(grad, (0, 0))
    img.putalpha(mask)

    shine = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    sd = ImageDraw.Draw(shine)
    sd.ellipse((-170, 5, 740, 510), fill=(255, 255, 255, 14))
    shine.putalpha(Image.composite(shine.getchannel("A"), Image.new("L", (SIZE, SIZE), 0), mask))
    img.alpha_composite(shine)

    return img, mask


def shadowed_layer(base: Image.Image, layer: Image.Image, opacity: int = 115) -> None:
    shadow = Image.new("RGBA", layer.size, (0, 0, 0, 0))
    shadow.alpha_composite(layer)
    alpha = shadow.getchannel("A").point(lambda a: int(a * opacity / 255))
    shadow = Image.new("RGBA", layer.size, (0, 0, 0, 255))
    shadow.putalpha(alpha.filter(ImageFilter.GaussianBlur(14)))
    base.alpha_composite(shadow, (0, 10))
    base.alpha_composite(layer)


def draw_letter(
    img: Image.Image,
    text: str,
    size: int,
    center: tuple[int, int],
    fill: tuple[int, int, int, int] = WHITE,
    x_adjust: int = 0,
    y_adjust: int = 0,
) -> None:
    layer = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    d = ImageDraw.Draw(layer)
    f = font(size)
    bbox = d.textbbox((0, 0), text, font=f)
    w = bbox[2] - bbox[0]
    h = bbox[3] - bbox[1]
    x = center[0] - w // 2 - bbox[0] + x_adjust
    y = center[1] - h // 2 - bbox[1] + y_adjust
    d.text((x + 7, y + 8), text, font=f, fill=(0, 0, 0, 80))
    d.text((x, y), text, font=f, fill=fill)
    shadowed_layer(img, layer, 90)


def rounded_bar(
    size: tuple[int, int],
    radius: int,
    fill: tuple[int, int, int, int],
    angle: float = 0,
) -> Image.Image:
    pad = 36
    w, h = size
    layer = Image.new("RGBA", (w + pad * 2, h + pad * 2), (0, 0, 0, 0))
    d = ImageDraw.Draw(layer)
    d.rounded_rectangle((pad, pad, pad + w, pad + h), radius=radius, fill=fill)
    if angle:
        layer = layer.rotate(angle, expand=True, resample=Image.Resampling.BICUBIC)
    return layer


def paste_center(base: Image.Image, layer: Image.Image, center: tuple[int, int]) -> None:
    x = int(center[0] - layer.width / 2)
    y = int(center[1] - layer.height / 2)
    base.alpha_composite(layer, (x, y))


def draw_deck_stack(
    img: Image.Image,
    center: tuple[int, int],
    width: int = 330,
    height: int = 70,
    angle: float = -18,
    gap: int = 62,
    colors: tuple[tuple[int, int, int, int], ...] = (SOFT_WHITE, SILVER, SILVER_DARK),
) -> None:
    for i, color in enumerate(reversed(colors)):
        layer = rounded_bar((width, height), 18, color, angle)
        c = (center[0] + i * 12, center[1] + (2 - i) * gap)
        paste_center(img, layer.filter(ImageFilter.GaussianBlur(0.15)), c)


def draw_spark(draw: ImageDraw.ImageDraw, x: int, y: int, scale: int = 1, color=BLUE_SOFT) -> None:
    w = 8 * scale
    draw.line((x - 34 * scale, y, x + 34 * scale, y), fill=color, width=w)
    draw.line((x, y - 34 * scale, x, y + 34 * scale), fill=color, width=w)
    draw.line((x - 22 * scale, y - 22 * scale, x + 22 * scale, y + 22 * scale), fill=color, width=max(4, w - 2))
    draw.line((x + 22 * scale, y - 22 * scale, x - 22 * scale, y + 22 * scale), fill=color, width=max(4, w - 2))


def concept_a() -> Image.Image:
    img, _ = rounded_tile()
    draw_letter(img, "D", 610, (430, 510), x_adjust=-58, y_adjust=0)
    draw_deck_stack(img, (675, 725), 330, 72, -19, 64)
    return img


def concept_b() -> Image.Image:
    img, _ = rounded_tile()
    draw_letter(img, "D", 585, (414, 510), x_adjust=-66)
    strike = rounded_bar((350, 48), 24, (235, 241, 250, 255), -34)
    paste_center(img, strike, (715, 414))
    accent = rounded_bar((252, 22), 11, BLUE, -34)
    paste_center(img, accent, (734, 387))
    d = ImageDraw.Draw(img)
    draw_spark(d, 835, 305, 2, BLUE_SOFT)
    draw_spark(d, 780, 492, 1, (225, 235, 255, 255))
    draw_deck_stack(img, (690, 752), 275, 56, -18, 48, (SOFT_WHITE, SILVER, SILVER_DARK))
    return img


def concept_c() -> Image.Image:
    img, _ = rounded_tile()
    draw_letter(img, "D", 585, (400, 500), x_adjust=-76)
    for y, w, fill in [
        (410, 280, SOFT_WHITE),
        (520, 326, (224, 231, 242, 255)),
        (630, 372, SILVER),
    ]:
        bar = rounded_bar((w, 62), 18, fill, 0)
        paste_center(img, bar, (685, y))
    d = ImageDraw.Draw(img)
    d.rounded_rectangle((635, 278, 875, 342), radius=22, fill=(248, 250, 252, 245))
    d.rounded_rectangle((676, 298, 790, 316), radius=9, fill=(11, 15, 22, 255))
    d.ellipse((812, 294, 836, 318), fill=BLUE)
    return img


def concept_d() -> Image.Image:
    img, _ = rounded_tile()
    draw_letter(img, "F", 630, (360, 505), x_adjust=-40)
    for y, w, color in [
        (392, 325, SOFT_WHITE),
        (528, 300, (226, 233, 244, 255)),
        (664, 270, SILVER),
    ]:
        layer = rounded_bar((w, 58), 20, color, 0)
        paste_center(img, layer, (685, y))
        small = rounded_bar((72, 14), 7, (12, 16, 24, 255), 0)
        paste_center(img, small, (590, y + 2))
    d = ImageDraw.Draw(img)
    draw_spark(d, 810, 280, 2, BLUE_SOFT)
    draw_spark(d, 740, 746, 1, (235, 241, 250, 255))
    return img


def concept_e() -> Image.Image:
    img, _ = rounded_tile()
    d = ImageDraw.Draw(img)
    mark = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    md = ImageDraw.Draw(mark)
    md.rounded_rectangle((245, 248, 400, 780), radius=30, fill=WHITE)
    md.pieslice((220, 224, 820, 824), start=-88, end=88, fill=WHITE)
    md.rectangle((245, 248, 520, 780), fill=WHITE)
    md.pieslice((335, 344, 650, 704), start=-90, end=90, fill=(0, 0, 0, 0))
    # Inner cutout is drawn on the alpha channel for a crisp carved monolith.
    alpha = mark.getchannel("A")
    cut = Image.new("L", (SIZE, SIZE), 0)
    cd = ImageDraw.Draw(cut)
    cd.rounded_rectangle((410, 344, 633, 704), radius=122, fill=255)
    alpha = Image.composite(Image.new("L", (SIZE, SIZE), 0), alpha, cut)
    mark.putalpha(alpha)
    shadowed_layer(img, mark, 95)

    draw_deck_stack(img, (687, 742), 315, 55, -18, 48, ((245, 248, 252, 255), SILVER, SILVER_DARK))
    d.rounded_rectangle((610, 300, 842, 344), radius=22, fill=(245, 248, 252, 245))
    d.rounded_rectangle((628, 314, 750, 327), radius=6, fill=(11, 15, 22, 255))
    d.ellipse((792, 309, 820, 337), fill=BLUE)
    return img


RENDERERS = {
    "a": concept_a,
    "b": concept_b,
    "c": concept_c,
    "d": concept_d,
    "e": concept_e,
}


def make_sheet(paths: list[tuple[Concept, Path]]) -> Image.Image:
    sheet = Image.new("RGBA", (2100, 1560), (29, 31, 36, 255))
    d = ImageDraw.Draw(sheet)
    title_font = font(58, FONT_BOLD)
    small_font = font(29, FONT_BOLD)
    label_font = font(36, FONT_BOLD)
    body_font = ImageFont.truetype(FONT_BOLD, 24)

    d.text((78, 56), "rDeckForge icon concepts", fill=(246, 248, 252, 255), font=title_font)
    d.text(
        (80, 128),
        "near-black r-series tile | large white mark | deck/forge domain cue | preview only",
        fill=(169, 176, 190, 255),
        font=small_font,
    )

    positions = [(86, 220), (744, 220), (1402, 220), (412, 895), (1070, 895)]
    for (concept, path), (x, y) in zip(paths, positions):
        card = (x, y, x + 610, y + 585)
        d.rounded_rectangle(card, radius=32, fill=(43, 46, 54, 255))
        icon = Image.open(path).convert("RGBA").resize((328, 328), Image.Resampling.LANCZOS)
        sheet.alpha_composite(icon, (x + 141, y + 36))

        mini128 = icon.resize((86, 86), Image.Resampling.LANCZOS)
        mini32 = icon.resize((32, 32), Image.Resampling.LANCZOS)
        sheet.alpha_composite(mini128, (x + 54, y + 374))
        sheet.alpha_composite(mini32, (x + 160, y + 401))

        d.text((x + 224, y + 378), f"{concept.key}. {concept.title}", fill=(247, 249, 252, 255), font=label_font)
        words = concept.subtitle.split()
        line = ""
        lines: list[str] = []
        for word in words:
            probe = f"{line} {word}".strip()
            if d.textlength(probe, font=body_font) > 340 and line:
                lines.append(line)
                line = word
            else:
                line = probe
        if line:
            lines.append(line)
        for idx, text in enumerate(lines[:3]):
            d.text((x + 224, y + 428 + idx * 34), text, fill=(178, 185, 198, 255), font=body_font)

    return sheet


def make_family_strip(paths: list[tuple[Concept, Path]]) -> Image.Image:
    strip = Image.new("RGBA", (2060, 470), (88, 88, 88, 255))
    d = ImageDraw.Draw(strip)
    font_label = ImageFont.truetype(FONT_BOLD, 32)
    font_caption = ImageFont.truetype(FONT_BOLD, 23)

    icons: list[tuple[str, str, Path]] = [
        (
            "Reference",
            "rDevTool",
            R_SERIES_ROOT / "rdevtool/src-tauri/icons/rdevtool-app-icon-source.png",
        ),
        (
            "Reference",
            "rCodexManager",
            R_SERIES_ROOT / "rcodexmanager/src-tauri/icons/rcodexmanager-app-icon-source.png",
        ),
    ]
    icons += [(f"Option {concept.key}", f"rDeckForge {concept.key}", path) for concept, path in paths]

    gap = 275
    start_x = 70
    y = 62
    for idx, (caption, label, path) in enumerate(icons):
        x = start_x + idx * gap
        icon = Image.open(path).convert("RGBA").resize((150, 150), Image.Resampling.LANCZOS)
        strip.alpha_composite(icon, (x + 36, y))
        tw = d.textlength(label, font=font_label)
        cw = d.textlength(caption, font=font_caption)
        d.text((x + 111 - cw / 2, y + 186), caption, fill=(213, 216, 222, 255), font=font_caption)
        d.text((x + 111 - tw / 2, y + 223), label, fill=(255, 255, 255, 255), font=font_label)

    return strip


def main() -> None:
    paths: list[tuple[Concept, Path]] = []
    for concept in CONCEPTS:
        img = RENDERERS[concept.renderer]()
        slug = concept.title.lower().replace(" ", "-")
        path = ROOT / f"rdeckforge-icon-{concept.key.lower()}-{slug}.png"
        img.save(path)
        paths.append((concept, path))

    sheet = make_sheet(paths)
    sheet.save(ROOT / "rdeckforge-icon-concepts-sheet.png")
    strip = make_family_strip(paths)
    strip.save(ROOT / "rdeckforge-icon-family-strip.png")


if __name__ == "__main__":
    main()
