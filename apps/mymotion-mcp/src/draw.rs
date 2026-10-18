//! Images for Claude to look at: a contact sheet of frames and a value/speed graph, labelled with
//! a built-in 5×7 bitmap font (no font files, so nothing to license or ship).

use image::{Rgba, RgbaImage};
use tiny_skia::{Color, Paint, PathBuilder, Pixmap, Stroke, StrokeDash, Transform};

const BG: Rgba<u8> = Rgba([27, 27, 31, 255]);
const LABEL: Rgba<u8> = Rgba([220, 220, 228, 255]);
const DIM: Rgba<u8> = Rgba([130, 130, 140, 255]);

/// 5×7 glyphs, one byte per row, bit 4 = leftmost pixel. Lowercase draws as uppercase; anything
/// else draws as a blank.
fn glyph(c: char) -> [u8; 7] {
    match c.to_ascii_uppercase() {
        '0' => [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E],
        '1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
        '2' => [0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F],
        '3' => [0x1E, 0x01, 0x01, 0x0E, 0x01, 0x01, 0x1E],
        '4' => [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02],
        '5' => [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E],
        '6' => [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E],
        '7' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
        '9' => [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C],
        '.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C],
        ',' => [0x00, 0x00, 0x00, 0x00, 0x0C, 0x04, 0x08],
        '-' => [0x00, 0x00, 0x00, 0x1F, 0x00, 0x00, 0x00],
        '+' => [0x00, 0x04, 0x04, 0x1F, 0x04, 0x04, 0x00],
        ':' => [0x00, 0x0C, 0x0C, 0x00, 0x0C, 0x0C, 0x00],
        '/' => [0x01, 0x02, 0x02, 0x04, 0x08, 0x08, 0x10],
        '%' => [0x19, 0x1A, 0x02, 0x04, 0x08, 0x0B, 0x13],
        '#' => [0x0A, 0x1F, 0x0A, 0x0A, 0x0A, 0x1F, 0x0A],
        '(' => [0x02, 0x04, 0x08, 0x08, 0x08, 0x04, 0x02],
        ')' => [0x08, 0x04, 0x02, 0x02, 0x02, 0x04, 0x08],
        '=' => [0x00, 0x00, 0x1F, 0x00, 0x1F, 0x00, 0x00],
        'A' => [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'B' => [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E],
        'C' => [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E],
        'D' => [0x1E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1E],
        'E' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F],
        'F' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10],
        'G' => [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F],
        'H' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'I' => [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
        'J' => [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0C],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F],
        'M' => [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x11, 0x19, 0x15, 0x13, 0x11, 0x11],
        'O' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'P' => [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10],
        'Q' => [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D],
        'R' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
        'S' => [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E],
        'T' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0A],
        'X' => [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11],
        'Y' => [0x11, 0x11, 0x0A, 0x04, 0x04, 0x04, 0x04],
        'Z' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F],
        _ => [0; 7],
    }
}

/// Width in pixels of `text` drawn at `scale`.
pub fn text_width(text: &str, scale: u32) -> u32 {
    (text.chars().count() as u32 * 6).saturating_sub(1) * scale
}

/// Draw `text` with its top-left corner at (x, y); pixels outside the image are skipped.
pub fn text(img: &mut RgbaImage, x: i64, y: i64, text: &str, scale: u32, color: Rgba<u8>) {
    let s = i64::from(scale.max(1));
    for (i, c) in text.chars().enumerate() {
        let gx = x + i as i64 * 6 * s;
        for (row, bits) in glyph(c).iter().enumerate() {
            for col in 0..5i64 {
                if bits & (0x10 >> col) == 0 {
                    continue;
                }
                for dy in 0..s {
                    for dx in 0..s {
                        put(img, gx + col * s + dx, y + row as i64 * s + dy, color);
                    }
                }
            }
        }
    }
}

