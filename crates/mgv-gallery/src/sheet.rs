//! Contact sheets: many models in one picture, a tile each with its name
//! and size under it, for looking over a batch at once.

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use mg_image::Rgba;
use mg_render::Gpu;
use mgv_library::Library;
use mgv_stage::headless::{self, Shot};
use mgv_stage::{Stage, Viewport};

use crate::Item;

/// How a sheet is made.
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    /// Each tile's picture (`size` is a tile's).
    pub shot: Shot,
    /// Tiles in a row (0: as square a sheet as the items make).
    pub columns: usize,
    /// Names and sizes under the tiles.
    pub labels: bool,
}

/// A sheet and what could not be shown (label and reason).
#[derive(Debug, Clone, PartialEq)]
pub struct Sheet {
    pub image: Rgba,
    pub failed: Vec<(String, String)>,
}

/// The text's colours (gamma), over the label strip's.
const TEXT: [u8; 3] = [230, 232, 236];
const DIM: [u8; 3] = [150, 156, 166];
const ERROR: [u8; 3] = [240, 110, 100];
const STRIP: [u8; 3] = [24, 27, 33];

fn font() -> FontRef<'static> {
    FontRef::try_from_slice(epaint_default_fonts::HACK_REGULAR).expect("the bundled font")
}

/// The width of `text` at `px` pixels high.
fn text_width(font: &FontRef<'_>, px: f32, text: &str) -> f32 {
    let scaled = font.as_scaled(PxScale::from(px));
    text.chars().map(|c| scaled.h_advance(font.glyph_id(c))).sum()
}

/// Draws `text` with its left edge at `x` and its line's top at `y`.
fn draw_text(img: &mut Rgba, font: &FontRef<'_>, px: f32, x: f32, y: f32, text: &str, c: [u8; 3]) {
    let scale = PxScale::from(px);
    let scaled = font.as_scaled(scale);
    let baseline = y + scaled.ascent();
    let mut pen = x;
    for ch in text.chars() {
        let id = font.glyph_id(ch);
        let glyph = id.with_scale_and_position(scale, ab_glyph::point(pen, baseline));
        pen += scaled.h_advance(id);
        let Some(outline) = font.outline_glyph(glyph) else { continue };
        let bounds = outline.px_bounds();
        outline.draw(|gx, gy, cover| {
            let (px, py) =
                (bounds.min.x as i64 + i64::from(gx), bounds.min.y as i64 + i64::from(gy));
            if px < 0 || py < 0 || px >= i64::from(img.width) || py >= i64::from(img.height) {
                return;
            }
            let i = (py as usize * img.width as usize + px as usize) * 4;
            let a = cover.clamp(0.0, 1.0);
            for (under, over) in img.data[i..i + 3].iter_mut().zip(c) {
                let u = f32::from(*under);
                *under = (u + (f32::from(over) - u) * a).round() as u8;
            }
            img.data[i + 3] = 255;
        });
    }
}

/// `text`, cut with an ellipsis to fit `width` pixels.
fn fitted(font: &FontRef<'_>, px: f32, text: &str, width: f32) -> String {
    if text_width(font, px, text) <= width {
        return text.to_string();
    }
    let mut out: Vec<char> = text.chars().collect();
    while !out.is_empty() {
        out.pop();
        let cut: String = out.iter().collect::<String>() + "…";
        if text_width(font, px, &cut) <= width {
            return cut;
        }
    }
    String::new()
}

/// Width, depth and height in metres, as `1.2 × 0.8 × 2.0 m`.
fn dimensions(stage: &Stage) -> Option<String> {
    let (min, max) = stage.bounds()?;
    let d = max - min;
    Some(format!("{:.1} × {:.1} × {:.1} m", d.x, d.y, d.z))
}

