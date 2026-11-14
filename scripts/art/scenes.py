#!/usr/bin/env python3
"""Original race-track illustrations for the launcher's hub.

Dev-time asset authoring only (the outputs, assets/art/*.jpg, are committed;
nothing in the build or the package runs this). Every scene is drawn here
from shapes and a fixed seed: no photos, no game files, no official art, no
logos, no characters and no item designs from any game. Three tracks seen
from the driver's seat:

- fzax:  a floating neon track at night between towers, guide lights along
         both edges, a loop in the sky and the glow of racers ahead;
- mkgp2: a sunny kart circuit with red and white kerbs, bunting, a
         grandstand and round hills;
- mkdx:  a seaside circuit at sunset with palm trees and kerbs.

usage: python3 -I scripts/art/scenes.py assets/art   (needs rsvg-convert, Pillow)
"""
import math
import os
import random
import subprocess
import sys

W, H = 2560, 1440
HORIZON = 690
VX = W // 2 - 80


def hexrgb(h):
    h = h.lstrip('#')
    return tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))


def mix(a, b, t):
    a, b = hexrgb(a), hexrgb(b)
    return '#%02x%02x%02x' % tuple(round(x + (y - x) * t) for x, y in zip(a, b))


class Scene:
    def __init__(self, slug, seed, sky, ground, road, light, accent, bend, wiggle, kind):
        self.slug, self.seed = slug, seed
        self.sky, self.ground, self.road, self.light, self.accent = sky, ground, road, light, accent
        self.bend, self.wiggle, self.kind = bend, wiggle, kind


SCENES = [
    # F-Zero AX: night, magenta and cyan, the track floats over the dark
    Scene('fzax', 7, ('#03020e', '#1a0a3a', '#5a1a78'), '#020208', ('#0a0c1c', '#04040a'),
          '#4df2ff', '#ff4fd8', -0.75, 0.35, 'neon'),
    # Mario Kart Arcade GP 2: noon, blue sky, green hills
    Scene('mkgp2', 11, ('#2f7fe0', '#6fb4f4', '#c8e6ff'), '#3fae4a', ('#5a5f6a', '#3a3e46'),
          '#ffffff', '#ffd23f', 0.6, 0.25, 'circuit'),
    # Mario Kart Arcade GP DX: sunset by the sea
    Scene('mkdx', 17, ('#2a1a5a', '#c4527a', '#ffb36a'), '#d9b36a', ('#4a4250', '#2a2430'),
          '#ffe2b0', '#ff7a3d', -0.45, 0.3, 'seaside'),
]


# ----------------------------------------------------------------- the road

def centre(s, u):
    """The road's centre line at depth u (0 at the horizon, 1 at the bottom edge)."""
    far = (1 - u) ** 2
    return VX + s.bend * 760 * far + s.wiggle * 220 * math.sin(u * 8.5 + s.seed) * (1 - u) ** 1.2


def half_width(u):
    return 40 + 1080 * u


def ypos(u):
    return HORIZON + (H - HORIZON) * u


def pt(s, u, lateral, height=0.0):
    """Screen point at depth u, `lateral` road half-widths from the centre, `height` above the road."""
    return centre(s, u) + lateral * half_width(u), ypos(u) - height * 520 * u


def depths(n, near=1.0, far=0.045, power=1.0):
    """Equal steps in distance (u is 1/z), near to far."""
    z0, z1 = 1 / near, 1 / far
    return [1 / (z0 + (z1 - z0) * (i / (n - 1)) ** power) for i in range(n)]


def poly(points, fill, extra=''):
    return f'<polygon points="{" ".join(f"{x:.0f},{y:.0f}" for x, y in points)}" fill="{fill}" {extra}/>'


