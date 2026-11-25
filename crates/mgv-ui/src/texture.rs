//! The texture view: an image with its mip levels, channels and alpha, its
//! TXI, and for PLTs the ten layers coloured from the game's palettes.

use egui::{Color32, ColorImage, TextureHandle, TextureOptions, Ui};
use mg_core::ResType;
use mg_image::plt::{LAYERS, PALETTES, Plt};
use mg_image::{Format, Rgba, Texture};
use mg_resman::ResKey;
use mg_uikit::TokensExt;
use mgv_library::{Kind, Library, Opened};

/// Which channels show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channels {
    /// Colour over a checkerboard where it is transparent.
    Rgba,
    /// Colour, alpha ignored.
    Rgb,
    Red,
    Green,
    Blue,
    Alpha,
}

/// What the shown image was made from: mip level, channels, PLT colours.
type Shown = (usize, Channels, [u8; 10]);

/// An open texture.
pub struct TextureView {
    name: String,
    info: String,
    image: Option<Texture>,
    plt: Option<(Plt, Vec<Option<Rgba>>)>,
    pub colors: [u8; 10],
    txi: Option<String>,
    pub mip: usize,
    pub channels: Channels,
    zoom: f32,
    handle: Option<(TextureHandle, Shown)>,
    error: Option<String>,
}

impl std::fmt::Debug for TextureView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextureView").field("name", &self.name).finish_non_exhaustive()
    }
}

impl TextureView {
    pub fn new(lib: &Library, opened: &Opened) -> TextureView {
        let name = opened.name();
        let mut v = TextureView {
            name: name.clone(),
            info: String::new(),
            image: None,
            plt: None,
            colors: [0; 10],
            txi: None,
            mip: 0,
            channels: Channels::Rgba,
            zoom: 1.0,
            handle: None,
            error: None,
        };
        let restype = opened.key.map(|k| k.restype).or_else(|| {
            let ext = opened.path.as_ref()?.extension()?.to_str()?.to_ascii_lowercase();
            ResType::from_extension(&ext)
        });
        match opened.kind {
            Kind::Plt => match Plt::read(&opened.data) {
                Ok(p) => {
                    let palettes = PALETTES
                        .iter()
                        .map(|n| {
                            let r = mg_core::ResRef::from_str(n).ok()?;
                            let (t, d) = lib.resman().texture(r)?;
                            mg_image::read(t, &d).ok().map(|t| t.to_rgba())
                        })
                        .collect();
                    let used: Vec<&str> = p
                        .layers()
                        .iter()
                        .zip(LAYERS)
                        .filter(|(u, _)| **u)
                        .map(|(_, n)| n)
                        .collect();
                    v.info = format!("PLT {}×{}, layers: {}", p.width, p.height, used.join(", "));
                    v.plt = Some((p, palettes));
                }
                Err(e) => v.error = Some(e.to_string()),
            },
            _ => {
                // A DDS may be named .tga and the other way round: the
                // content decides.
                let t = if opened.data.starts_with(b"DDS ") {
                    ResType::DDS
                } else {
                    restype.unwrap_or(ResType::TGA)
                };
                match mg_image::read(t, &opened.data)
                    .or_else(|_| mg_image::read(other(t), &opened.data))
                {
                    Ok(tex) => {
                        v.info = format!(
                            "{:?} {}×{}, {} mip levels{}",
                            tex.format,
                            tex.width,
                            tex.height,
                            tex.mips.len(),
                            if tex.has_alpha { ", alpha" } else { "" }
                        );
                        v.image = Some(tex);
                    }
                    Err(e) => v.error = Some(e.to_string()),
                }
            }
        }
        v.txi = mg_core::ResRef::from_str(&name)
            .ok()
            .and_then(|r| lib.get(&ResKey::new(r, ResType::TXI)))
            .map(|d| String::from_utf8_lossy(&d).into_owned());
        v
    }