fn put(img: &mut RgbaImage, x: i64, y: i64, c: Rgba<u8>) {
    if let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y))
        && let Some(p) = img.get_pixel_mut_checked(x, y)
    {
        *p = c;
    }
}

/// Copy `src` onto `dst` at (x, y), compositing over a checkerboard when `checker` (so transparent
/// renders stay readable).
fn blit(dst: &mut RgbaImage, src: &RgbaImage, x: u32, y: u32, checker: bool) {
    for (sx, sy, p) in src.enumerate_pixels() {
        let base = if checker && ((sx / 8 + sy / 8) % 2 == 0) { 200u16 } else { 150u16 };
        let a = u16::from(p[3]);
        let px = if checker {
            let mix = |c: u8| ((u16::from(c) * a + base * (255 - a)) / 255) as u8;
            Rgba([mix(p[0]), mix(p[1]), mix(p[2]), 255])
        } else {
            Rgba([p[0], p[1], p[2], 255])
        };
        if let Some(d) = dst.get_pixel_mut_checked(x + sx, y + sy) {
            *d = px;
        }
    }
}

/// One frame of a filmstrip and its caption.
pub struct Cell {
    pub image: RgbaImage,
    pub caption: String,
}

/// A contact sheet: frames in a grid, `columns` wide, each captioned underneath.
pub fn filmstrip(cells: &[Cell], columns: u32, checker: bool, title: &str) -> RgbaImage {
    const PAD: u32 = 8;
    const SCALE: u32 = 2;
    let caption_h = 7 * SCALE + 8;
    let title_h = 7 * SCALE + 12;
    let columns = columns.clamp(1, cells.len().max(1) as u32);
    let rows = (cells.len() as u32).div_ceil(columns).max(1);
    let cw = cells.iter().map(|c| c.image.width()).max().unwrap_or(1);
    let ch = cells.iter().map(|c| c.image.height()).max().unwrap_or(1);
    let w = PAD + columns * (cw + PAD);
    let h = title_h + PAD + rows * (ch + caption_h + PAD);
    let mut img = RgbaImage::from_pixel(w, h, BG);
    text(&mut img, i64::from(PAD), i64::from(PAD), title, SCALE, DIM);
    for (i, cell) in cells.iter().enumerate() {
        let (col, row) = (i as u32 % columns, i as u32 / columns);
        let x = PAD + col * (cw + PAD);
        let y = title_h + PAD + row * (ch + caption_h + PAD);
        blit(&mut img, &cell.image, x, y, checker);
        text(&mut img, i64::from(x), i64::from(y + ch + 5), &cell.caption, SCALE, LABEL);
    }
    img
}

/// A sampled animated property, ready to plot.
pub struct Series {
    pub times: Vec<f64>,
    /// One curve per kept dimension, each as long as `times`.
    pub dims: Vec<(String, Vec<f64>)>,
    /// Magnitude of the rate of change (units per second), as long as `times`.
    pub speed: Vec<f64>,
    /// Keyframe times (comp seconds) inside the plotted range.
    pub keys: Vec<f64>,
}

const DIM_COLORS: [[u8; 3]; 4] = [[255, 92, 122], [92, 214, 138], [92, 168, 255], [200, 140, 255]];
const SPEED_COLOR: [u8; 3] = [255, 204, 77];

fn range(values: impl Iterator<Item = f64>) -> (f64, f64) {
    let (lo, hi) = values.filter(|v| v.is_finite()).fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| (lo.min(v), hi.max(v)));
    if !lo.is_finite() {
        return (0.0, 1.0);
    }
    if (hi - lo).abs() < 1e-9 { (lo - 1.0, hi + 1.0) } else { (lo, hi) }
}

fn fmt(v: f64) -> String {
    if v.abs() >= 100.0 {
        format!("{v:.0}")
    } else if v.abs() >= 10.0 {
        format!("{v:.1}")
    } else {
        format!("{v:.2}")
    }
}