def defs(s):
    top, mid, low = s.sky
    return f'''<defs>
  <linearGradient id="sky" x1="0" y1="0" x2="0" y2="1">
    <stop offset="0" stop-color="{top}"/>
    <stop offset="0.65" stop-color="{mid}"/>
    <stop offset="1" stop-color="{low}"/>
  </linearGradient>
  <linearGradient id="road" x1="0" y1="0" x2="0" y2="1">
    <stop offset="0" stop-color="{s.road[0]}"/>
    <stop offset="1" stop-color="{s.road[1]}"/>
  </linearGradient>
  <radialGradient id="vig" cx="0.5" cy="0.5" r="0.75">
    <stop offset="0.6" stop-color="#000" stop-opacity="0"/>
    <stop offset="1" stop-color="#000" stop-opacity="0.6"/>
  </radialGradient>
  <filter id="blur6" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="6"/></filter>
  <filter id="blur18" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="18"/></filter>
  <filter id="blur50" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="50"/></filter>
</defs>'''


def sky(r, s):
    out = [f'<rect width="{W}" height="{H}" fill="url(#sky)"/>']
    if s.kind == 'neon':
        for _ in range(260):
            out.append(f'<circle cx="{r.randint(0, W)}" cy="{r.randint(10, HORIZON - 200)}" r="{r.uniform(0.7, 2.2):.1f}" '
                       f'fill="#fff" opacity="{r.uniform(0.2, 0.9):.2f}"/>')
        # a ringed planet low in the sky
        px, py = 1950, 250
        out.append(f'<circle cx="{px}" cy="{py}" r="220" fill="{s.accent}" opacity="0.12" filter="url(#blur50)"/>')
        out.append(f'<circle cx="{px}" cy="{py}" r="92" fill="#3a1f6e"/>')
        out.append(f'<circle cx="{px - 26}" cy="{py - 22}" r="80" fill="#5a3a9a" opacity="0.7"/>')
        out.append(f'<ellipse cx="{px}" cy="{py}" rx="190" ry="34" fill="none" stroke="{s.light}" stroke-width="5" '
                   f'opacity="0.7" transform="rotate(-14 {px} {py})"/>')
    else:
        sun_x, sun_y = (2020, 160) if s.kind == 'circuit' else (VX - 480, HORIZON - 70)
        out.append(f'<circle cx="{sun_x}" cy="{sun_y}" r="300" fill="{s.light}" opacity="0.35" filter="url(#blur50)"/>')
        out.append(f'<circle cx="{sun_x}" cy="{sun_y}" r="{78 if s.kind == "circuit" else 120}" fill="#fff6dc" opacity="0.95"/>')
        for _ in range(9 if s.kind == 'circuit' else 6):
            cx, cy = r.randint(-100, W + 100), r.randint(70, HORIZON - 240)
            col = '#ffffff' if s.kind == 'circuit' else mix('#ffd0b0', s.sky[1], 0.4)
            for k in range(5):
                out.append(f'<ellipse cx="{cx + k * 70 - 140 + r.randint(-20, 20)}" cy="{cy + r.randint(-18, 18)}" '
                           f'rx="{r.randint(80, 150)}" ry="{r.randint(36, 60)}" fill="{col}" opacity="0.85"/>')
    return '\n'.join(out)


def towers(r, s):
    """Lit towers on both sides of the floating track, far below its edge."""
    out = []
    for _ in range(70):
        x = r.choice([r.randint(-60, VX - 380), r.randint(VX + 380, W + 60)])
        w = r.randint(40, 150)
        top = HORIZON - r.randint(60, 520)
        col = mix('#05040f', s.sky[1], r.uniform(0.15, 0.4))
        out.append(f'<rect x="{x}" y="{top}" width="{w}" height="{H - top}" fill="{col}"/>')
        for _ in range(r.randint(6, 22)):
            wx, wy = x + r.randint(4, max(5, w - 10)), top + r.randint(6, 560)
            c = r.choice([s.light, s.accent, '#ffffff', '#ffd27a'])
            out.append(f'<rect x="{wx}" y="{wy}" width="6" height="3" fill="{c}" opacity="{r.uniform(0.4, 0.95):.2f}"/>')
        if r.random() < 0.3:
            out.append(f'<circle cx="{x + w // 2}" cy="{top - 6}" r="4" fill="#ff4040"/>')
    # haze under the track
    out.append(f'<rect x="0" y="{HORIZON - 40}" width="{W}" height="{H - HORIZON + 40}" fill="{s.sky[2]}" opacity="0.25" filter="url(#blur50)"/>')
    return '\n'.join(out)