    /// The pixels to show, rows top first.
    fn pixels(&self) -> Option<Rgba> {
        if let Some((p, palettes)) = &self.plt {
            let refs: [Option<&Rgba>; 10] =
                std::array::from_fn(|i| palettes.get(i).and_then(Option::as_ref));
            return Some(p.colorize(&refs, self.colors).top_down());
        }
        let t = self.image.as_ref()?;
        let level = self.mip.min(t.mips.len().saturating_sub(1));
        let (w, h) = t.level_dims(level);
        let data = match t.format {
            Format::Rgba8 => t.mips[level].clone(),
            f => mg_image::bc::decode(f, w, h, &t.mips[level]),
        };
        let mut img = Rgba { width: w, height: h, data };
        if !t.has_alpha {
            for px in img.data.as_chunks_mut::<4>().0 {
                px[3] = 255;
            }
        }
        Some(img.top_down())
    }

    fn color_image(&self) -> Option<ColorImage> {
        let img = self.pixels()?;
        let checker =
            |x: u32, y: u32| if ((x / 8) + (y / 8)).is_multiple_of(2) { 0x99 } else { 0x66 };
        let pixels: Vec<Color32> = img
            .data
            .as_chunks::<4>()
            .0
            .iter()
            .enumerate()
            .map(|(i, px)| {
                let (x, y) = (i as u32 % img.width, i as u32 / img.width);
                let [r, g, b, a] = [px[0], px[1], px[2], px[3]];
                match self.channels {
                    Channels::Rgba => {
                        let c = checker(x, y) as f32;
                        let k = a as f32 / 255.0;
                        let mix = |v: u8| (v as f32 * k + c * (1.0 - k)).round() as u8;
                        Color32::from_rgb(mix(r), mix(g), mix(b))
                    }
                    Channels::Rgb => Color32::from_rgb(r, g, b),
                    Channels::Red => Color32::from_gray(r),
                    Channels::Green => Color32::from_gray(g),
                    Channels::Blue => Color32::from_gray(b),
                    Channels::Alpha => Color32::from_gray(a),
                }
            })
            .collect();
        Some(ColorImage::new([img.width as usize, img.height as usize], pixels))
    }
}

fn other(t: ResType) -> ResType {
    if t == ResType::DDS { ResType::TGA } else { ResType::DDS }
}

/// The Texture's header's name (its record, `mg_uikit::header`).
pub(crate) const HEADER: &str = "Texture header";

/// The channel switch's choices: what each shows, its words and its tip.
const CHANNELS: [(Channels, &str, &str); 6] = [
    (Channels::Rgba, "RGBA", "Color over a checkerboard where it is transparent"),
    (Channels::Rgb, "RGB", "Color, alpha ignored"),
    (Channels::Red, "R", "Red channel"),
    (Channels::Green, "G", "Green channel"),
    (Channels::Blue, "B", "Blue channel"),
    (Channels::Alpha, "A", "Alpha channel"),
];

/// A mini slider's width in the header (the board's `.mini-slider`).
const MINI_SLIDER: f32 = 60.0;
/// A number's width beside it (`.mini-num`).
const MINI_NUMBER: f32 = 48.0;
/// The caption line's height (`.tex-cap`).
const CAPTION: f32 = 24.0;
/// The room round the picture (`.tex-body`).
const BODY_MARGIN: i8 = 8;

/// The Texture tab (the board "Creature, Colors, Texture"'s second
/// group): its header (the channel switch at the left; Mip and Zoom,
/// each a slider and a number, at the right), the caption line with the
/// texture's name and what it is, then the picture (Ctrl + Wheel zooms),
/// a PLT's layers' colors and its TXI.
pub(crate) fn ui(app: &mut crate::Viewer, ui: &mut Ui) {
    let Some(v) = &mut app.texture else {
        egui::Frame::new().inner_margin(egui::Margin::same(BODY_MARGIN)).show(ui, |ui| {
            ui.weak("Textures (TGA, DDS, PLT) open here.");
        });
        return;
    };
    header(v, ui);
    caption(ui, &v.name, &v.info);
    egui::Frame::new().inner_margin(egui::Margin::same(BODY_MARGIN)).show(ui, |ui| body(v, ui));
}