fn fill(img: &mut Rgba, x0: u32, y0: u32, w: u32, h: u32, c: [u8; 3]) {
    for y in y0..(y0 + h).min(img.height) {
        for x in x0..(x0 + w).min(img.width) {
            let i = (y as usize * img.width as usize + x as usize) * 4;
            img.data[i..i + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
        }
    }
}

fn blit(img: &mut Rgba, tile: &Rgba, x0: u32, y0: u32) {
    for y in 0..tile.height.min(img.height.saturating_sub(y0)) {
        let w = tile.width.min(img.width.saturating_sub(x0)) as usize;
        let from = y as usize * tile.width as usize * 4;
        let to = ((y0 + y) as usize * img.width as usize + x0 as usize) * 4;
        img.data[to..to + w * 4].copy_from_slice(&tile.data[from..from + w * 4]);
    }
}

/// The height of the strip under a tile `width` pixels wide: two lines.
fn strip_height(width: u32) -> (u32, f32) {
    let px = (width as f32 / 16.0).clamp(11.0, 28.0);
    ((px * 2.7).ceil() as u32, px)
}

/// Renders `items` into one picture, row by row. Items that cannot be shown
/// keep their tile, with the reason in it.
pub fn render(
    lib: &mut Library,
    gpu: &Gpu,
    items: &[Item],
    opts: &Options,
    progress: &mut dyn FnMut(usize, usize, &Item),
) -> Sheet {
    let (tw, th) = opts.shot.size;
    let (strip, px) = if opts.labels { strip_height(tw) } else { (0, 0.0) };
    let columns = match opts.columns {
        0 => (items.len() as f32).sqrt().ceil() as usize,
        n => n,
    }
    .clamp(1, items.len().max(1));
    let rows = items.len().div_ceil(columns).max(1);
    let background = opts.shot.background.unwrap_or(mgv_stage::Lighting::studio().background);
    let bg = background.map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8);
    let mut image = Rgba::new(tw * columns as u32, (th + strip) * rows as u32);
    let (w, h) = (image.width, image.height);
    fill(&mut image, 0, 0, w, h, bg);
    let font = font();
    let mut stage = Stage::new(gpu.clone());
    let mut viewport = Viewport::new(gpu);
    let mut failed = Vec::new();
    for (i, item) in items.iter().enumerate() {
        progress(i, items.len(), item);
        let (x0, y0) = ((i % columns) as u32 * tw, (i / columns) as u32 * (th + strip));
        let shown = crate::show(&mut stage, lib, item);
        let second = match &shown {
            Ok(()) => {
                let tile = headless::still(&mut stage, &mut viewport, lib, &opts.shot);
                blit(&mut image, &tile, x0, y0);
                (dimensions(&stage).unwrap_or_default(), DIM)
            }
            Err(e) => {
                failed.push((item.label.clone(), e.clone()));
                (e.clone(), ERROR)
            }
        };
        if opts.labels {
            fill(&mut image, x0, y0 + th, tw, strip, STRIP);
            let (pad, room) = (px * 0.4, tw as f32 - px * 0.8);
            let y = (y0 + th) as f32 + px * 0.15;
            let name = fitted(&font, px, &item.label, room);
            draw_text(&mut image, &font, px, x0 as f32 + pad, y, &name, TEXT);
            let small = px * 0.85;
            let line = fitted(&font, small, &second.0, room);
            draw_text(&mut image, &font, small, x0 as f32 + pad, y + px * 1.25, &line, second.1);
        }
    }
    Sheet { image, failed }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_drawn_and_cut_to_fit() {
        let font = font();
        let mut img = Rgba::new(120, 24);
        draw_text(&mut img, &font, 14.0, 2.0, 2.0, "plc_a01", TEXT);
        assert!(img.data.as_chunks::<4>().0.iter().any(|p| p[0] > 200), "something is drawn");
        // Past the edges: nothing written out of bounds.
        draw_text(&mut img, &font, 14.0, 110.0, 18.0, "wide text", TEXT);
        let cut = fitted(&font, 14.0, "a_very_long_model_name_indeed", 80.0);
        assert!(cut.ends_with('…') && text_width(&font, 14.0, &cut) <= 80.0, "{cut}");
        assert_eq!(fitted(&font, 14.0, "short", 200.0), "short");
    }

    #[test]
    fn strips_grow_with_their_tiles() {
        assert!(strip_height(128).0 < strip_height(512).0);
        assert_eq!(strip_height(4096).1, 28.0);
    }
}
