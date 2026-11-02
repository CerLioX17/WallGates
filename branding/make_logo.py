"""Generates the Wallgates brand assets (logo, icon, installer art).

Run: python3 branding/make_logo.py <fonts_dir>
Fonts: Sora (OFL) for the wordmark.
"""
import math, sys, os
from PIL import Image, ImageDraw, ImageFilter, ImageFont, ImageChops

OUT = os.path.join(os.path.dirname(__file__), "out")
os.makedirs(OUT, exist_ok=True)
FONTS = sys.argv[1] if len(sys.argv) > 1 else "."

# Brand palette
NIGHT = (8, 8, 16)
NIGHT_2 = (22, 20, 44)
CYAN = (34, 211, 238)
VIOLET = (139, 92, 246)
MAGENTA = (236, 72, 153)
WHITE = (245, 246, 255)

def lerp(a, b, t):
    return tuple(int(a[i] + (b[i] - a[i]) * t) for i in range(len(a)))

def gradient(size, stops, angle_deg=90):
    """Linear gradient over `stops` [(t, rgb)], angle 90 = top->bottom."""
    w, h = size
    img = Image.new("RGB", size)
    px = img.load()
    a = math.radians(angle_deg)
    dx, dy = math.cos(a), math.sin(a)
    proj = [x * dx + y * dy for x, y in [(0, 0), (w, 0), (0, h), (w, h)]]
    lo, hi = min(proj), max(proj)
    for y in range(h):
        for x in range(w):
            t = ((x * dx + y * dy) - lo) / (hi - lo)
            for i in range(len(stops) - 1):
                t0, c0 = stops[i]
                t1, c1 = stops[i + 1]
                if t <= t1 or i == len(stops) - 2:
                    tt = 0 if t1 == t0 else min(max((t - t0) / (t1 - t0), 0), 1)
                    px[x, y] = lerp(c0, c1, tt)
                    break
    return img

def arch_mask(size, box, radius_top=None):
    """Doorway shape: rectangle with a semicircular top."""
    m = Image.new("L", size, 0)
    d = ImageDraw.Draw(m)
    x0, y0, x1, y1 = box
    r = (x1 - x0) / 2 if radius_top is None else radius_top
    d.ellipse([x0, y0, x1, y0 + 2 * r], fill=255)
    d.rectangle([x0, y0 + r, x1, y1], fill=255)
    return m

