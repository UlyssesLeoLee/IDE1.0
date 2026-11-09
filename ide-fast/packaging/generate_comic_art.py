#!/usr/bin/env python3
"""
Generate Streamlined 3D-to-2D Cel-Shaded Installer Graphics for WiX:
- banner.bmp: 493 x 58 pixels (24-bit RGB BMP)
- dialog.bmp: 493 x 312 pixels (24-bit RGB BMP)

Design style:
- Authentic 3D-to-2D Cel-Shaded Pop Comic palette (#161824, #1f2230, #ffcc00)
- Official 3D diamond crystal logo at top-left with Greek meander / laurel gold accents
- Bold 2.0px pure black ink outlines and hard drop shadows
- Halftone cyan Ben-Day screentone dots
"""

import os
import re
from PIL import Image, ImageDraw, ImageFont

ROOT = os.path.dirname(os.path.abspath(__file__))
REPO_ROOT = os.path.abspath(os.path.join(ROOT, "..", ".."))
LOGO_PATH = os.path.join(ROOT, "logo-256.png")

def get_version():
    cargo_toml = os.path.join(REPO_ROOT, "Cargo.toml")
    if os.path.exists(cargo_toml):
        with open(cargo_toml, "r", encoding="utf-8") as f:
            m = re.search(r'version\s*=\s*"([0-9.]+)"', f.read())
            if m:
                return m.group(1)
    return "0.6.6"

VERSION = get_version()

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

