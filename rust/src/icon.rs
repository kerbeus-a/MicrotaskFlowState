//! App icon, drawn procedurally so the window, taskbar and .exe all share one source.
//! Shared with build.rs (via `#[path]`), so this file must not depend on any crate.

/// Gradient start (top-left) and end (bottom-right) of the icon background.
const FROM: [f32; 3] = [124.0, 92.0, 255.0];
const TO: [f32; 3] = [34.0, 211.0, 238.0];

fn sd_round_box(px: f32, py: f32, half: f32, r: f32) -> f32 {
    let qx = px.abs() - half + r;
    let qy = py.abs() - half + r;
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    outside + qx.max(qy).min(0.0) - r
}

fn sd_segment(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32, r: f32) -> f32 {
    let (pax, pay, bax, bay) = (px - ax, py - ay, bx - ax, by - ay);
    let h = ((pax * bax + pay * bay) / (bax * bax + bay * bay)).clamp(0.0, 1.0);
    ((pax - bax * h).powi(2) + (pay - bay * h).powi(2)).sqrt() - r
}

/// True when the normalized point (0..1) lies on the white microphone glyph.
fn on_glyph(x: f32, y: f32) -> bool {
    // Capsule body
    if sd_segment(x, y, 0.5, 0.31, 0.5, 0.47, 0.115) <= 0.0 {
        return true;
    }
    // U-shaped holder (lower half of a ring)
    let (cx, cy, radius, thickness) = (0.5, 0.47, 0.205, 0.04);
    let d = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
    if y >= cy && (d - radius).abs() <= thickness {
        return true;
    }
    // Stem and base
    sd_segment(x, y, 0.5, 0.67, 0.5, 0.77, 0.04) <= 0.0
        || sd_segment(x, y, 0.39, 0.78, 0.61, 0.78, 0.04) <= 0.0
}

/// Render the icon as straight (non-premultiplied) RGBA8, `size * size * 4` bytes.
pub fn render_rgba(size: u32) -> Vec<u8> {
    const SS: u32 = 4; // supersampling per axis for anti-aliasing
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    let s = size as f32;
    for py in 0..size {
        for px in 0..size {
            let (mut bg, mut glyph) = (0u32, 0u32);
            for sy in 0..SS {
                for sx in 0..SS {
                    let x = (px as f32 + (sx as f32 + 0.5) / SS as f32) / s;
                    let y = (py as f32 + (sy as f32 + 0.5) / SS as f32) / s;
                    if sd_round_box(x - 0.5, y - 0.5, 0.47, 0.22) <= 0.0 {
                        bg += 1;
                        if on_glyph(x, y) {
                            glyph += 1;
                        }
                    }
                }
            }
            let total = (SS * SS) as f32;
            let t = ((px + py) as f32 / (2.0 * s)).clamp(0.0, 1.0);
            let mut rgb = [0.0f32; 3];
            for i in 0..3 {
                let grad = FROM[i] + (TO[i] - FROM[i]) * t;
                let g = if bg > 0 { glyph as f32 / bg as f32 } else { 0.0 };
                rgb[i] = grad * (1.0 - g) + 255.0 * g;
            }
            out.extend_from_slice(&[
                rgb[0] as u8,
                rgb[1] as u8,
                rgb[2] as u8,
                (bg as f32 / total * 255.0).round() as u8,
            ]);
        }
    }
    out
}

/// Encode a multi-resolution Windows .ico (32-bit BMP entries).
#[allow(dead_code)]
pub fn render_ico(sizes: &[u32]) -> Vec<u8> {
    let images: Vec<Vec<u8>> = sizes
        .iter()
        .map(|&size| {
            let rgba = render_rgba(size);
            let mask_row = ((size + 31) / 32 * 4) as usize;
            let mut img = Vec::new();
            // BITMAPINFOHEADER; height is doubled to account for the AND mask.
            img.extend_from_slice(&40u32.to_le_bytes());
            img.extend_from_slice(&(size as i32).to_le_bytes());
            img.extend_from_slice(&((size * 2) as i32).to_le_bytes());
            img.extend_from_slice(&1u16.to_le_bytes());
            img.extend_from_slice(&32u16.to_le_bytes());
            img.extend_from_slice(&[0u8; 24]);
            // Pixels: BGRA, bottom-up.
            for row in (0..size).rev() {
                for col in 0..size {
                    let i = ((row * size + col) * 4) as usize;
                    img.extend_from_slice(&[rgba[i + 2], rgba[i + 1], rgba[i], rgba[i + 3]]);
                }
            }
            img.extend(std::iter::repeat(0u8).take(mask_row * size as usize));
            img
        })
        .collect();

    let mut ico = Vec::new();
    ico.extend_from_slice(&[0, 0, 1, 0]);
    ico.extend_from_slice(&(sizes.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * sizes.len() as u32;
    for (&size, img) in sizes.iter().zip(&images) {
        let dim = if size >= 256 { 0 } else { size as u8 };
        ico.extend_from_slice(&[dim, dim, 0, 0]);
        ico.extend_from_slice(&1u16.to_le_bytes());
        ico.extend_from_slice(&32u16.to_le_bytes());
        ico.extend_from_slice(&(img.len() as u32).to_le_bytes());
        ico.extend_from_slice(&offset.to_le_bytes());
        offset += img.len() as u32;
    }
    for img in images {
        ico.extend(img);
    }
    ico
}