def loop_in_the_sky(r, s):
    """A loop of track high above, outlined by guide lights."""
    cx, cy, rx, ry = VX + 520, HORIZON - 360, 210, 250
    out = [f'<ellipse cx="{cx}" cy="{cy}" rx="{rx}" ry="{ry}" fill="none" stroke="{mix("#101030", s.light, 0.2)}" stroke-width="34"/>']
    for k in range(48):
        a = 2 * math.pi * k / 48
        for d, col in ((-17, s.light), (17, s.accent)):
            x = cx + (rx + d) * math.cos(a)
            y = cy + (ry + d) * math.sin(a)
            out.append(f'<circle cx="{x:.0f}" cy="{y:.0f}" r="4" fill="{col}"/>')
    out.append(f'<ellipse cx="{cx}" cy="{cy}" rx="{rx}" ry="{ry}" fill="none" stroke="{s.light}" stroke-width="40" opacity="0.15" filter="url(#blur18)"/>')
    # its pylons
    for dx in (-140, 140):
        out.append(f'<rect x="{cx + dx - 10}" y="{cy + ry - 20}" width="20" height="{HORIZON - cy - ry + 120}" fill="#0a0818"/>')
    return '\n'.join(out)


def hills(r, s):
    out = []
    for k, (base, amp) in enumerate([(HORIZON - 60, 160), (HORIZON - 10, 110)]):
        col = mix(s.ground, s.sky[2], 0.45 - 0.2 * k)
        x = -200
        while x < W + 200:
            rad = r.randint(260, 520)
            out.append(f'<ellipse cx="{x}" cy="{base + rad * 0.55}" rx="{rad}" ry="{amp + rad * 0.4}" fill="{col}"/>')
            x += r.randint(260, 480)
    return '\n'.join(out)


def grandstand(r, s):
    """A covered stand beside the circuit, its crowd as dots of colour."""
    x0, y0, w, h = VX + 420, HORIZON - 170, 760, 150
    out = [poly([(x0, y0 + h), (x0 + w, y0 + h), (x0 + w - 40, y0), (x0 + 40, y0)], '#d8dde6'),
           f'<rect x="{x0 + 10}" y="{y0 - 34}" width="{w - 20}" height="22" fill="#e04848"/>']
    for row in range(6):
        for k in range(48):
            cx = x0 + 60 + k * (w - 120) / 47
            cy = y0 + 16 + row * 22
            out.append(f'<circle cx="{cx:.0f}" cy="{cy}" r="5" fill="{r.choice(["#ff5a5a", "#3a7bff", "#ffd23f", "#4cc36a", "#ffffff", "#ff9a3d"])}"/>')
    for k in range(6):
        px = x0 + 20 + k * (w - 40) / 5
        out.append(f'<rect x="{px - 4:.0f}" y="{y0 - 14}" width="8" height="{h + 14}" fill="#9aa3b2"/>')
    return '\n'.join(out)


def sea(r, s):
    out = [f'<rect x="0" y="{HORIZON - 40}" width="{W}" height="140" fill="{mix("#2a4a8a", s.sky[2], 0.35)}"/>']
    for _ in range(140):
        x, y = r.randint(0, W), r.randint(HORIZON - 36, HORIZON + 90)
        out.append(f'<line x1="{x}" y1="{y}" x2="{x + r.randint(20, 70)}" y2="{y}" stroke="#ffd9a8" stroke-width="2" opacity="{r.uniform(0.2, 0.6):.2f}"/>')
    # the sun's path on the water
    sx = VX - 480
    out.append(f'<ellipse cx="{sx}" cy="{HORIZON + 30}" rx="90" ry="120" fill="#fff0c8" opacity="0.35" filter="url(#blur18)"/>')
    return '\n'.join(out)


