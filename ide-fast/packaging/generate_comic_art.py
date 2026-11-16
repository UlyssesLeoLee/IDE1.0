#!/usr/bin/env python3
"""
Generate Authentic American Comic & Cel-Shaded Pop-Art Installer Graphics for WiX:
- banner.bmp: 493 x 58 pixels (24-bit RGB BMP)
- dialog.bmp: 493 x 312 pixels (24-bit RGB BMP)

Aesthetic Features:
- Bold Comic Inking: Thick black ink outlines (2.0px ~ 3.0px solid #000000)
- Ben-Day Screentone Dots: Authentic pop-art halftone dot matrix patterns
- Two-Tone Cel Shading: Crisp stepped light/shadow without soft blurry gradients
- Action Burst Stars & Speech Balloons: Spiky "POW!", "FAST!" stickers and dialog bubbles
- Hard Offset Drop Shadows: Pure black offset rectangular blocks ([3, 3] / [4, 4])
- Vibrant Comic Palette: Comic Yellow (#FFE600), Action Cyan (#00E5FF), Punchy Red (#FF2A55), Deep Comic Navy (#141726)
"""

import os
import re
import math
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
    return "0.6.9"

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

def draw_ben_day_dots(draw, x0, y0, x1, y1, dot_color=(0, 229, 255), spacing=10, radius=1.2):
    """Draws pop-art Ben-Day halftone screen dots"""
    for y in range(int(y0), int(y1), int(spacing)):
        offset = (spacing // 2) if ((y // spacing) % 2 == 1) else 0
        for x in range(int(x0) + int(offset), int(x1), int(spacing)):
            draw.ellipse([x - radius, y - radius, x + radius, y + radius], fill=dot_color)

def draw_comic_panel(draw, rect, fill_color, outline_color=(0, 0, 0), outline_width=2, shadow_offset=(3, 3)):
    """Draws a comic book panel card with a solid black hard drop shadow"""
    x0, y0, x1, y1 = [int(v) for v in rect]
    sx, sy = int(shadow_offset[0]), int(shadow_offset[1])
    outline_w = int(max(1, round(outline_width)))
    # 1. Hard black drop shadow
    if sx > 0 or sy > 0:
        draw.rectangle([x0 + sx, y0 + sy, x1 + sx, y1 + sy], fill=(0, 0, 0))
    # 2. Main panel body
    draw.rectangle([x0, y0, x1, y1], fill=fill_color, outline=outline_color, width=outline_w)

def draw_burst_star(draw, center, r_outer, r_inner, points=10, fill_color=(255, 230, 0), outline_color=(0, 0, 0), shadow_offset=(2, 2)):
    """Draws a spiky comic action burst star with hard drop shadow"""
    cx, cy = center
    poly = []
    angle_step = math.pi / points
    for i in range(2 * points):
        r = r_outer if i % 2 == 0 else r_inner
        angle = i * angle_step - math.pi / 2
        poly.append((cx + r * math.cos(angle), cy + r * math.sin(angle)))

    # Hard shadow
    if shadow_offset[0] > 0:
        shadow_poly = [(px + shadow_offset[0], py + shadow_offset[1]) for px, py in poly]
        draw.polygon(shadow_poly, fill=(0, 0, 0))

    # Main Star
    draw.polygon(poly, fill=fill_color, outline=outline_color)

def draw_authentic_logo(img, center, size):
    """Puts the user's authentic IDE1.0 Crystal Logo onto the image with sharp rendering and comic offset shadow"""
    cx, cy = int(center[0]), int(center[1])
    s = int(size)
    if os.path.exists(LOGO_PATH):
        try:
            logo = Image.open(LOGO_PATH).convert("RGBA")
            logo = logo.resize((s, s), Image.Resampling.LANCZOS)
            x = cx - s // 2
            y = cy - s // 2
            # 纯黑美漫硬投影 (Hard Comic Drop Shadow behind logo)
            shadow_mask = logo.split()[3]
            img.paste((0, 0, 0), (x + 3, y + 3), shadow_mask)
            # 正式用户真实 Logo 贴附
            img.paste(logo, (x, y), logo)
            return
        except Exception as e:
            print(f"Error loading authentic logo: {e}")

def create_comic_banner():
    # 493 x 58 WiX Header Banner
    w, h = 493, 58
    img = Image.new("RGB", (w, h), color=(20, 23, 38)) # Comic Night Navy
    draw = ImageDraw.Draw(img)

    # Pop-art Ben-Day dots in background
    draw_ben_day_dots(draw, 10, 4, w - 10, h - 4, dot_color=(38, 48, 76), spacing=10, radius=1.2)

    # Dynamic angled Comic Yellow action stripe in background
    draw.polygon([(180, 0), (250, 0), (220, h), (150, h)], fill=(255, 230, 0))
    draw.line([(180, 0), (150, h)], fill=(0, 0, 0), width=2)
    draw.line([(250, 0), (220, h)], fill=(0, 0, 0), width=2)

    # Left Authentic Crystal Logo
    draw_authentic_logo(img, (32, 29), 36)

    # Comic Title with 3D drop shadow
    font_title = get_font(14, bold=True)
    font_sub = get_font(9, bold=True)

    # Shadow
    draw.text((64, 11), "IDE 1.0", font=font_title, fill=(0, 0, 0))
    # Fill
    draw.text((62, 9), "IDE 1.0", font=font_title, fill=(255, 255, 255))

    # Comic Burst sticker for FAST EDITION
    draw_burst_star(draw, (150, 20), 16, 10, points=8, fill_color=(255, 42, 85), outline_color=(0, 0, 0), shadow_offset=(2, 2))
    draw.text((135, 14), "FAST!", font=get_font(8, bold=True), fill=(255, 255, 255))

    # Subtitle with comic speech frame
    draw.text((62, 32), "★ RUST NATIVE MICROKERNEL · MULTI-AGENT WORKSPACE ★", font=font_sub, fill=(0, 229, 255))

    # Right side Action Version Badge
    draw_comic_panel(draw, [w - 92, 12, w - 14, 42], fill_color=(255, 230, 0), outline_color=(0, 0, 0), outline_width=2, shadow_offset=(2, 2))
    draw.text((w - 82, 19), f"★ v{VERSION}", font=get_font(10, bold=True), fill=(0, 0, 0))

    # 2.5px solid black border around entire banner
    draw.rectangle([0, 0, w - 1, h - 1], outline=(0, 0, 0), width=2)
    return img

def create_comic_dialog():
    # 493 x 312 WiX Background Dialog
    w, h = 493, 312
    img = Image.new("RGB", (w, h), color=(18, 21, 35)) # Deep comic indigo
    draw = ImageDraw.Draw(img)

    # 1. Background Halftone Screen Pattern (Ben-Day dots)
    draw_ben_day_dots(draw, 8, 48, w - 8, h - 8, dot_color=(34, 42, 68), spacing=12, radius=1.2)

    # 2. Top Comic Title Bar (Comic Yellow with black ink border)
    header_h = 44
    draw.rectangle([0, 0, w, header_h], fill=(255, 230, 0), outline=(0, 0, 0), width=2)
    # Diagonal comic speed stripes on header
    for sx in range(12, 160, 16):
        draw.line([(sx, 0), (sx + 20, header_h)], fill=(240, 210, 0), width=4)
        draw.line([(sx, 0), (sx + 20, header_h)], fill=(0, 0, 0), width=1)

    draw_authentic_logo(img, (24, header_h // 2), 26)

    font_head = get_font(13, bold=True)
    draw.text((46, 12), "IDE 1.0  FAST EDITION · SETUP WIZARD", font=font_head, fill=(0, 0, 0))

    # Version Action Tag on Top Right
    draw_comic_panel(draw, [w - 86, 9, w - 14, 34], fill_color=(255, 42, 85), outline_color=(0, 0, 0), outline_width=2, shadow_offset=(2, 2))
    draw.text((w - 76, 14), f"v{VERSION} ★", font=get_font(9, bold=True), fill=(255, 255, 255))

    # 3. Main Center Split Comic Panels
    # Left Hero Stage Panel (Ionic pedestal + Floating Core + Speech Bubble)
    left_w = 148
    draw_comic_panel(draw, [12, 54, 12 + left_w, 296], fill_color=(24, 28, 46), outline_color=(0, 0, 0), outline_width=2, shadow_offset=(3, 3))
    # Left panel Ben-Day dots
    draw_ben_day_dots(draw, 14, 56, 12 + left_w - 2, 294, dot_color=(0, 229, 255), spacing=8, radius=1.0)

    # Radiating Burst Rays from center
    core_cx = 12 + left_w // 2
    core_cy = 135
    for a in range(0, 360, 30):
        rad = math.radians(a)
        rx = core_cx + 48 * math.cos(rad)
        ry = core_cy + 48 * math.sin(rad)
        draw.line([(core_cx, core_cy), (rx, ry)], fill=(255, 230, 0), width=1)

    # Authentic Crystal Logo
    draw_authentic_logo(img, (core_cx, core_cy), 52)

    # Comic Pedestal (Ionic marble style in 2-tone cel shading)
    ped_top = 180
    # Capital
    draw_comic_panel(draw, [core_cx - 44, ped_top, core_cx + 44, ped_top + 14], fill_color=(240, 240, 245), outline_color=(0, 0, 0), outline_width=2, shadow_offset=(0, 0))
    # Volute circles
    draw.ellipse([core_cx - 42, ped_top + 1, core_cx - 30, ped_top + 13], outline=(0, 0, 0), width=1)
    draw.ellipse([core_cx + 30, ped_top + 1, core_cx + 42, ped_top + 13], outline=(0, 0, 0), width=1)
    # Shaft with flutes
    draw_comic_panel(draw, [core_cx - 36, ped_top + 14, core_cx + 36, ped_top + 64], fill_color=(220, 225, 235), outline_color=(0, 0, 0), outline_width=2, shadow_offset=(0, 0))
    for fx in [-24, -12, 0, 12, 24]:
        draw.line([(core_cx + fx, ped_top + 16), (core_cx + fx, ped_top + 62)], fill=(160, 170, 190), width=2)
        draw.line([(core_cx + fx - 1, ped_top + 16), (core_cx + fx - 1, ped_top + 62)], fill=(0, 0, 0), width=1)
    # Base
    draw_comic_panel(draw, [core_cx - 48, ped_top + 64, core_cx + 48, ped_top + 78], fill_color=(245, 245, 250), outline_color=(0, 0, 0), outline_width=2, shadow_offset=(0, 0))
    # Gold Greek key accent strip
    draw.rectangle([core_cx - 44, ped_top + 68, core_cx + 44, ped_top + 74], fill=(255, 204, 0), outline=(0, 0, 0), width=1)

    # Left Panel Bottom Sticker
    draw_comic_panel(draw, [core_cx - 56, 268, core_cx + 56, 290], fill_color=(255, 42, 85), outline_color=(0, 0, 0), outline_width=2, shadow_offset=(2, 2))
    draw.text((core_cx - 44, 272), "100% NATIVE", font=get_font(9, bold=True), fill=(255, 255, 255))

    # Right Content Area (Comic Action Cards)
    rx0 = 12 + left_w + 14
    rx1 = w - 12

    # Hero Speech Banner (Top Right)
    draw_comic_panel(draw, [rx0, 54, rx1, 116], fill_color=(255, 255, 255), outline_color=(0, 0, 0), outline_width=2, shadow_offset=(3, 3))
    # Speech bubble tail pointing left
    draw.polygon([(rx0, 78), (rx0 - 8, 86), (rx0, 94)], fill=(255, 255, 255), outline=(0, 0, 0))
    draw.line([(rx0, 79), (rx0, 93)], fill=(255, 255, 255), width=2)

    font_hero = get_font(13, bold=True)
    font_desc = get_font(9, bold=False)
    draw.text((rx0 + 12, 62), "⚡ 开启全新极速编码冒险！", font=font_hero, fill=(0, 0, 0))
    draw.text((rx0 + 12, 84), "纯 Rust 极速内核 · 多 Agent 协同生态 · 零卡顿全真透视体验", font=font_desc, fill=(40, 45, 60))
    draw_burst_star(draw, (rx1 - 22, 75), 14, 8, points=6, fill_color=(255, 230, 0), outline_color=(0, 0, 0), shadow_offset=(1, 1))
    draw.text((rx1 - 32, 70), "POW!", font=get_font(7, bold=True), fill=(0, 0, 0))

    # 3 Comic Feature Cards (Stacked Action Cards)
    cards = [
        ("⚡ 毫秒极速冷启", "原生编译无运行时开销，即开即写秒速响应", (0, 229, 255), "01"),
        ("🤖 多 Agent 协同生态", "空间不相交规则化编辑，补丁原子级隔离应用", (255, 204, 0), "02"),
        ("💎 3D美漫与晶透渲染", "美漫赛璐珞与高透晶面渲染，视觉风格随心切换", (16, 185, 129), "03"),
    ]
    card_y = 126
    for title, desc, accent, num in cards:
        draw_comic_panel(draw, [rx0, card_y, rx1, card_y + 44], fill_color=(24, 29, 48), outline_color=(0, 0, 0), outline_width=2, shadow_offset=(2, 2))
        # Left accent stripe
        draw.rectangle([rx0, card_y, rx0 + 5, card_y + 44], fill=accent)
        # Action number badge
        draw_comic_panel(draw, [rx0 + 12, card_y + 10, rx0 + 36, card_y + 34], fill_color=accent, outline_color=(0, 0, 0), outline_width=1, shadow_offset=(1, 1))
        draw.text((rx0 + 18, card_y + 13), num, font=get_font(10, bold=True), fill=(0, 0, 0))

        draw.text((rx0 + 44, card_y + 6), title, font=get_font(10, bold=True), fill=(255, 255, 255))
        draw.text((rx0 + 44, card_y + 24), desc, font=get_font(8, bold=False), fill=(160, 175, 200))

        # Comic Ready checkmark tag
        draw_comic_panel(draw, [rx1 - 58, card_y + 12, rx1 - 8, card_y + 32], fill_color=(255, 230, 0), outline_color=(0, 0, 0), outline_width=1, shadow_offset=(1, 1))
        draw.text((rx1 - 50, card_y + 15), "★ READY", font=get_font(7, bold=True), fill=(0, 0, 0))

        card_y += 50

    # Bottom Footer Strip
    draw.line([(0, h - 22), (w, h - 22)], fill=(0, 0, 0), width=2)
    draw.rectangle([0, h - 20, w, h], fill=(255, 230, 0))
    draw.text((12, h - 17), "★ AGPL-3.0 OPEN SOURCE · PURE NATIVE · NO BLOAT · NO ADS ★", font=get_font(8, bold=True), fill=(0, 0, 0))

    # Outer 2.5px solid black border around entire dialog
    draw.rectangle([0, 0, w - 1, h - 1], outline=(0, 0, 0), width=2)
    return img

if __name__ == "__main__":
    banner = create_comic_banner()
    banner.save(os.path.join(ROOT, "banner.bmp"), "BMP")
    print(f"Generated Comic Banner: {os.path.join(ROOT, 'banner.bmp')} (493x58) [v{VERSION}]")

    dialog = create_comic_dialog()
    dialog.save(os.path.join(ROOT, "dialog.bmp"), "BMP")
    print(f"Generated Comic Dialog: {os.path.join(ROOT, 'dialog.bmp')} (493x312) [v{VERSION}]")
