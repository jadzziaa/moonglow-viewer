//! The colour chooser for PLT colours, as Moonglow Toolset's: a swatch of
//! the colour's tones and a list of the palette's 176 colours to pick from.

use std::collections::HashMap;
use std::sync::Arc;

use egui::Ui;
use mg_core::ResRef;
use mg_image::Rgba;
use mgv_library::Library;

/// The game's palette images (`pal_cloth01`, …), each read once.
#[derive(Debug, Default)]
pub struct Palettes(HashMap<String, Option<Arc<Rgba>>>);

impl Palettes {
    pub fn get(&mut self, lib: &Library, name: &str) -> Option<Arc<Rgba>> {
        if let Some(p) = self.0.get(name) {
            return p.clone();
        }
        let rgba = ResRef::from_str(name).ok().and_then(|r| {
            let (t, data) = lib.resman().texture(r)?;
            Some(Arc::new(mg_image::read(t, &data).ok()?.to_rgba()))
        });
        self.0.insert(name.to_string(), rgba.clone());
        rgba
    }

    /// Forgets them (the library was read again).
    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// Each PLT layer's palette.
    pub fn layers(&mut self, lib: &Library) -> [Option<Arc<Rgba>>; 10] {
        mg_image::plt::PALETTES.map(|name| self.get(lib, name))
    }
}

/// A palette colour's tones: its row (`index` counted from the top) at
/// four grey levels.
fn tones(palette: &Rgba, index: u8) -> [egui::Color32; 4] {
    let rows = palette.height.max(1);
    let y = rows - 1 - u32::from(index).min(rows - 1);
    [64u32, 128, 192, 250].map(|x| {
        if palette.width == 0 || palette.height == 0 {
            return egui::Color32::GRAY;
        }
        let [r, g, b, _] = palette.pixel(x.min(palette.width - 1), y);
        egui::Color32::from_rgb(r, g, b)
    })
}

/// Paints a swatch: the tones side by side.
fn swatch(ui: &Ui, rect: egui::Rect, tones: &[egui::Color32; 4]) {
    let w = rect.width() / tones.len() as f32;
    for (i, c) in tones.iter().enumerate() {
        let r = egui::Rect::from_min_size(
            rect.min + egui::vec2(w * i as f32, 0.0),
            egui::vec2(w, rect.height()),
        );
        ui.painter().rect_filled(r, 0.0, *c);
    }
}

/// How many colours a palette has.
pub const COLORS: u8 = 176;

/// A PLT colour (0–175) picked from a palette's swatches: `what` names it
/// (the swatches are "`what` N"). Without the palette (no game), a number
/// to drag. Returns whether it changed.
pub fn pick(ui: &mut Ui, palette: Option<&Rgba>, what: &str, color: &mut u8) -> bool {
    let Some(p) = palette else {
        return ui.add(egui::DragValue::new(color).range(0..=COLORS - 1)).changed();
    };
    let current = (*color).min(COLORS - 1);
    let mut pick = None;
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(28.0, 16.0), egui::Sense::hover());
        swatch(ui, rect, &tones(p, current));
        let combo = egui::ComboBox::from_id_salt(("palette color", what))
            .selected_text(current.to_string())
            .width(56.0)
            .height(11.0 * 16.0 + 24.0)
            .show_ui(ui, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(2.0, 2.0);
                // 176 colours, 16 to a row.
                for row in 0..COLORS / 16 {
                    ui.horizontal(|ui| {
                        for i in row * 16..row * 16 + 16 {
                            let size = egui::vec2(16.0, 14.0);
                            let (rect, r) = ui.allocate_exact_size(size, egui::Sense::click());
                            swatch(ui, rect, &tones(p, i));
                            if i == current {
                                let stroke = ui.visuals().selection.stroke;
                                ui.painter().rect_stroke(
                                    rect,
                                    0.0,
                                    stroke,
                                    egui::StrokeKind::Outside,
                                );
                            }
                            let name = format!("{what} {i}");
                            r.widget_info(|| {
                                egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name)
                            });
                            if r.on_hover_text(&name).clicked() {
                                pick = Some(i);
                                ui.close();
                            }
                        }
                    });
                }
            });
        combo.response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, true, format!("{what} color"))
        });
    });
    match pick.filter(|&v| v != *color) {
        Some(v) => {
            *color = v;
            true
        }
        None => false,
    }
}
