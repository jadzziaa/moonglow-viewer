//! The texture view: an image with its mip levels, channels and alpha, its
//! TXI, and for PLTs the ten layers coloured from the game's palettes.

use egui::{Color32, ColorImage, TextureHandle, TextureOptions, Ui};
use mg_core::ResType;
use mg_image::plt::{LAYERS, PALETTES, Plt};
use mg_image::{Format, Rgba, Texture};
use mg_resman::ResKey;
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

pub(crate) fn ui(app: &mut crate::Viewer, ui: &mut Ui) {
    let Some(v) = &mut app.texture else {
        ui.weak("Textures (TGA, DDS, PLT) open here.");
        return;
    };
    ui.horizontal(|ui| {
        ui.strong(&v.name);
        ui.weak(&v.info);
    });
    if let Some(e) = &v.error {
        ui.colored_label(ui.visuals().error_fg_color, e);
        return;
    }
    ui.horizontal(|ui| {
        for (c, n) in [
            (Channels::Rgba, "RGBA"),
            (Channels::Rgb, "RGB"),
            (Channels::Red, "R"),
            (Channels::Green, "G"),
            (Channels::Blue, "B"),
            (Channels::Alpha, "A"),
        ] {
            ui.selectable_value(&mut v.channels, c, n);
        }
        if let Some(t) = &v.image
            && t.mips.len() > 1
        {
            ui.add(egui::Slider::new(&mut v.mip, 0..=t.mips.len() - 1).text("mip"));
        }
        ui.add(egui::Slider::new(&mut v.zoom, 0.125..=8.0).logarithmic(true).text("zoom"));
    });
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
    egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
        let r = ui.add(egui::Image::new((h.id(), size)).sense(egui::Sense::hover()));
        if r.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 && ui.input(|i| i.modifiers.command) {
                v.zoom = (v.zoom * (scroll * 0.003).exp()).clamp(0.125, 8.0);
            }
        }
    });
    if let Some(txi) = &v.txi {
        crate::widgets::next_section(ui, "TXI");
        ui.code(txi);
    }
}