/// The Texture's header.
fn header(v: &mut TextureView, ui: &mut Ui) {
    use mg_uikit::header::{EditorHeader, Item};
    let style = egui::TextStyle::Button.resolve(ui.style());
    let words =
        |w: &str| ui.painter().layout_no_wrap(w.into(), style.clone(), Color32::WHITE).size().x;
    let pad = 2.0 * mg_uikit::metrics::CONTROL_PADDING_X;
    let channels_width = CHANNELS.iter().map(|c| words(c.1) + pad + 1.0).sum::<f32>() + 2.0;
    let gap = ui.spacing().item_spacing.x;
    let mip_width = words("Mip") + MINI_SLIDER + MINI_NUMBER + 2.0 * gap;
    let zoom_width = words("Zoom") + MINI_SLIDER + MINI_NUMBER + 2.0 * gap;
    let mips = v.image.as_ref().map_or(1, |t| t.mips.len());
    let channels = &mut v.channels;
    // (Mip and Zoom drawn on the row or in More where the row is short:
    // each of the two forms sets the one value.)
    let mip = std::cell::Cell::new(v.mip);
    let zoom = std::cell::Cell::new(v.zoom);
    let label = |ui: &mut Ui, w: &str| ui.label(egui::RichText::new(w).color(ui.tokens().text_2));
    let mip_ui = |ui: &mut Ui| {
        let mut m = mip.get();
        label(ui, "Mip");
        let most = mips.saturating_sub(1);
        ui.spacing_mut().slider_width = MINI_SLIDER;
        let slider = egui::Slider::new(&mut m, 0..=most).show_value(false);
        mg_uikit::slider(ui, slider)
            .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Slider, true, "Mip level"));
        let number = egui::DragValue::new(&mut m).range(0..=most);
        ui.add_sized([MINI_NUMBER, ui.spacing().interact_size.y], number);
        mip.set(m);
    };
    let zoom_ui = |ui: &mut Ui| {
        let mut z = zoom.get();
        label(ui, "Zoom");
        ui.spacing_mut().slider_width = MINI_SLIDER;
        let slider = egui::Slider::new(&mut z, 0.125..=8.0).logarithmic(true).show_value(false);
        mg_uikit::slider(ui, slider)
            .on_hover_text("Ctrl + Wheel over the picture")
            .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Slider, true, "Zoom"));
        let number = egui::DragValue::new(&mut z).range(0.125..=8.0).speed(0.01).fixed_decimals(2);
        ui.add_sized([MINI_NUMBER, ui.spacing().interact_size.y], number);
        zoom.set(z);
    };
    EditorHeader::new(HEADER)
        .left(Item::custom("Channels", channels_width, |ui| {
            mg_uikit::segmented(ui, "Channels", channels, &CHANNELS);
        }))
        .right(Item::custom("Mip", mip_width, mip_ui).more(mip_width, mip_ui).enabled(mips > 1))
        .right(Item::separator())
        .right(Item::custom("Zoom", zoom_width, zoom_ui).more(zoom_width, zoom_ui))
        .show(ui);
    v.mip = mip.get();
    v.zoom = zoom.get();
}

/// The caption line (the board's `.tex-cap`): the texture's name in
/// `text`, what it is (format, size, mip levels, alpha; a PLT's layers)
/// in `text-3`, on the panel with a line under it.
pub(crate) fn caption(ui: &mut Ui, name: &str, info: &str) {
    let t = ui.tokens();
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), CAPTION), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, t.panel);
    painter.hline(rect.x_range(), rect.bottom() - 0.5, egui::Stroke::new(1.0, t.line_soft));
    let font = egui::FontId::proportional(mg_uikit::metrics::BODY_SIZE);
    let name_galley = painter.layout_no_wrap(name.to_string(), font.clone(), t.text);
    let y = |h: f32| (rect.center().y - h / 2.0).round();
    let x = rect.left() + 8.0;
    let name_width = name_galley.size().x;
    painter.galley(egui::pos2(x, y(name_galley.size().y)), name_galley, t.text);
    let info_galley = painter.layout_no_wrap(info.to_string(), font, t.text_3);
    let at = egui::pos2(x + name_width + 8.0, y(info_galley.size().y));
    painter.galley(at, info_galley, t.text_3);
    let words = format!("{name} {info}");
    let r = ui.interact(rect, ui.id().with("texture-caption"), egui::Sense::hover());
    r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &words));
}

