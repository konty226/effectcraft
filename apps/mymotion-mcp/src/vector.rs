//! Raster logo → SVG, to extrude it in 3D (Blender) or use it as a sharp shape layer: marching
//! squares at sub-pixel precision on the upsampled coverage, undirected loop chaining (even-odd
//! fill, so loop direction does not matter), Chaikin smoothing, Douglas–Peucker simplification.

use std::collections::HashMap;

type P = (f64, f64);

/// Coverage 0..1 per pixel: alpha when the image has transparency, else darkness (dark logo on
/// light background).
pub fn coverage(img: &image::RgbaImage) -> Vec<Vec<f64>> {
    let has_alpha = img.pixels().any(|p| p[3] < 250);
    (0..img.height())
        .map(|y| {
            (0..img.width())
                .map(|x| {
                    let p = img.get_pixel(x, y);
                    if has_alpha {
                        f64::from(p[3]) / 255.0
                    } else {
                        1.0 - (0.2126 * f64::from(p[0]) + 0.7152 * f64::from(p[1]) + 0.0722 * f64::from(p[2])) / 255.0
                    }
                })
                .collect()
        })
        .collect()
}

/// Bilinear ×k with a one-cell transparent border.
fn upsample(a: &[Vec<f64>], k: usize) -> Vec<Vec<f64>> {
    let h = a.len();
    let w = a.first().map_or(0, Vec::len);
    let (ww, hh) = (w * k + 2, h * k + 2);
    let mut out = vec![vec![0.0; ww]; hh];
    if w == 0 || h == 0 {
        return out;
    }
    let get = |y: usize, x: usize| a.get(y).and_then(|r| r.get(x)).copied().unwrap_or(0.0);
    for (yy, row) in out.iter_mut().enumerate().take(hh - 1).skip(1) {
        let fy = (yy as f64 - 1.0 + 0.5) / k as f64 - 0.5;
        let y0 = (fy.floor().max(0.0) as usize).min(h - 1);
        let y1 = (y0 + 1).min(h - 1);
        let ty = (fy - y0 as f64).clamp(0.0, 1.0);
        for (xx, cell) in row.iter_mut().enumerate().take(ww - 1).skip(1) {
            let fx = (xx as f64 - 1.0 + 0.5) / k as f64 - 0.5;
            let x0 = (fx.floor().max(0.0) as usize).min(w - 1);
            let x1 = (x0 + 1).min(w - 1);
            let tx = (fx - x0 as f64).clamp(0.0, 1.0);
            let top = get(y0, x0) * (1.0 - tx) + get(y0, x1) * tx;
            let bot = get(y1, x0) * (1.0 - tx) + get(y1, x1) * tx;
            *cell = top * (1.0 - ty) + bot * ty;
        }
    }
    out
}

fn marching(g: &[Vec<f64>], iso: f64) -> Vec<(P, P)> {
    let lerp = |mut p: P, mut q: P, mut vp: f64, mut vq: f64| -> P {
        // Same point whichever cell computes the shared edge.
        if (q.1, q.0) < (p.1, p.0) {
            std::mem::swap(&mut p, &mut q);
            std::mem::swap(&mut vp, &mut vq);
        }
        let t = if (vq - vp).abs() > 1e-12 { (iso - vp) / (vq - vp) } else { 0.5 };
        (p.0 + (q.0 - p.0) * t, p.1 + (q.1 - p.1) * t)
    };
    let mut segs = vec![];
    for y in 0..g.len().saturating_sub(1) {
        let (r0, r1) = (&g[y], &g[y + 1]);
        for x in 0..r0.len().saturating_sub(1).min(r1.len().saturating_sub(1)) {
            let v = [r0[x], r0[x + 1], r1[x + 1], r1[x]];
            let (fx, fy) = (x as f64, y as f64);
            let c = [(fx, fy), (fx + 1.0, fy), (fx + 1.0, fy + 1.0), (fx, fy + 1.0)];
            let inside: Vec<bool> = v.iter().map(|&a| a >= iso).collect();
            let mut e: Vec<P> = vec![];
            for (i, j) in [(0, 1), (1, 2), (2, 3), (3, 0)] {
                if inside[i] != inside[j] {
                    e.push(lerp(c[i], c[j], v[i], v[j]));
                }
            }
            match e.len() {
                2 => segs.push((e[0], e[1])),
                4 => {
                    // Saddle: pair the crossings by the cell's mean.
                    if (v.iter().sum::<f64>() / 4.0 >= iso) == inside[0] {
                        segs.push((e[0], e[3]));
                        segs.push((e[1], e[2]));
                    } else {
                        segs.push((e[0], e[1]));
                        segs.push((e[2], e[3]));
                    }
                }
                _ => {}
            }
        }
    }
    segs
}

