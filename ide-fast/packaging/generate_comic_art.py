#!/usr/bin/env python3
"""
Generate Comic 3D-to-2D Cel-Shaded Installer Graphics for WiX:
- banner.bmp: 493 x 58 pixels (24-bit RGB BMP)
- dialog.bmp: 493 x 312 pixels (24-bit RGB BMP)

Design style:
- Cel-shaded bold black ink outlines (赛璐珞硬黑边)
- Pop art high-saturation palette:
    Pop Yellow (#ffcc00), Comic Pink (#ff3366), Cyber Cyan (#00b4d8), Deep Ink (#12131a)
- Halftone dot screen pattern (美漫半调网点)
- 3D-to-2D extruded typography and Greek Golden Laurel motif
"""

import math
from PIL import Image, ImageDraw, ImageFont

def draw_halftone_dots(draw, x0, y0, x1, y1, dot_color, spacing=10, radius=2):
    for y in range(y0, y1, spacing):
        for x in range(x0, x1, spacing):
            # stagger odd rows
            cx = x + (spacing // 2 if (y // spacing) % 2 == 1 else 0)
            if cx <= x1:
                draw.ellipse([cx - radius, y - radius, cx + radius, y + radius], fill=dot_color)

def draw_greek_laurel_leaf(draw, cx, cy, radius, angle_deg, fill_color=(255, 204, 0), stroke_color=(0, 0, 0)):
    # Cel-shaded Greek golden laurel leaf
    rad = math.radians(angle_deg)
    cos_a = math.cos(rad)
    sin_a = math.sin(rad)
    # Leaf diamond/oval points
    p0 = (cx - radius * cos_a, cy - radius * sin_a)
    p1 = (cx - (radius * 0.4) * sin_a, cy + (radius * 0.4) * cos_a)
    p2 = (cx + radius * 1.2 * cos_a, cy + radius * 1.2 * sin_a)
    p3 = (cx + (radius * 0.4) * sin_a, cy - (radius * 0.4) * cos_a)
    poly = [(int(x), int(y)) for (x, y) in [p0, p1, p2, p3]]
    draw.polygon(poly, fill=fill_color, outline=stroke_color)

def draw_cel_shaded_text(draw, pos, text, font, fill_color, stroke_color=(0, 0, 0), shadow_offset=(3, 3), stroke_width=2):
    x, y = pos
    # Hard drop shadow
    sx, sy = x + shadow_offset[0], y + shadow_offset[1]
    draw.text((sx, sy), text, font=font, fill=stroke_color, stroke_width=stroke_width, stroke_fill=stroke_color)
    # 3D extrusion intermediate layers
    steps = max(abs(shadow_offset[0]), abs(shadow_offset[1]))
    extrude_color = (max(0, fill_color[0] - 60), max(0, fill_color[1] - 60), max(0, fill_color[2] - 30))
    for i in range(1, steps):
        ex = x + int(shadow_offset[0] * i / steps)
        ey = y + int(shadow_offset[1] * i / steps)
        draw.text((ex, ey), text, font=font, fill=extrude_color, stroke_width=stroke_width, stroke_fill=stroke_color)
    # Front face
    draw.text((x, y), text, font=font, fill=fill_color, stroke_width=stroke_width, stroke_fill=stroke_color)

def create_banner():
    # 493 x 58
    w, h = 493, 58
    img = Image.new("RGB", (w, h), color=(18, 19, 26)) # Deep Ink
    draw = ImageDraw.Draw(img)

    # Halftone dots on the right background
    draw_halftone_dots(draw, 240, 4, 490, 54, (35, 40, 58), spacing=8, radius=1)

    # Comic diagonal energy stripe
    stripe = [(330, 0), (370, 0), (320, h), (280, h)]
    draw.polygon(stripe, fill=(255, 204, 0), outline=(0, 0, 0)) # Pop Yellow
    stripe2 = [(375, 0), (395, 0), (345, h), (325, h)]
    draw.polygon(stripe2, fill=(255, 51, 102), outline=(0, 0, 0)) # Comic Pink
    stripe3 = [(400, 0), (430, 0), (380, h), (350, h)]
    draw.polygon(stripe3, fill=(0, 180, 216), outline=(0, 0, 0)) # Cyber Cyan

    # Top & bottom black cel border
    draw.line([(0, 0), (w, 0)], fill=(0, 0, 0), width=3)
    draw.line([(0, h - 2), (w, h - 2)], fill=(0, 0, 0), width=3)

    # Load default or fallback font
    try:
        font_large = ImageFont.truetype("arialbd.ttf", 26)
        font_badge = ImageFont.truetype("arialbd.ttf", 11)
    except Exception:
        font_large = ImageFont.load_default()
        font_badge = font_large

    # Cel-shaded title
    draw_cel_shaded_text(draw, (18, 12), "IDE1.0 FAST", font_large, (255, 204, 0), shadow_offset=(3, 3), stroke_width=2)

    # Cel-shaded badge: "3渲2 CARTOON"
    badge_x, badge_y, badge_w, badge_h = 190, 16, 100, 24
    draw.rectangle([badge_x + 2, badge_y + 2, badge_x + badge_w + 2, badge_y + badge_h + 2], fill=(0, 0, 0))
    draw.rectangle([badge_x, badge_y, badge_x + badge_w, badge_y + badge_h], fill=(255, 51, 102), outline=(0, 0, 0), width=2)
    draw.text((badge_x + 8, badge_y + 4), "CEL-SHADED", font=font_badge, fill=(255, 255, 255), stroke_width=1, stroke_fill=(0, 0, 0))

    # Greek Golden Laurel Leaf Branch Motif (Minimalist Comic Style)
    def draw_laurel_branch(cx, cy, scale=1.0, flip=False):
        sign = -1 if flip else 1
        # Stem arc
        stem = [(cx + int(sign * 2 * i), cy - 20 + i * 5) for i in range(9)]
        draw.line(stem, fill=(0, 0, 0), width=3)
        draw.line([(x + sign, y) for (x, y) in stem], fill=(245, 158, 11), width=2)
        # Leaves
        for i, (lx, ly) in enumerate(stem[1:-1]):
            leaf_dx = int(sign * (10 + (i % 2) * 4) * scale)
            leaf_dy = int(-4 * scale)
            leaf_poly = [(lx, ly), (lx + leaf_dx, ly + leaf_dy), (lx + leaf_dx * 1.3, ly), (lx + leaf_dx * 0.7, ly + 4)]
            draw.polygon(leaf_poly, fill=(255, 204, 0), outline=(0, 0, 0))

    draw_laurel_branch(450, 28, scale=0.9, flip=False)
    draw_laurel_branch(462, 28, scale=0.9, flip=True)

    return img

def create_dialog():
    # 493 x 312
    w, h = 493, 312
    img = Image.new("RGB", (w, h), color=(248, 249, 250)) # Comic Paper on right side
    draw = ImageDraw.Draw(img)

    # Left panel: 0..164 is the illustration zone
    panel_w = 164
    draw.rectangle([0, 0, panel_w, h], fill=(22, 24, 34)) # Dark Pop Ink

    # Dynamic diagonal halftone background on left
    draw_halftone_dots(draw, 4, 4, panel_w - 4, h - 4, (42, 45, 64), spacing=10, radius=2)

    # Dramatic comic action burst rays
    ray_origin = (panel_w // 2, 90)
    for i in range(12):
        angle = i * (2 * math.pi / 12)
        end_x = ray_origin[0] + int(140 * math.cos(angle))
        end_y = ray_origin[1] + int(140 * math.sin(angle))
        color = (255, 204, 0) if i % 3 == 0 else ((255, 51, 102) if i % 3 == 1 else (0, 180, 216))
        # narrow ray triangle
        dx = int(6 * math.sin(angle))
        dy = int(6 * -math.cos(angle))
        draw.polygon([ray_origin, (end_x + dx, end_y + dy), (end_x - dx, end_y - dy)], fill=color)

    # Overprint dark ring to focus center
    draw.ellipse([ray_origin[0] - 55, ray_origin[1] - 55, ray_origin[0] + 55, ray_origin[1] + 55],
                 fill=(22, 24, 34), outline=(0, 0, 0), width=3)

    # Fonts
    try:
        font_hero = ImageFont.truetype("arialbd.ttf", 28)
        font_sub = ImageFont.truetype("arialbd.ttf", 15)
        font_tag = ImageFont.truetype("arialbd.ttf", 11)
    except Exception:
        font_hero = ImageFont.load_default()
        font_sub = font_hero
        font_tag = font_hero

    # Center Logo in Hero circle
    draw_cel_shaded_text(draw, (ray_origin[0] - 46, ray_origin[1] - 22), "IDE1.0", font_hero, (255, 204, 0), shadow_offset=(3, 3), stroke_width=2)

    # Greek golden laurel leaves ornamenting the hero circle
    for deg in [-60, -30, 0, 30, 60]:
        rad = math.radians(deg)
        lx1 = int(ray_origin[0] - 68 * math.cos(rad))
        ly1 = int(ray_origin[1] + 68 * math.sin(rad))
        draw_greek_laurel_leaf(draw, lx1, ly1, 10, deg + 90, fill_color=(255, 204, 0))
        lx2 = int(ray_origin[0] + 68 * math.cos(rad))
        ly2 = int(ray_origin[1] + 68 * math.sin(rad))
        draw_greek_laurel_leaf(draw, lx2, ly2, 10, -deg - 90, fill_color=(255, 204, 0))

    # Banner tags in left panel
    # Box 1: "FAST EDITION"
    b1_x, b1_y, b1_w, b1_h = 16, 175, 132, 28
    draw.rectangle([b1_x + 3, b1_y + 3, b1_x + b1_w + 3, b1_y + b1_h + 3], fill=(0, 0, 0))
    draw.rectangle([b1_x, b1_y, b1_x + b1_w, b1_y + b1_h], fill=(255, 204, 0), outline=(0, 0, 0), width=2)
    draw.text((b1_x + 12, b1_y + 5), "FAST EDITION", font=font_sub, fill=(0, 0, 0), stroke_width=1, stroke_fill=(0, 0, 0))

    # Box 2: "美漫 3渲2 卡通渲染"
    b2_x, b2_y, b2_w, b2_h = 16, 215, 132, 26
    draw.rectangle([b2_x + 3, b2_y + 3, b2_x + b2_w + 3, b2_y + b2_h + 3], fill=(0, 0, 0))
    draw.rectangle([b2_x, b2_y, b2_x + b2_w, b2_y + b2_h], fill=(255, 51, 102), outline=(0, 0, 0), width=2)
    draw.text((b2_x + 10, b2_y + 5), "3D-TO-2D COMIC", font=font_tag, fill=(255, 255, 255), stroke_width=1, stroke_fill=(0, 0, 0))

    # Box 3: "RESIDENT TRAY"
    b3_x, b3_y, b3_w, b3_h = 16, 248, 132, 22
    draw.rectangle([b3_x + 2, b3_y + 2, b3_x + b3_w + 2, b3_y + b3_h + 2], fill=(0, 0, 0))
    draw.rectangle([b3_x, b3_y, b3_x + b3_w, b3_y + b3_h], fill=(0, 180, 216), outline=(0, 0, 0), width=2)
    draw.text((b3_x + 8, b3_y + 3), "⚡ RESIDENT TRAY", font=font_tag, fill=(0, 0, 0), stroke_width=1, stroke_fill=(0, 0, 0))

    # Thick black divider line separating panels
    draw.line([(panel_w, 0), (panel_w, h)], fill=(0, 0, 0), width=3)
    draw.line([(panel_w + 3, 0), (panel_w + 3, h)], fill=(255, 204, 0), width=2)

    # Right side: very subtle pop art comic dots in top-right corner
    draw_halftone_dots(draw, 420, 10, 485, 80, (230, 233, 238), spacing=12, radius=2)

    # Outer border
    draw.rectangle([0, 0, w - 1, h - 1], outline=(0, 0, 0), width=2)

    return img

if __name__ == "__main__":
    banner = create_banner()
    banner.save("E:/IDE1.0/ide-fast/packaging/banner.bmp", "BMP")
    print("Saved E:/IDE1.0/ide-fast/packaging/banner.bmp (493x58)")

    dialog = create_dialog()
    dialog.save("E:/IDE1.0/ide-fast/packaging/dialog.bmp", "BMP")
    print("Saved E:/IDE1.0/ide-fast/packaging/dialog.bmp (493x312)")