def icon(size=1024, with_bg=True):
    S = size * 2  # supersample
    canvas = Image.new("RGBA", (S, S), (0, 0, 0, 0))

    if with_bg:
        bg = gradient((S, S), [(0, NIGHT_2), (1, NIGHT)], 90).convert("RGBA")
        mask = Image.new("L", (S, S), 0)
        ImageDraw.Draw(mask).rounded_rectangle([0, 0, S - 1, S - 1], radius=int(S * 0.225), fill=255)
        canvas.paste(bg, (0, 0), mask)

    # Gate geometry
    gx0, gx1 = int(S * 0.29), int(S * 0.71)
    gy0, gy1 = int(S * 0.20), int(S * 0.80)
    stroke = int(S * 0.055)

    # Portal light (inside the gate): aurora gradient
    inner = arch_mask((S, S), (gx0 + stroke, gy0 + stroke, gx1 - stroke, gy1))
    portal = gradient((S, S), [(0, VIOLET), (0.45, (99, 102, 241)), (0.75, CYAN), (1, (14, 116, 144))], 90).convert("RGBA")
    # Horizon waves inside portal
    waves = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    wd = ImageDraw.Draw(waves)
    for i, (yy, alpha) in enumerate([(0.55, 230), (0.635, 170), (0.72, 110)]):
        pts = [(x, S * yy + S * 0.018 * math.sin(x / S * math.pi * 4 + i * 1.3)) for x in range(gx0, gx1, 6)]
        wd.line(pts, fill=(255, 255, 255, alpha), width=int(S * 0.014))
    waves = Image.alpha_composite(waves.filter(ImageFilter.GaussianBlur(S * 0.008)), waves)
    portal = Image.alpha_composite(portal, waves)
    # Glow behind portal
    glow = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    gm = arch_mask((S, S), (gx0 - stroke, gy0 - stroke, gx1 + stroke, gy1 + stroke // 2))
    glow_color = Image.new("RGBA", (S, S), VIOLET + (255,))
    glow.paste(glow_color, (0, 0), gm)
    glow = glow.filter(ImageFilter.GaussianBlur(S * 0.06))
    glow.putalpha(glow.getchannel("A").point(lambda a: int(a * 0.8)))
    canvas = Image.alpha_composite(canvas, glow)
    canvas.paste(portal, (0, 0), inner)

    # Gate frame: gradient stroke cyan -> violet -> magenta
    outer = arch_mask((S, S), (gx0, gy0, gx1, gy1))
    hole = arch_mask((S, S), (gx0 + stroke, gy0 + stroke, gx1 - stroke, gy1 + 10))
    frame = ImageChops.subtract(outer, hole)
    frame_fill = gradient((S, S), [(0, CYAN), (0.5, VIOLET), (1, MAGENTA)], 45).convert("RGBA")
    canvas.paste(frame_fill, (0, 0), frame)

    # Threshold / floor light
    floor = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    fd = ImageDraw.Draw(floor)
    fd.ellipse([gx0 - S * 0.06, gy1 - S * 0.02, gx1 + S * 0.06, gy1 + S * 0.05], fill=CYAN + (170,))
    floor = floor.filter(ImageFilter.GaussianBlur(S * 0.02))
    canvas = Image.alpha_composite(canvas, floor)
    fd2 = ImageDraw.Draw(canvas)
    fd2.rounded_rectangle([gx0 - S * 0.04, gy1 - S * 0.008, gx1 + S * 0.04, gy1 + S * 0.012], radius=S * 0.01, fill=WHITE + (235,))

    if with_bg:
        # Clip everything to the squircle
        m = Image.new("L", (S, S), 0)
        ImageDraw.Draw(m).rounded_rectangle([0, 0, S - 1, S - 1], radius=int(S * 0.225), fill=255)
        canvas.putalpha(ImageChops.multiply(canvas.getchannel("A"), m))
    return canvas.resize((size, size), Image.LANCZOS)

def font(size, weight=700):
    f = ImageFont.truetype(os.path.join(FONTS, "Sora.ttf"), size)
    try:
        f.set_variation_by_axes([weight])
    except Exception:
        pass
    return f

def text_gradient(text, f, stops, angle=0):
    tmp = ImageDraw.Draw(Image.new("L", (1, 1)))
    l, t, r, b = tmp.textbbox((0, 0), text, font=f)
    w, h = r - l, b - t
    m = Image.new("L", (w + 4, h + 4), 0)
    ImageDraw.Draw(m).text((-l + 2, -t + 2), text, font=f, fill=255)
    g = gradient(m.size, stops, angle).convert("RGBA")
    g.putalpha(m)
    return g

def wordmark(height=200):
    f = font(int(height * 0.72), 650)
    return text_gradient("Wallgates", f, [(0, WHITE), (1, (200, 205, 255))], 0)

def lockup(width=2000, height=520, dark=True):
    img = Image.new("RGBA", (width, height), NIGHT + (255,) if dark else (0, 0, 0, 0))
    ic = icon(int(height * 0.78))
    img.alpha_composite(ic, (int(height * 0.11), int(height * 0.11)))
    wm = wordmark(int(height * 0.62))
    img.alpha_composite(wm, (int(height * 0.11) + ic.width + int(height * 0.14), (height - wm.height) // 2 - int(height * 0.02)))
    return img

def bmp(img, path):
    img.convert("RGB").save(path, format="BMP")

def installer_art():
    # NSIS sidebar 164x314 (welcome/finish pages)
    W, H = 164 * 2, 314 * 2
    side = gradient((W, H), [(0, NIGHT_2), (0.6, NIGHT), (1, (30, 16, 52))], 90).convert("RGBA")
    ic = icon(220)
    side.alpha_composite(ic, ((W - 220) // 2, 110))
    wm = text_gradient("Wallgates", font(52, 650), [(0, WHITE), (1, (205, 210, 255))])
    side.alpha_composite(wm, ((W - wm.width) // 2, 350))
    tag = text_gradient("Live wallpapers,", font(22, 400), [(0, (170, 175, 210)), (1, (170, 175, 210))])
    tag2 = text_gradient("open to everyone.", font(22, 400), [(0, (170, 175, 210)), (1, (170, 175, 210))])
    side.alpha_composite(tag, ((W - tag.width) // 2, 430))
    side.alpha_composite(tag2, ((W - tag2.width) // 2, 462))
    bar = gradient((W, 8), [(0, CYAN), (0.5, VIOLET), (1, MAGENTA)], 0).convert("RGBA")
    side.alpha_composite(bar, (0, H - 8))
    bmp(side.resize((164, 314), Image.LANCZOS), os.path.join(OUT, "nsis-sidebar.bmp"))

    # NSIS header 150x57
    hdr = Image.new("RGBA", (300, 114), (255, 255, 255, 255))
    ic = icon(84)
    hdr.alpha_composite(ic, (300 - 84 - 14, 15))
    bmp(hdr.resize((150, 57), Image.LANCZOS), os.path.join(OUT, "nsis-header.bmp"))

    # WiX dialog 493x312 (left ~164px is the art area, right is white for text)
    W, H = 493 * 2, 312 * 2
    dlg = Image.new("RGBA", (W, H), (255, 255, 255, 255))
    art = gradient((328, H), [(0, NIGHT_2), (0.6, NIGHT), (1, (30, 16, 52))], 90).convert("RGBA")
    ic = icon(200)
    art.alpha_composite(ic, ((328 - 200) // 2, 120))
    wm = text_gradient("Wallgates", font(46, 650), [(0, WHITE), (1, (205, 210, 255))])
    art.alpha_composite(wm, ((328 - wm.width) // 2, 345))
    bar = gradient((328, 8), [(0, CYAN), (0.5, VIOLET), (1, MAGENTA)], 0).convert("RGBA")
    art.alpha_composite(bar, (0, H - 8))
    dlg.alpha_composite(art, (0, 0))
    bmp(dlg.resize((493, 312), Image.LANCZOS), os.path.join(OUT, "wix-dialog.bmp"))

    # WiX banner 493x58 (top strip; text on the left is drawn by WiX)
    ban = Image.new("RGBA", (986, 116), (255, 255, 255, 255))
    ic = icon(88)
    ban.alpha_composite(ic, (986 - 88 - 18, 14))
    bar = gradient((986, 6), [(0, CYAN), (0.5, VIOLET), (1, MAGENTA)], 0).convert("RGBA")
    ban.alpha_composite(bar, (0, 110))
    bmp(ban.resize((493, 58), Image.LANCZOS), os.path.join(OUT, "wix-banner.bmp"))

def social():
    W, H = 1280, 640
    img = gradient((W, H), [(0, NIGHT_2), (1, NIGHT)], 90).convert("RGBA")
    glow = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    ImageDraw.Draw(glow).ellipse([W * 0.15, H * 0.1, W * 0.85, H * 1.2], fill=VIOLET + (90,))
    img = Image.alpha_composite(img, glow.filter(ImageFilter.GaussianBlur(120)))
    lk = lockup(1000, 260, dark=False)
    img.alpha_composite(lk, ((W - lk.width) // 2, 170))
    t = text_gradient("The open-source live wallpaper engine", font(40, 400), [(0, (190, 195, 230)), (1, (190, 195, 230))])
    img.alpha_composite(t, ((W - t.width) // 2, 470))
    img.convert("RGB").save(os.path.join(OUT, "social-preview.png"))

if __name__ == "__main__":
    icon(1024).save(os.path.join(OUT, "icon-1024.png"))
    icon(1024, with_bg=False).save(os.path.join(OUT, "mark-1024.png"))
    lockup().save(os.path.join(OUT, "logo-lockup.png"))
    lockup(dark=False).save(os.path.join(OUT, "logo-lockup-transparent.png"))
    installer_art()
    social()
    print("ok")
