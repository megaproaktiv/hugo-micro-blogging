//! The hmb icon: a white "µ" on a blue rounded square (like the logo in the app's top bar).
//! Drawn in code so no image file is needed; used for the Dock / window and the menu bar.

/// Distance from point p to the segment a–b.
fn seg(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (px, py) = (p.0 - a.0, p.1 - a.1);
    let (bx, by) = (b.0 - a.0, b.1 - a.1);
    let t = ((px * bx + py * by) / (bx * bx + by * by)).clamp(0.0, 1.0);
    let (dx, dy) = (px - bx * t, py - by * t);
    (dx * dx + dy * dy).sqrt()
}

/// Distance from p to the lower half of a circle (the bowl of the µ).
fn lower_arc(p: (f32, f32), c: (f32, f32), r: f32) -> f32 {
    if p.1 >= c.1 {
        (((p.0 - c.0).powi(2) + (p.1 - c.1).powi(2)).sqrt() - r).abs()
    } else {
        let l = ((p.0 - (c.0 - r)).powi(2) + (p.1 - c.1).powi(2)).sqrt();
        let rr = ((p.0 - (c.0 + r)).powi(2) + (p.1 - c.1).powi(2)).sqrt();
        l.min(rr)
    }
}

/// Distance from p (in a 0..1 box) to the strokes of the µ.
fn mu_distance(p: (f32, f32)) -> f32 {
    let (left, right, top, bowl_y, r) = (0.33, 0.65, 0.27, 0.56, 0.16);
    let stem = seg(p, (left, top), (left, 0.84)); // long left stem with descender
    let right_stem = seg(p, (right, top), (right, 0.69)); // right stem
    let tail = seg(p, (right, 0.69), (0.72, 0.73)); // small foot on the right
    let bowl = lower_arc(p, ((left + right) / 2.0, bowl_y), r);
    stem.min(right_stem).min(tail).min(bowl)
}

/// Coverage (0..1) of the µ strokes at pixel (x, y) of a `size`×`size` image, glyph box `[x0, x0+w]`.
fn mu_coverage(x: f32, y: f32, size: f32, x0: f32, w: f32, stroke: f32) -> f32 {
    let p = ((x - x0) / w, (y - x0) / w);
    let d = mu_distance(p) * w;
    (stroke / 2.0 - d + 0.5).clamp(0.0, 1.0) * if size > 0.0 { 1.0 } else { 0.0 }
}

/// App icon (Dock / window): blue rounded square with a white µ, RGBA.
pub fn app_icon_rgba(size: u32) -> Vec<u8> {
    let s = size as f32;
    let mut px = vec![0u8; (size * size * 4) as usize];
    let margin = s * 0.09; // macOS icons leave some room around the square
    let (l, t, r, b) = (margin, margin, s - margin, s - margin);
    let rad = s * 0.2;
    let (blue, blue_dark) = ([37.0, 99.0, 235.0], [29.0, 78.0, 216.0]);
    for y in 0..size {
        for x in 0..size {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            // rounded square, anti-aliased
            let cx = fx.clamp(l + rad, r - rad);
            let cy = fy.clamp(t + rad, b - rad);
            let d = ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt();
            let a = (rad - d + 0.5).clamp(0.0, 1.0);
            if a <= 0.0 {
                continue;
            }
            // slight vertical gradient
            let g = ((fy - t) / (b - t)).clamp(0.0, 1.0);
            let mut c = [
                blue[0] + (blue_dark[0] - blue[0]) * g,
                blue[1] + (blue_dark[1] - blue[1]) * g,
                blue[2] + (blue_dark[2] - blue[2]) * g,
            ];
            let m = mu_coverage(fx, fy, s, l + (r - l) * 0.12, (r - l) * 0.76, s * 0.085);
            for ch in c.iter_mut() {
                *ch += (255.0 - *ch) * m;
            }
            let i = ((y * size + x) * 4) as usize;
            px[i] = c[0] as u8;
            px[i + 1] = c[1] as u8;
            px[i + 2] = c[2] as u8;
            px[i + 3] = (a * 255.0) as u8;
        }
    }
    px
}

/// Menu-bar icon: a black µ on transparent (used as a template image on macOS).
pub fn tray_icon_rgba(size: u32) -> Vec<u8> {
    let s = size as f32;
    let mut px = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let m = mu_coverage(x as f32 + 0.5, y as f32 + 0.5, s, s * 0.02, s * 0.96, s * 0.11);
            let i = ((y * size + x) * 4) as usize;
            px[i + 3] = (m * 255.0) as u8;
        }
    }
    px
}

/// Writes the PNG files of a macOS `.iconset` folder (turned into `hmb.icns` by `iconutil`).
pub fn export_iconset(dir: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    for (name, size) in [
        ("icon_16x16", 16),
        ("icon_16x16@2x", 32),
        ("icon_32x32", 32),
        ("icon_32x32@2x", 64),
        ("icon_128x128", 128),
        ("icon_128x128@2x", 256),
        ("icon_256x256", 256),
        ("icon_256x256@2x", 512),
        ("icon_512x512", 512),
        ("icon_512x512@2x", 1024),
    ] {
        let px = app_icon_rgba(size);
        image::save_buffer(dir.join(format!("{name}.png")), &px, size, size, image::ColorType::Rgba8)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn app_icon() -> eframe::egui::IconData {
    let size = 256;
    eframe::egui::IconData { rgba: app_icon_rgba(size), width: size, height: size }
}

#[cfg(test)]
mod tests {
    /// `HMB_ICON_OUT=shots cargo test icon` writes the icons as PNG files to look at.
    #[test]
    fn icon_pngs() {
        let app = super::app_icon_rgba(256);
        assert!(app.chunks(4).any(|p| p == [255, 255, 255, 255]), "white µ strokes present");
        let Ok(dir) = std::env::var("HMB_ICON_OUT") else { return };
        let _ = std::fs::create_dir_all(&dir);
        image::save_buffer(format!("{dir}/icon.png"), &app, 256, 256, image::ColorType::Rgba8).unwrap();
        let tray = super::tray_icon_rgba(44);
        image::save_buffer(format!("{dir}/tray.png"), &tray, 44, 44, image::ColorType::Rgba8).unwrap();
    }
}
