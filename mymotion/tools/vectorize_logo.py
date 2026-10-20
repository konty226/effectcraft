"""Vectorise un PNG à alpha (logo monochrome) en SVG pour l'extruder en 3D (Blender) : marching squares
sur-échantillonné, chaînage des segments en boucles, lissage de Chaikin, simplification.
Usage : python -I vectorize.py entree.png sortie.svg [facteur]"""
import struct, sys, zlib, math

def load_png_alpha(path):
    d = open(path, 'rb').read()
    w, h, bd, ct = struct.unpack('>IIBB', d[16:26])
    assert bd == 8 and ct == 6, "PNG RGBA 8 bits attendu"
    i, idat = 8, b''
    while i < len(d):
        L = struct.unpack('>I', d[i:i + 4])[0]; t = d[i + 4:i + 8]
        if t == b'IDAT': idat += d[i + 8:i + 8 + L]
        i += 12 + L
    raw = zlib.decompress(idat); stride = w * 4; prev = bytearray(stride); p = 0; alpha = []
    for y in range(h):
        f = raw[p]; line = bytearray(raw[p + 1:p + 1 + stride]); p += 1 + stride
        for x in range(stride):
            a = line[x - 4] if x >= 4 else 0; b = prev[x]; c = prev[x - 4] if x >= 4 else 0
            if f == 1: line[x] = (line[x] + a) & 255
            elif f == 2: line[x] = (line[x] + b) & 255
            elif f == 3: line[x] = (line[x] + (a + b) // 2) & 255
            elif f == 4:
                pa, pb, pc = abs(b - c), abs(a - c), abs(a + b - 2 * c)
                line[x] = (line[x] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 255
        prev = line
        alpha.append([line[x * 4 + 3] / 255 for x in range(w)])
    return w, h, alpha

def upsample(w, h, a, k):
    """Bilinéaire ×k, avec une marge transparente d'un pixel."""
    W, H = w * k + 2, h * k + 2
    out = [[0.0] * W for _ in range(H)]
    for Y in range(1, H - 1):
        fy = (Y - 1 + 0.5) / k - 0.5; y0 = max(0, min(h - 1, int(math.floor(fy)))); y1 = min(h - 1, y0 + 1); ty = min(1, max(0, fy - y0))
        r0, r1 = a[y0], a[y1]
        row = out[Y]
        for X in range(1, W - 1):
            fx = (X - 1 + 0.5) / k - 0.5; x0 = max(0, min(w - 1, int(math.floor(fx)))); x1 = min(w - 1, x0 + 1); tx = min(1, max(0, fx - x0))
            top = r0[x0] * (1 - tx) + r0[x1] * tx; bot = r1[x0] * (1 - tx) + r1[x1] * tx
            row[X] = top * (1 - ty) + bot * ty
    return W, H, out

def marching(W, H, g, iso=0.5):
    """Segments orientés (intérieur à gauche), extrémités sur les arêtes de la grille."""
    def lerp(p, q, vp, vq):
        # Même point quel que soit le sens de l'arête (cellules voisines) : sinon les boucles ne se ferment pas.
        if (q[1], q[0]) < (p[1], p[0]):
            p, q, vp, vq = q, p, vq, vp
        t = (iso - vp) / (vq - vp) if vq != vp else 0.5
        return (p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t)
    segs = []
    for y in range(H - 1):
        for x in range(W - 1):
            v = [g[y][x], g[y][x + 1], g[y + 1][x + 1], g[y + 1][x]]
            c = [(x, y), (x + 1, y), (x + 1, y + 1), (x, y + 1)]
            idx = sum(1 << i for i in range(4) if v[i] >= iso)
            if idx in (0, 15): continue
            e = {}
            for k, (i, j) in enumerate([(0, 1), (1, 2), (2, 3), (3, 0)]):
                if (v[i] >= iso) != (v[j] >= iso):
                    e[k] = lerp(c[i], c[j], v[i], v[j])
            # table : pour chaque arête d'entrée, l'arête de sortie (intérieur à gauche)
            table = {1: [(3, 0)], 2: [(0, 1)], 3: [(3, 1)], 4: [(1, 2)], 6: [(0, 2)], 7: [(3, 2)], 8: [(2, 3)],
                     9: [(2, 0)], 11: [(2, 1)], 12: [(1, 3)], 13: [(1, 0)], 14: [(0, 3)],
                     5: [(3, 2), (1, 0)], 10: [(0, 3), (2, 1)]}
            for a, b in table[idx]:
                segs.append((e[a], e[b]))
    return segs

def chain(segs):
    """Boucles par adjacence non orientée (remplissage pair-impair : le sens ne compte pas)."""
    key = lambda p: (round(p[0], 6), round(p[1], 6))
    adj, pos = {}, {}
    for a, b in segs:
        ka, kb = key(a), key(b)
        if ka == kb: continue
        pos[ka], pos[kb] = a, b
        adj.setdefault(ka, []).append(kb); adj.setdefault(kb, []).append(ka)
    loops, seen = [], set()
    for k0 in list(adj):
        if k0 in seen: continue
        loop, prev, k = [], None, k0
        while k not in seen:
            seen.add(k); loop.append(pos[k])
            nxt = [n for n in adj[k] if n != prev and n not in seen]
            if not nxt: break
            prev, k = k, nxt[0]
        if len(loop) > 8: loops.append(loop)
    return loops

def area(l):
    return 0.5 * sum(l[i][0] * l[(i + 1) % len(l)][1] - l[(i + 1) % len(l)][0] * l[i][1] for i in range(len(l)))

def chaikin(l, n=2):
    for _ in range(n):
        out = []
        for i in range(len(l)):
            p, q = l[i], l[(i + 1) % len(l)]
            out += [(0.75 * p[0] + 0.25 * q[0], 0.75 * p[1] + 0.25 * q[1]), (0.25 * p[0] + 0.75 * q[0], 0.25 * p[1] + 0.75 * q[1])]
        l = out
    return l

def simplify(l, eps):
    """Douglas-Peucker sur une boucle fermée."""
    def dp(pts):
        if len(pts) < 3: return pts
        a, b = pts[0], pts[-1]; dx, dy = b[0] - a[0], b[1] - a[1]; n = math.hypot(dx, dy) or 1e-9
        i, dmax = 0, -1
        for j in range(1, len(pts) - 1):
            d = abs(dy * pts[j][0] - dx * pts[j][1] + b[0] * a[1] - b[1] * a[0]) / n
            if d > dmax: i, dmax = j, d
        if dmax > eps: return dp(pts[:i + 1])[:-1] + dp(pts[i:])
        return [a, b]
    half = len(l) // 2
    return dp(l[:half + 1])[:-1] + dp(l[half:] + [l[0]])[:-1]

def main():
    src, dst = sys.argv[1], sys.argv[2]; k = int(sys.argv[3]) if len(sys.argv) > 3 else 4
    w, h, a = load_png_alpha(src)
    W, H, g = upsample(w, h, a, k)
    loops = [simplify(chaikin(l, 2), 0.04 * k) for l in chain(marching(W, H, g))]
    loops = [l for l in loops if abs(area(l)) > (1.5 * k) ** 2]
    s = 1 / k
    parts = []
    for l in loops:
        pts = " ".join(f"{(x - 1) * s:.3f},{(y - 1) * s:.3f}" for x, y in l)
        parts.append(f"M{pts}Z")
    svg = f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}"><path fill="#0957a2" fill-rule="evenodd" d="{" ".join(parts)}"/></svg>\n'
    open(dst, 'w', encoding='utf-8').write(svg)
    print(f"{len(loops)} contours, {sum(len(l) for l in loops)} points -> {dst}")

main()
