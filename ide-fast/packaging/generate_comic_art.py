#!/usr/bin/env python3
"""
Generate Streamlined 3D-to-2D Cel-Shaded Installer Graphics for WiX:
- banner.bmp: 493 x 58 pixels (24-bit RGB BMP)
- dialog.bmp: 493 x 312 pixels (24-bit RGB BMP)

Design style:
- No left picture area (clean, modern full-width layout)
- Official 3D diamond crystal logo placed at top-left
- Cel-shaded dark cyber slate and pop gold accents
- Halftone cyan Ben-Day dots
"""

import os
from PIL import Image, ImageDraw, ImageFont

ROOT = os.path.dirname(os.path.abspath(__file__))
LOGO_PATH = os.path.join(ROOT, "logo-256.png")

def get_font(size, bold=True):
    fonts_to_try = [
        "msyhbd.ttc" if bold else "msyh.ttc",
        "arialbd.ttf" if bold else "arial.ttf",
        "segoeuib.ttf" if bold else "segoeui.ttf",
    ]
    for fn in fonts_to_try:
        try:
            return ImageFont.truetype(fn, size)
        except Exception:
            continue
    return ImageFont.load_default()

def draw_halftone_dots(draw, x0, y0, x1, y1, dot_color=(6, 182, 212), spacing=12, radius=1):
    for y in range(y0, y1, spacing):
        for x in range(x0, x1, spacing):
            cx = x + (spacing // 2 if (y // spacing) % 2 == 1 else 0)
            if cx <= x1:
                draw.ellipse([cx - radius, y - radius, cx + radius, y + radius], fill=dot_color)

def create_banner():
    # 493 x 58
    w, h = 493, 58
    img = Image.new("RGB", (w, h), color=(15, 23, 42)) # Slate 900
    draw = ImageDraw.Draw(img)

    # Halftone dots across the right background
    draw_halftone_dots(draw, 240, 4, w - 8, h - 4, dot_color=(20, 40, 65), spacing=10, radius=1)

    # Top-left Logo
    if os.path.exists(LOGO_PATH):
        logo = Image.open(LOGO_PATH).convert("RGBA").resize((32, 32), Image.Resampling.LANCZOS)
        # Paste with alpha
        img.paste(logo, (14, 13), logo)

    # Title next to logo
    font_title = get_font(14, bold=True)
    font_ver = get_font(11, bold=True)

    draw.text((54, 14), "IDE 1.0 Fast Edition", font=font_title, fill=(248, 250, 252))
    draw.text((54, 32), "Windows x64 · High-Performance Editor", font=font_ver, fill=(148, 163, 184))

    # Version pill on top-right
    pill_w, pill_h = 60, 20
    px, py = w - 76, 19
    draw.rectangle([px, py, px + pill_w, py + pill_h], fill=(30, 41, 59), outline=(6, 182, 212), width=1)
    draw.text((px + 12, py + 3), "v0.6.4", font=font_ver, fill=(6, 182, 212))

    # Bottom hairline border
    draw.line([(0, h - 1), (w, h - 1)], fill=(30, 41, 59), width=1)
    draw.line([(0, h - 2), (w, h - 2)], fill=(0, 0, 0), width=1)

    return img

def create_dialog():
    # 493 x 312
    w, h = 493, 312
    img = Image.new("RGB", (w, h), color=(9, 13, 22)) # Cyber Dark Slate
    draw = ImageDraw.Draw(img)

    # Halftone background dots
    draw_halftone_dots(draw, 10, 50, w - 10, h - 10, dot_color=(15, 30, 50), spacing=12, radius=1)

    # Header Bar (Top 46px)
    draw.rectangle([0, 0, w, 46], fill=(15, 23, 42))
    draw.line([(0, 46), (w, 46)], fill=(30, 41, 59), width=1)

    # Top-Left Official 3D Diamond Crystal Logo
    if os.path.exists(LOGO_PATH):
        logo = Image.open(LOGO_PATH).convert("RGBA").resize((32, 32), Image.Resampling.LANCZOS)
        # Glow ring behind logo
        draw.ellipse([14 - 4, 7 - 4, 14 + 36, 7 + 36], fill=(2, 132, 199, 60))
        img.paste(logo, (14, 7), logo)

    font_head = get_font(13, bold=True)
    font_ver = get_font(11, bold=True)
    draw.text((54, 14), "IDE 1.0 Fast Edition · 安装向导", font=font_head, fill=(248, 250, 252))

    # Top-Right Controls & Version
    draw.rectangle([w - 110, 13, w - 50, 33], fill=(30, 41, 59), outline=(6, 182, 212), width=1)
    draw.text((w - 98, 16), "v0.6.4", font=font_ver, fill=(6, 182, 212))

    # Window control dots
    draw.ellipse([w - 40, 20, w - 32, 28], fill=(16, 185, 129)) # green
    draw.ellipse([w - 28, 20, w - 20, 28], fill=(245, 158, 11)) # yellow
    draw.ellipse([w - 16, 20, w - 8, 28], fill=(239, 68, 68))   # red

    # Center Hero Box
    draw.rectangle([14, 56, w - 14, 120], fill=(15, 23, 42), outline=(30, 41, 59), width=1)
    font_hero = get_font(16, bold=True)
    font_desc = get_font(11, bold=False)
    draw.text((28, 66), "欢迎使用 IDE 1.0 FAST EDITION", font=font_hero, fill=(248, 250, 252))
    draw.text((28, 92), "Rust 极速微内核 · 多 Agent 协同架构 · 纯粹编码体验", font=font_desc, fill=(148, 163, 184))

    # 3 Features Highlights (Horizontal Chips)
    chips = [
        ("• 毫秒级极速冷启动", (6, 182, 212)),
        ("• 系统托盘常驻守护", (245, 158, 11)),
        ("• 字体防畸变净化", (16, 185, 129)),
    ]
    chip_x = 14
    chip_y = 132
    chip_w = (w - 28 - 16) // 3
    for text, color in chips:
        draw.rectangle([chip_x, chip_y, chip_x + chip_w, chip_y + 30], fill=(15, 23, 42), outline=color, width=1)
        font_chip = get_font(10, bold=True)
        draw.text((chip_x + 8, chip_y + 8), text, font=font_chip, fill=(241, 245, 249))
        chip_x += chip_w + 8

    # Outer border
    draw.rectangle([0, 0, w - 1, h - 1], outline=(0, 0, 0), width=2)

    return img

if __name__ == "__main__":
    banner = create_banner()
    banner.save(os.path.join(ROOT, "banner.bmp"), "BMP")
    print(f"Saved {os.path.join(ROOT, 'banner.bmp')} (493x58)")

    dialog = create_dialog()
    dialog.save(os.path.join(ROOT, "dialog.bmp"), "BMP")
    print(f"Saved {os.path.join(ROOT, 'dialog.bmp')} (493x312)")