def palms(r, s):
    out = []
    items = []
    for _ in range(18):
        u = r.uniform(0.08, 0.95) ** 1.2
        side = r.choice((-1, 1))
        items.append((u, side * r.uniform(1.6, 2.6 + 2 * u)))
    for u, lat in sorted(items):
        x, y = pt(s, u, lat)
        h = 80 + 700 * u
        lean = (1 if lat > 0 else -1) * h * 0.12
        tx, ty = x + lean, y - h
        col = mix('#1a1020', s.sky[1], 0.15)
        out.append(f'<path d="M{x - 6 - 10 * u:.0f},{y:.0f} Q{x + lean * 0.3:.0f},{y - h * 0.5:.0f} {tx:.0f},{ty:.0f} '
                   f'L{tx + 6 + 8 * u:.0f},{ty:.0f} Q{x + lean * 0.3 + 12:.0f},{y - h * 0.5:.0f} {x + 6 + 10 * u:.0f},{y:.0f} Z" fill="{col}"/>')
        for k in range(7):
            a = -math.pi / 2 + (k - 3) * 0.48
            fl = h * 0.42
            ex, ey = tx + math.cos(a) * fl * 1.3, ty + math.sin(a) * fl * 0.5 + fl * 0.35
            mx_, my_ = tx + math.cos(a) * fl * 0.7, ty - fl * 0.25
            out.append(f'<path d="M{tx:.0f},{ty:.0f} Q{mx_:.0f},{my_:.0f} {ex:.0f},{ey:.0f}" stroke="{col}" '
                       f'stroke-width="{4 + 20 * u:.0f}" fill="none" stroke-linecap="round"/>')
    return '\n'.join(out)


def track(r, s):
    out = []
    if s.kind != 'neon':
        out.append(f'<rect x="0" y="{HORIZON}" width="{W}" height="{H - HORIZON}" fill="{s.ground}"/>')
        # mown stripes on the verge
        for i, u in enumerate(depths(30, 1.0, 0.05)):
            if i % 2:
                continue
            (xa, ya), (xb, yb) = pt(s, u, -8), pt(s, u, 8)
            out.append(f'<line x1="{xa:.0f}" y1="{ya:.0f}" x2="{xb:.0f}" y2="{yb:.0f}" stroke="#ffffff" '
                       f'stroke-width="{2 + 30 * u:.0f}" opacity="0.06"/>')
    us = depths(70, 1.0, 0.045, 1.0)
    for u0, u1 in zip(us, us[1:]):
        shade = mix(s.road[1], s.road[0], 1 - u0)
        out.append(poly([pt(s, u0, -1), pt(s, u0, 1), pt(s, u1, 1), pt(s, u1, -1)], shade))
    if s.kind == 'neon':
        # the deck's underside and its glow into the dark below
        for side in (-1, 1):
            pts = [pt(s, u, side * 1.0) for u in us] + [pt(s, u, side * 1.0, -0.12) for u in reversed(us)]
            out.append(poly(pts, '#0a0820'))
        for lat, col in ((-1.0, s.light), (1.0, s.accent)):
            line = ' '.join(f'{x:.0f},{y:.0f}' for x, y in (pt(s, u, lat) for u in us))
            out.append(f'<polyline points="{line}" stroke="{col}" stroke-width="40" fill="none" opacity="0.25" filter="url(#blur18)"/>')
            out.append(f'<polyline points="{line}" stroke="{col}" stroke-width="7" fill="none"/>')
            for u in depths(40, 1.0, 0.05):
                x, y = pt(s, u, lat * 0.97, 0.02)
                out.append(f'<circle cx="{x:.0f}" cy="{y:.0f}" r="{2 + 12 * u:.1f}" fill="#ffffff"/>')
        # boost strips: chevrons of light on the deck
        for u in (0.22, 0.36, 0.58):
            for k in range(3):
                uu = u + k * 0.025 * u
                (xl, yl), (xc, yc), (xr, yr) = pt(s, uu, -0.35), pt(s, uu - 0.03 * uu, 0), pt(s, uu, 0.35)
                out.append(f'<polyline points="{xl:.0f},{yl:.0f} {xc:.0f},{yc:.0f} {xr:.0f},{yr:.0f}" stroke="#ffe34d" '
                           f'stroke-width="{3 + 14 * uu:.0f}" fill="none" opacity="0.9"/>')
            (xa, ya) = pt(s, u, 0)
            out.append(f'<ellipse cx="{xa:.0f}" cy="{ya:.0f}" rx="{300 * u:.0f}" ry="{60 * u:.0f}" fill="#ffe34d" opacity="0.25" filter="url(#blur18)"/>')
        # grid lines on the deck
        for lat in (-0.5, 0.5):
            line = ' '.join(f'{x:.0f},{y:.0f}' for x, y in (pt(s, u, lat) for u in us))
            out.append(f'<polyline points="{line}" stroke="{s.light}" stroke-width="2" fill="none" opacity="0.25"/>')
    else:
        # red and white kerbs on both edges
        ks = depths(46, 1.0, 0.05)
        for side in (-1, 1):
            for i, (u0, u1) in enumerate(zip(ks, ks[1:])):
                col = '#e23a3a' if i % 2 == 0 else '#f6f6f6'
                out.append(poly([pt(s, u0, side * 1.0), pt(s, u0, side * 1.14), pt(s, u1, side * 1.14), pt(s, u1, side * 1.0)], col))
        # the centre line
        dash = depths(40, 1.0, 0.05)
        for i in range(0, len(dash) - 1, 2):
            u0, u1 = dash[i], dash[i + 1]
            (xa, ya), (xb, yb) = pt(s, u0, 0), pt(s, u1, 0)
            w0, w1 = 2 + 14 * u0, 2 + 14 * u1
            out.append(poly([(xa - w0, ya), (xa + w0, ya), (xb + w1, yb), (xb - w1, yb)], '#ffffff', f'opacity="{0.4 + 0.4 * u0:.2f}"'))
        # a chequered start line across the road
        u = 0.42
        n = 16
        for row in range(2):
            uu0, uu1 = u + row * 0.02, u + (row + 1) * 0.02
            for k in range(n):
                l0, l1 = -1 + 2 * k / n, -1 + 2 * (k + 1) / n
                col = '#111111' if (k + row) % 2 else '#f4f4f4'
                out.append(poly([pt(s, uu0, l0), pt(s, uu0, l1), pt(s, uu1, l1), pt(s, uu1, l0)], col))
    return '\n'.join(out)