def draw_halftone_dots(draw, x0, y0, x1, y1, dot_color=(0, 240, 255), spacing=12, radius=1):
    for y in range(y0, y1, spacing):
        for x in range(x0, x1, spacing):
            cx = x + (spacing // 2 if (y // spacing) % 2 == 1 else 0)
            if cx <= x1:
                draw.ellipse([cx - radius, y - radius, cx + radius, y + radius], fill=dot_color)

def create_banner():
    # 493 x 58
    w, h = 493, 58
    img = Image.new("RGB", (w, h), color=(22, 24, 36)) # Comic paper #161824
    draw = ImageDraw.Draw(img)

    # Subtle cyan Ben-Day halftone dots on the right
    draw_halftone_dots(draw, 220, 6, w - 10, h - 6, dot_color=(32, 50, 75), spacing=10, radius=1)

    # Top-left Logo
    if os.path.exists(LOGO_PATH):
        logo = Image.open(LOGO_PATH).convert("RGBA").resize((32, 32), Image.Resampling.LANCZOS)
        # Gold highlight ring
        draw.ellipse([12, 11, 48, 47], fill=(255, 204, 0, 40))
        img.paste(logo, (14, 13), logo)

    # Title & Subtitle next to logo
    font_title = get_font(14, bold=True)
    font_ver = get_font(10, bold=True)

    draw.text((54, 13), "IDE 1.0 Fast Edition", font=font_title, fill=(248, 250, 252))
    draw.text((54, 32), "3D Cel-Shaded Comic · High-Performance Editor", font=font_ver, fill=(148, 163, 184))

    # Version comic badge on top-right
    pill_w, pill_h = 64, 22
    px, py = w - 80, 18
    # Solid black drop shadow
    draw.rectangle([px + 2, py + 2, px + pill_w + 2, py + pill_h + 2], fill=(0, 0, 0))
    # Pop yellow fill with black border
    draw.rectangle([px, py, px + pill_w, py + pill_h], fill=(255, 204, 0), outline=(0, 0, 0), width=2)
    draw.text((px + 10, py + 4), f"v{VERSION}", font=font_ver, fill=(0, 0, 0))

    # Bottom 2px solid black ink border
    draw.line([(0, h - 2), (w, h - 2)], fill=(0, 0, 0), width=2)
    return img

def create_dialog():
    # 493 x 312
    w, h = 493, 312
    img = Image.new("RGB", (w, h), color=(22, 24, 36)) # Comic paper #161824
    draw = ImageDraw.Draw(img)

    # Halftone background dots
    draw_halftone_dots(draw, 10, 52, w - 10, h - 10, dot_color=(28, 38, 56), spacing=12, radius=1)

    # Header Bar (Top 46px): #1f2230
    draw.rectangle([0, 0, w, 46], fill=(31, 34, 48))
    draw.line([(0, 46), (w, 46)], fill=(0, 0, 0), width=2)

    # Top-Left Official 3D Diamond Crystal Logo
    if os.path.exists(LOGO_PATH):
        logo = Image.open(LOGO_PATH).convert("RGBA").resize((32, 32), Image.Resampling.LANCZOS)
        # Soft cyan glow ring
        draw.ellipse([10, 3, 50, 43], fill=(0, 240, 255, 30))
        img.paste(logo, (14, 7), logo)

    font_head = get_font(13, bold=True)
    font_ver = get_font(10, bold=True)
    draw.text((54, 14), "IDE 1.0 Fast Edition · 安装向导", font=font_head, fill=(248, 250, 252))

    # Top-Right Controls & Version Badge
    badge_w, badge_h = 60, 22
    bx, by = w - 110, 12
    draw.rectangle([bx + 1, by + 1, bx + badge_w + 1, by + badge_h + 1], fill=(0, 0, 0))
    draw.rectangle([bx, by, bx + badge_w, by + badge_h], fill=(255, 204, 0), outline=(0, 0, 0), width=1)
    draw.text((bx + 10, by + 4), f"v{VERSION}", font=font_ver, fill=(0, 0, 0))

    # Window control dots (Comic Cel-Shaded)
    draw.ellipse([w - 40, 19, w - 32, 27], fill=(0, 245, 155), outline=(0, 0, 0), width=1) # green
    draw.ellipse([w - 28, 19, w - 20, 27], fill=(255, 204, 0), outline=(0, 0, 0), width=1) # yellow
    draw.ellipse([w - 16, 19, w - 8, 27], fill=(255, 51, 102), outline=(0, 0, 0), width=1)  # red

    # Center Hero Comic Panel
    panel_x0, panel_y0, panel_x1, panel_y1 = 14, 56, w - 14, 122
    # Hard drop shadow
    draw.rectangle([panel_x0 + 2, panel_y0 + 2, panel_x1 + 2, panel_y1 + 2], fill=(0, 0, 0))
    # Comic cell fill
    draw.rectangle([panel_x0, panel_y0, panel_x1, panel_y1], fill=(32, 35, 49), outline=(0, 0, 0), width=2)

    font_hero = get_font(15, bold=True)
    font_desc = get_font(11, bold=False)
    draw.text((28, 66), "⚡ 欢迎使用 IDE 1.0 FAST EDITION", font=font_hero, fill=(255, 204, 0))
    draw.text((28, 93), "Rust 极速微内核 · 多 Agent 协同架构 · 美漫 3渲2 卡通渲染 · 纯粹编码体验", font=font_desc, fill=(148, 163, 184))

    # 3 Features Highlights (Horizontal Chips)
    chips = [
        ("⚡ 毫秒级极速冷启动", (0, 240, 255)),
        ("🛡️ 系统托盘常驻守护", (255, 204, 0)),
        ("🔤 纯净像素点阵字形", (0, 245, 155)),
    ]
    chip_x = 14
    chip_y = 136
    chip_w = (w - 28 - 16) // 3
    for text, color in chips:
        # Hard shadow
        draw.rectangle([chip_x + 1, chip_y + 1, chip_x + chip_w + 1, chip_y + 31], fill=(0, 0, 0))
        # Card
        draw.rectangle([chip_x, chip_y, chip_x + chip_w, chip_y + 30], fill=(30, 32, 45), outline=(0, 0, 0), width=1)
        # Top color accent strip
        draw.rectangle([chip_x, chip_y, chip_x + chip_w, chip_y + 3], fill=color)
        font_chip = get_font(10, bold=True)
        draw.text((chip_x + 8, chip_y + 9), text, font=font_chip, fill=(241, 245, 249))
        chip_x += chip_w + 8

    # Outer 2.5px solid black border
    draw.rectangle([0, 0, w - 1, h - 1], outline=(0, 0, 0), width=2)
    return img

if __name__ == "__main__":
    banner = create_banner()
    banner.save(os.path.join(ROOT, "banner.bmp"), "BMP")
    print(f"Saved {os.path.join(ROOT, 'banner.bmp')} (493x58) [v{VERSION}]")

    dialog = create_dialog()
    dialog.save(os.path.join(ROOT, "dialog.bmp"), "BMP")
    print(f"Saved {os.path.join(ROOT, 'dialog.bmp')} (493x312) [v{VERSION}]")