/// Value graph on top, speed graph underneath, keyframes as dashed lines, as in a graph editor.
pub fn curves(s: &Series, title: &str, width: u32, height: u32) -> Option<RgbaImage> {
    let (w, h) = (width.clamp(320, 2400), height.clamp(240, 1600));
    let mut pm = Pixmap::new(w, h)?;
    pm.fill(Color::from_rgba8(BG[0], BG[1], BG[2], 255));
    let (left, right, top, gap, bottom) = (70.0f32, 16.0f32, 34.0f32, 30.0f32, 28.0f32);
    let plot_h = h as f32 - top - gap - bottom;
    let value_box = (top, top + plot_h * 0.62);
    let speed_box = (value_box.1 + gap, h as f32 - bottom);
    let (t0, t1) = (s.times.first().copied().unwrap_or(0.0), s.times.last().copied().unwrap_or(1.0));
    let span = if t1 > t0 { t1 - t0 } else { 1.0 };
    let x_of = |t: f64| left + ((t - t0) / span) as f32 * (w as f32 - left - right);
    let (vlo, vhi) = range(s.dims.iter().flat_map(|(_, v)| v.iter().copied()));
    let (slo, shi) = (0.0, range(s.speed.iter().copied()).1.max(1e-6));
    let y_in = |v: f64, lo: f64, hi: f64, b: (f32, f32)| b.1 - ((v - lo) / (hi - lo)) as f32 * (b.1 - b.0);

    let mut grid = Paint::default();
    grid.set_color_rgba8(60, 60, 68, 255);
    let thin = Stroke { width: 1.0, ..Stroke::default() };
    for b in [value_box, speed_box] {
        let mut pb = PathBuilder::new();
        pb.move_to(left, b.0);
        pb.line_to(left, b.1);
        pb.line_to(w as f32 - right, b.1);
        if let Some(p) = pb.finish() {
            pm.stroke_path(&p, &grid, &thin, Transform::identity(), None);
        }
    }
    // Keyframes.
    let mut key_paint = Paint::default();
    key_paint.set_color_rgba8(150, 150, 160, 200);
    let dashed = Stroke { width: 1.0, dash: StrokeDash::new(vec![4.0, 4.0], 0.0), ..Stroke::default() };
    for &k in &s.keys {
        let x = x_of(k);
        let mut pb = PathBuilder::new();
        pb.move_to(x, value_box.0);
        pb.line_to(x, speed_box.1);
        if let Some(p) = pb.finish() {
            pm.stroke_path(&p, &key_paint, &dashed, Transform::identity(), None);
        }
        let mut d = PathBuilder::new();
        d.move_to(x, value_box.0 - 6.0);
        d.line_to(x + 5.0, value_box.0 - 1.0);
        d.line_to(x, value_box.0 + 4.0);
        d.line_to(x - 5.0, value_box.0 - 1.0);
        d.close();
        if let Some(p) = d.finish() {
            pm.fill_path(&p, &key_paint, tiny_skia::FillRule::Winding, Transform::identity(), None);
        }
    }
    let line = Stroke { width: 2.0, ..Stroke::default() };
    let mut draw = |values: &[f64], lo: f64, hi: f64, b: (f32, f32), rgb: [u8; 3]| {
        let mut pb = PathBuilder::new();
        for (i, (&t, &v)) in s.times.iter().zip(values).enumerate() {
            let (x, y) = (x_of(t), y_in(v, lo, hi, b));
            if i == 0 { pb.move_to(x, y) } else { pb.line_to(x, y) }
        }
        let mut paint = Paint::default();
        paint.set_color_rgba8(rgb[0], rgb[1], rgb[2], 255);
        paint.anti_alias = true;
        if let Some(p) = pb.finish() {
            pm.stroke_path(&p, &paint, &line, Transform::identity(), None);
        }
    };
    for (i, (_, v)) in s.dims.iter().enumerate() {
        draw(v, vlo, vhi, value_box, DIM_COLORS[i % DIM_COLORS.len()]);
    }
    draw(&s.speed, slo, shi, speed_box, SPEED_COLOR);

    // Pixmap data is premultiplied; the background is opaque, so it is plain RGBA here.
    let mut img = RgbaImage::from_raw(w, h, pm.data().to_vec())?;
    let sc = 2;
    text(&mut img, 8, 8, title, sc, LABEL);
    let mut lx = i64::from(w) - 8;
    for (i, (name, _)) in s.dims.iter().enumerate().rev() {
        let c = DIM_COLORS[i % DIM_COLORS.len()];
        lx -= i64::from(text_width(name, sc)) + 14;
        text(&mut img, lx, 8, name, sc, Rgba([c[0], c[1], c[2], 255]));
    }
    let lbl = |img: &mut RgbaImage, v: f64, y: f32| {
        let t = fmt(v);
        text(img, i64::from(left as u32) - 6 - i64::from(text_width(&t, 1)), y as i64 - 3, &t, 1, DIM);
    };
    lbl(&mut img, vhi, value_box.0);
    lbl(&mut img, vlo, value_box.1);
    text(&mut img, 6, value_box.0 as i64 + 14, "VALUE", 1, DIM);
    lbl(&mut img, shi, speed_box.0);
    lbl(&mut img, 0.0, speed_box.1);
    text(&mut img, 6, speed_box.0 as i64 + 14, "SPEED/S", 1, Rgba([SPEED_COLOR[0], SPEED_COLOR[1], SPEED_COLOR[2], 255]));
    for t in [t0, (t0 + t1) / 2.0, t1] {
        let label = format!("{t:.2}S");
        let x = (x_of(t) as i64 - i64::from(text_width(&label, 1)) / 2).clamp(0, i64::from(w) - i64::from(text_width(&label, 1)));
        text(&mut img, x, speed_box.1 as i64 + 8, &label, 1, DIM);
    }
    Some(img)
}