def bunting(r, s):
    """A line of flags strung across the road, and a start gantry."""
    out = []
    u = 0.30
    (xl, yl), (xr, yr) = pt(s, u, -1.5, 1.05), pt(s, u, 1.5, 1.05)
    for x in (xl, xr):
        _, yb = pt(s, u, -1.5)
        out.append(f'<rect x="{x - 9:.0f}" y="{yl:.0f}" width="18" height="{yb - yl:.0f}" fill="#eef0f4"/>')
    out.append(f'<rect x="{xl:.0f}" y="{yl - 20:.0f}" width="{xr - xl:.0f}" height="46" rx="8" fill="#2a63d8"/>')
    for k in range(5):
        cx = xl + (k + 0.5) * (xr - xl) / 5
        out.append(f'<circle cx="{cx:.0f}" cy="{yl + 3:.0f}" r="13" fill="{"#3ce06a" if k < 3 else "#2a3446"}"/>')
    # flags below the gantry
    n = 28
    for k in range(n):
        t0, t1 = k / n, (k + 1) / n
        x0, x1 = xl + (xr - xl) * t0, xl + (xr - xl) * t1
        sag = lambda t: 70 * math.sin(math.pi * t)
        y0, y1 = yl + 40 + sag(t0), yl + 40 + sag(t1)
        col = ['#ff4b4b', '#ffd23f', '#3a7bff', '#4cc36a'][k % 4]
        out.append(poly([(x0, y0), (x1, y1), ((x0 + x1) / 2, (y0 + y1) / 2 + 34)], col))
    return '\n'.join(out)


