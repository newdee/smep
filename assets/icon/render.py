"""Render smep's icon: a cinnabar seal with an M on warm white paper.

The geometry is the one in logo.svg next to this file; this draws it with
Pillow (no SVG rasterizer needed) at every size the platforms want,
supersampled, and packs the Windows .ico.

    python assets/icon/render.py
"""

from pathlib import Path

from PIL import Image, ImageDraw

HERE = Path(__file__).parent
PAPER = (0xF8, 0xF7, 0xF3, 255)
LINE = (0xE4, 0xE1, 0xDA, 255)
CINNABAR = (0xC3, 0x42, 0x3F, 255)
SIZES = [16, 32, 48, 64, 128, 256, 512, 1024]
ICO_SIZES = [16, 32, 48, 64, 128, 256]


def draw(size: int, scale: int = 4) -> Image.Image:
    """The icon at `size` px, drawn `scale` times larger and shrunk."""
    s = size * scale
    u = s / 1024.0  # one unit of the 1024 design grid
    img = Image.new("RGBA", (s, s), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    # Paper: a rounded square with a hairline edge.
    d.rounded_rectangle([0, 0, s - 1, s - 1], radius=224 * u, fill=PAPER, outline=LINE, width=max(1, int(8 * u)))
    # Seal: a smaller rounded square in cinnabar.
    d.rounded_rectangle([212 * u, 212 * u, 812 * u, 812 * u], radius=72 * u, fill=CINNABAR)
    # The M: one stroke with round joins and caps.
    points = [(322 * u, 668 * u), (322 * u, 372 * u), (512 * u, 590 * u), (702 * u, 372 * u), (702 * u, 668 * u)]
    width = 74 * u
    d.line(points, fill=PAPER, width=int(width), joint="curve")
    for x, y in (points[0], points[-1]):
        d.ellipse([x - width / 2, y - width / 2, x + width / 2, y + width / 2], fill=PAPER)
    return img.resize((size, size), Image.LANCZOS)


def main() -> None:
    rendered = {size: draw(size) for size in SIZES}
    for size, img in rendered.items():
        img.save(HERE / f"smep-{size}.png")
    base = rendered[256]
    base.save(
        HERE / "smep.ico",
        format="ICO",
        sizes=[(n, n) for n in ICO_SIZES],
        append_images=[rendered[n] for n in ICO_SIZES if n != 256],
    )
    print("rendered", ", ".join(f"{n}px" for n in SIZES), "and smep.ico")


if __name__ == "__main__":
    main()