fn chain(segs: &[(P, P)]) -> Vec<Vec<P>> {
    let key = |p: P| ((p.0 * 1e6).round() as i64, (p.1 * 1e6).round() as i64);
    let mut adj: HashMap<(i64, i64), Vec<(i64, i64)>> = HashMap::new();
    let mut pos: HashMap<(i64, i64), P> = HashMap::new();
    for &(a, b) in segs {
        let (ka, kb) = (key(a), key(b));
        if ka == kb {
            continue;
        }
        pos.insert(ka, a);
        pos.insert(kb, b);
        adj.entry(ka).or_default().push(kb);
        adj.entry(kb).or_default().push(ka);
    }
    let mut keys: Vec<(i64, i64)> = adj.keys().copied().collect();
    keys.sort_unstable();
    let mut seen = std::collections::HashSet::new();
    let mut loops = vec![];
    for k0 in keys {
        if seen.contains(&k0) {
            continue;
        }
        let (mut lp, mut prev, mut k) = (vec![], None, k0);
        while seen.insert(k) {
            if let Some(&p) = pos.get(&k) {
                lp.push(p);
            }
            let next = adj.get(&k).and_then(|n| n.iter().find(|n| Some(**n) != prev && !seen.contains(*n)).copied());
            let Some(n) = next else { break };
            prev = Some(k);
            k = n;
        }
        if lp.len() > 8 {
            loops.push(lp);
        }
    }
    loops
}

fn area(l: &[P]) -> f64 {
    let n = l.len();
    0.5 * (0..n).map(|i| l[i].0 * l[(i + 1) % n].1 - l[(i + 1) % n].0 * l[i].1).sum::<f64>()
}

fn chaikin(mut l: Vec<P>, n: usize) -> Vec<P> {
    for _ in 0..n {
        let m = l.len();
        let mut out = Vec::with_capacity(m * 2);
        for i in 0..m {
            let (p, q) = (l[i], l[(i + 1) % m]);
            out.push((0.75 * p.0 + 0.25 * q.0, 0.75 * p.1 + 0.25 * q.1));
            out.push((0.25 * p.0 + 0.75 * q.0, 0.25 * p.1 + 0.75 * q.1));
        }
        l = out;
    }
    l
}

fn dp(pts: &[P], eps: f64, out: &mut Vec<P>) {
    let (Some(&a), Some(&b)) = (pts.first(), pts.last()) else { return };
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let n = dx.hypot(dy).max(1e-9);
    let (mut idx, mut dmax) = (0, -1.0);
    for (j, p) in pts.iter().enumerate().take(pts.len().saturating_sub(1)).skip(1) {
        let d = (dy * p.0 - dx * p.1 + b.0 * a.1 - b.1 * a.0).abs() / n;
        if d > dmax {
            idx = j;
            dmax = d;
        }
    }
    if dmax > eps && idx > 0 {
        dp(&pts[..=idx], eps, out);
        out.pop();
        dp(&pts[idx..], eps, out);
    } else {
        out.push(a);
        out.push(b);
    }
}

fn simplify(l: &[P], eps: f64) -> Vec<P> {
    if l.len() < 8 {
        return l.to_vec();
    }
    let half = l.len() / 2;
    let mut a = vec![];
    dp(&l[..=half], eps, &mut a);
    a.pop();
    let mut tail: Vec<P> = l[half..].to_vec();
    tail.push(l[0]);
    let mut b = vec![];
    dp(&tail, eps, &mut b);
    b.pop();
    a.extend(b);
    a
}

/// SVG of the shape covered in `cov` (pixels of the source image), and the number of contours.
pub fn to_svg(cov: &[Vec<f64>], k: usize, color: &str) -> (String, usize, usize) {
    let h = cov.len();
    let w = cov.first().map_or(0, Vec::len);
    let k = k.clamp(1, 12);
    let g = upsample(cov, k);
    let kf = k as f64;
    let loops: Vec<Vec<P>> =
        chain(&marching(&g, 0.5)).into_iter().map(|l| simplify(&chaikin(l, 2), 0.04 * kf)).filter(|l| area(l).abs() > (1.5 * kf).powi(2)).collect();
    let points = loops.iter().map(Vec::len).sum();
    let d: Vec<String> = loops
        .iter()
        .map(|l| {
            let pts: Vec<String> = l.iter().map(|(x, y)| format!("{:.3},{:.3}", (x - 1.0) / kf, (y - 1.0) / kf)).collect();
            format!("M{}Z", pts.join(" "))
        })
        .collect();
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\"><path fill=\"{color}\" fill-rule=\"evenodd\" d=\"{}\"/></svg>\n",
        d.join(" ")
    );
    (svg, loops.len(), points)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_gives_two_contours() {
        // A ring: outer disc minus inner disc → one outer contour and one hole.
        let n = 60;
        let cov: Vec<Vec<f64>> = (0..n)
            .map(|y| {
                (0..n)
                    .map(|x| {
                        let r = ((x as f64 - 30.0).powi(2) + (y as f64 - 30.0).powi(2)).sqrt();
                        if (12.0..=24.0).contains(&r) { 1.0 } else { 0.0 }
                    })
                    .collect()
            })
            .collect();
        let (svg, contours, points) = to_svg(&cov, 4, "#000");
        assert_eq!(contours, 2, "{svg}");
        assert!(points > 16 && svg.contains("evenodd"));
        let (_, empty, _) = to_svg(&[], 4, "#000");
        assert_eq!(empty, 0);
    }
}