def racers_ahead(r, s):
    """Glows of the racers ahead: thruster pairs (neon) or kart rear lights."""
    out = []
    for u, lat, col in ((0.2, -0.25, s.accent), (0.12, 0.3, s.light), (0.075, -0.1, '#ffe34d')):
        x, y = pt(s, u, lat, 0.03)
        w = 140 * u + 6
        if s.kind == 'neon':
            out.append(f'<ellipse cx="{x:.0f}" cy="{y:.0f}" rx="{w * 1.6:.0f}" ry="{w * 0.5:.0f}" fill="{col}" opacity="0.35" filter="url(#blur18)"/>')
            out.append(f'<path d="M{x - w:.0f},{y:.0f} L{x:.0f},{y - w * 0.45:.0f} L{x + w:.0f},{y:.0f} Z" fill="#0c0c1a"/>')
            for dx in (-0.45, 0.45):
                out.append(f'<circle cx="{x + dx * w:.0f}" cy="{y - w * 0.08:.0f}" r="{w * 0.16:.0f}" fill="#ffffff"/>')
                out.append(f'<circle cx="{x + dx * w:.0f}" cy="{y - w * 0.08:.0f}" r="{w * 0.4:.0f}" fill="{col}" opacity="0.6" filter="url(#blur6)"/>')
        else:
            out.append(f'<ellipse cx="{x:.0f}" cy="{y + w * 0.08:.0f}" rx="{w * 1.1:.0f}" ry="{w * 0.18:.0f}" fill="#000" opacity="0.25"/>')
            out.append(f'<rect x="{x - w * 0.8:.0f}" y="{y - w * 0.5:.0f}" width="{w * 1.6:.0f}" height="{w * 0.42:.0f}" rx="{w * 0.12:.0f}" fill="{col}"/>')
            for dx in (-0.65, 0.65):
                out.append(f'<rect x="{x + dx * w - w * 0.18:.0f}" y="{y - w * 0.32:.0f}" width="{w * 0.36:.0f}" height="{w * 0.36:.0f}" rx="{w * 0.08:.0f}" fill="#1a1a1a"/>')
    return '\n'.join(out)


def speed_streaks(r, s):
    out = []
    for _ in range(90):
        y = r.randint(HORIZON + 40, H)
        side = r.choice((-1, 1))
        x = VX + side * r.randint(300, 1400)
        ln = r.randint(80, 360)
        out.append(f'<line x1="{x}" y1="{y}" x2="{x + side * ln}" y2="{y + ln * 0.35:.0f}" stroke="{r.choice([s.light, s.accent, "#ffffff"])}" '
                   f'stroke-width="{r.uniform(1.5, 4):.1f}" opacity="{r.uniform(0.1, 0.35):.2f}"/>')
    return '\n'.join(out)


def svg(s):
    r = random.Random(s.seed)
    parts = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" viewBox="0 0 {W} {H}">', defs(s), sky(r, s)]
    if s.kind == 'neon':
        parts += [towers(r, s), loop_in_the_sky(r, s), track(r, s), racers_ahead(r, s), speed_streaks(r, s)]
    elif s.kind == 'circuit':
        parts += [hills(r, s), grandstand(r, s), track(r, s), racers_ahead(r, s), bunting(r, s)]
    else:
        parts += [sea(r, s), track(r, s), palms(r, s), racers_ahead(r, s)]
    parts.append(f'<rect width="{W}" height="{H}" fill="url(#vig)"/>')
    parts.append('</svg>')
    return '\n'.join(parts)


def main():
    out = sys.argv[1]
    os.makedirs(out, exist_ok=True)
    from PIL import Image
    for s in SCENES:
        svg_path = os.path.join(out, s.slug + '.svg')
        png_path = os.path.join(out, s.slug + '.png')
        with open(svg_path, 'w') as f:
            f.write(svg(s))
        subprocess.run(['rsvg-convert', '-o', png_path, svg_path], check=True)
        im = Image.open(png_path).convert('RGB')
        # light film grain, from the fixed generator so the files repeat
        grain = Image.frombytes('L', im.size, random.Random(s.seed).randbytes(im.size[0] * im.size[1]))
        im = Image.blend(im, grain.convert('RGB'), 0.025)
        im.resize((1920, 1080), Image.LANCZOS).save(os.path.join(out, s.slug + '.jpg'), quality=86, optimize=True, progressive=True)
        im.resize((512, 288), Image.LANCZOS).save(os.path.join(out, s.slug + '-tile.jpg'), quality=86)
        os.remove(png_path)
        os.remove(svg_path)
        print(s.slug)


if __name__ == '__main__':
    main()