/// PNG bytes of an RGBA image.
pub fn png(img: &RgbaImage) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new(&mut out)
        .write_image(img.as_raw(), img.width(), img.height(), image::ExtendedColorType::Rgba8)
        .map_err(|e| format!("png: {e}"))?;
    Ok(out)
}

use image::ImageEncoder;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_drawn_and_clipped() {
        let mut img = RgbaImage::from_pixel(40, 10, BG);
        text(&mut img, 1, 1, "1.5S", 1, LABEL);
        assert!(img.pixels().any(|p| *p == LABEL));
        // Off-image text must not panic.
        text(&mut img, -50, -50, "OFF", 3, LABEL);
        text(&mut img, 39, 9, "ÉDGE", 2, LABEL);
        assert_eq!(text_width("AB", 1), 11);
    }

    #[test]
    fn filmstrip_layout() {
        let cell = |c| Cell { image: RgbaImage::from_pixel(30, 50, Rgba([c, 0, 0, 128])), caption: "0.00S".into() };
        let img = filmstrip(&[cell(10), cell(20), cell(30)], 2, true, "TEST");
        assert_eq!(img.width(), 8 + 2 * (30 + 8));
        assert!(img.height() > 2 * 50);
        assert!(filmstrip(&[], 4, false, "").width() > 0);
    }

    #[test]
    fn curves_render() {
        let times: Vec<f64> = (0..50).map(|i| f64::from(i) / 49.0).collect();
        let x: Vec<f64> = times.iter().map(|t| t * 100.0).collect();
        let s = Series { speed: vec![100.0; 50], dims: vec![("X".into(), x)], keys: vec![0.0, 1.0], times };
        let img = curves(&s, "SCALE", 600, 400);
        assert!(img.is_some_and(|i| i.width() == 600));
        let empty = Series { times: vec![], dims: vec![], speed: vec![], keys: vec![] };
        assert!(curves(&empty, "", 10, 10).is_some());
    }
}