/// The picture, a PLT's layers' colors and the TXI.
fn body(v: &mut TextureView, ui: &mut Ui) {
    if let Some(e) = &v.error {
        ui.colored_label(ui.visuals().error_fg_color, e);
        return;
    }
    if let Some((p, _)) = &v.plt {
        let used = p.layers();
        ui.horizontal_wrapped(|ui| {
            for (i, name) in LAYERS.iter().enumerate().filter(|(i, _)| used[*i]) {
                ui.add(
                    egui::DragValue::new(&mut v.colors[i])
                        .range(0..=175)
                        .prefix(format!("{name} ")),
                );
            }
        });
    }
    let key = (v.mip, v.channels, v.colors);
    if v.handle.as_ref().is_none_or(|(_, k)| *k != key)
        && let Some(img) = v.color_image()
    {
        let h = ui.ctx().load_texture(format!("tex-{}", v.name), img, TextureOptions::NEAREST);
        v.handle = Some((h, key));
    }
    let Some((h, _)) = &v.handle else { return };
    let size = h.size_vec2() * v.zoom;
    let txi = v.txi.clone();
    // (The TXI under the picture, the picture's room the rest.)
    let below = if txi.is_some() { 120.0 } else { 0.0 };
    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .max_height((ui.available_height() - below).max(80.0))
        .show(ui, |ui| {
            let r = ui.add(egui::Image::new((h.id(), size)).sense(egui::Sense::hover()));
            if r.hovered() {
                let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                if scroll != 0.0 && ui.input(|i| i.modifiers.command) {
                    v.zoom = (v.zoom * (scroll * 0.003).exp()).clamp(0.125, 8.0);
                }
            }
        });
    if let Some(txi) = &txi {
        crate::panels::next_section(ui, "TXI");
        ui.code(txi);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Action, NoDialogs, Settings, Viewer};
    use mg_uikit::header::{self, Place as At};

    /// An uncompressed TGA `w` by `h`, 32 bits, gray and half transparent.
    fn tga(w: u16, h: u16) -> Vec<u8> {
        let mut d = vec![0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        d.extend(w.to_le_bytes());
        d.extend(h.to_le_bytes());
        d.extend([32, 8]);
        for _ in 0..u32::from(w) * u32::from(h) {
            d.extend([90, 100, 110, 128]);
        }
        d
    }

    /// The Texture tab's header (the channel switch; Mip and Zoom, on the
    /// row or in More, never cut) and its caption line with the name and
    /// what it is, drawn without a GPU at the board's size and the wide
    /// one, in both themes; the channel switch chooses.
    #[test]
    fn the_texture_header_and_caption_line() {
        let dir = std::env::temp_dir().join(format!("mgv-ui-texture-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("stone.tga");
        std::fs::write(&file, tga(8, 8)).unwrap();
        for theme in mg_uikit::Theme::ALL {
            for size in [(1600.0, 900.0), (3440.0, 1440.0)] {
                let size = egui::vec2(size.0, size.1);
                let settings = Settings {
                    game_root: Some(std::path::PathBuf::from("/nonexistent")),
                    use_user_dir: false,
                    theme,
                    ..Settings::default()
                };
                let mut v = Viewer::new(settings, Box::new(NoDialogs));
                v.actions.push(Action::Open(file.clone()));
                let ctx = egui::Context::default();
                ctx.enable_accesskit();
                header::record_headers(&ctx);
                let mut caption = None;
                for _ in 0..4 {
                    let input = egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                        ..Default::default()
                    };
                    let mut out = ctx.run_ui(input, |ui| v.ui(ui));
                    out.textures_delta.clear();
                    v.run_actions();
                    let update = out.platform_output.accesskit_update.as_ref();
                    caption = update.and_then(|u| {
                        u.nodes.iter().find_map(|(_, n)| {
                            let words = n.label().or(n.value());
                            words.filter(|l| l.starts_with("stone ")).map(str::to_string)
                        })
                    });
                }
                assert!(v.texture.is_some(), "{size:?}: the texture opened");
                assert_eq!(caption.as_deref(), Some("stone Rgba8 8×8, 1 mip levels, alpha"));
                let places = header::places(&ctx, HEADER);
                let at = |name: &str| places.iter().find(|(n, _)| n == name).map(|p| p.1);
                assert_eq!(at("Channels"), Some(At::Row), "{size:?}");
                for name in ["Mip", "Zoom"] {
                    assert!(matches!(at(name), Some(At::Row | At::More)), "{size:?}: {name}");
                }
                if size.x >= mg_uikit::shell::WIDE_FROM {
                    assert_eq!(at("Zoom"), Some(At::Row), "all on the wide row");
                }
                v.texture.as_mut().unwrap().channels = Channels::Alpha;
            }
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
