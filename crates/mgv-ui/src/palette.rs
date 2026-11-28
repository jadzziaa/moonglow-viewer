//! The color chooser for PLT colors, as Moonglow Toolset's: a swatch of
//! the color's tones and a combo opening a grid of the palette's 176
//! colors to pick from.

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

/// A palette color's tones: its row (`index` counted from the top) at
/// four gray levels.
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

/// How many colors a palette has.
pub const COLORS: u8 = 176;

/// The swatch's tones' width: with the kit's swatch padding the swatch
/// is 28 wide (the board's `.tones`), so a swatch and its combo are 84,
/// the pair grid's value (`panels::PAIR_VALUE`).
const TONES: f32 = 28.0 - 2.0 * mg_uikit::SWATCH_PADDING;
/// The combo's width beside it (`.pal-combo`).
const COMBO: f32 = 52.0;

/// A PLT color (0 to 175) chosen from a palette (the board "Creature,
/// Colors"'s `.pick`): the kit's swatch button showing the color's tones,
/// and a combo with its number that opens on the palette's 176 colors,
/// sixteen to a row and eleven rows (`mg_uikit::palette_swatches`, each
/// named "`what` N"), the color shown outlined; the swatch opens it too.
/// The combo is named "`what` color". Without the palette (no game), a
/// number to drag. Whether the color changed.
pub fn pick(ui: &mut Ui, palette: Option<&Rgba>, what: &str, color: &mut u8) -> bool {
    use mg_uikit::TokensExt;
    let Some(p) = palette else {
        return ui.add(egui::DragValue::new(color).range(0..=COLORS - 1)).changed();
    };
    let current = (*color).min(COLORS - 1);
    let mut pick = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let salt = ("palette color", what);
        // The combo's own popup id: its button's id is the ui's id with the
        // salt hashed as an `IdSalt`, its popup's that id with "popup".
        let popup = ui.make_persistent_id(egui::IdSalt::new(salt)).with("popup");
        let open = egui::Popup::is_id_open(ui.ctx(), popup);
        let side = 2.0 * mg_uikit::SWATCH_PADDING;
        let size = egui::vec2(TONES + side, ui.spacing().interact_size.y.max(16.0 + side));
        let (rect, r) = ui.allocate_exact_size(size, egui::Sense::click());
        let name = format!("{what} swatch");
        r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::ColorWell, true, &name));
        let look = mg_uikit::SwatchLook { hovered: r.hovered(), open, focused: r.has_focus() };
        let t = ui.tokens();
        let tones = tones(p, current);
        mg_uikit::paint_swatch_button(ui.painter(), &t, rect, look, |painter, inner| {
            paint_tones(painter, inner, &tones);
        });
        if r.on_hover_text(format!("{what}: color {current}")).clicked() {
            egui::Popup::toggle_id(ui.ctx(), popup);
        }
        let combo = egui::ComboBox::from_id_salt(salt)
            .selected_text(current.to_string())
            .width(COMBO)
            .height(11.0 * 16.0 + 24.0)
            .show_ui(ui, |ui| {
                let swatches =
                    mg_uikit::palette_swatches(ui, |i| self::tones(p, i), what, current, true);
                if let Some(i) = swatches {
                    pick = Some(i);
                    ui.close();
                }
            });
        combo.response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::Role::ComboBox, true, format!("{what} color"))
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

/// Paints a color's tones side by side in `rect`.
fn paint_tones(painter: &egui::Painter, rect: egui::Rect, tones: &[egui::Color32; 4]) {
    let w = rect.width() / tones.len() as f32;
    for (i, c) in tones.iter().enumerate() {
        let x = rect.left() + w * i as f32;
        let r = egui::Rect::from_x_y_ranges(x..=x + w, rect.y_range());
        painter.rect_filled(r, 0.0, *c);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A palette whose row `r` is the gray `r` (rows counted from the
    /// bottom, as the game's are).
    pub(crate) fn palette() -> Rgba {
        let (width, height) = (256, u32::from(COLORS));
        let mut data = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height {
            for _ in 0..width {
                let g = (height - 1 - y) as u8;
                data.extend([g, g, g, 255]);
            }
        }
        Rgba { width, height, data }
    }

    /// One frame of the chooser for "Skin" at `color`, given `events`; its
    /// accesskit output.
    fn frame(
        ctx: &egui::Context,
        p: &Rgba,
        color: &mut u8,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(600.0, 500.0),
            )),
            events,
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            mg_uikit::apply(ui.ctx(), mg_uikit::Theme::Dark);
            egui::CentralPanel::default().show(ui, |ui| {
                pick(ui, Some(p), "Skin", color);
            });
        });
        out.textures_delta.clear();
        out
    }

    fn click(ctx: &egui::Context, p: &Rgba, color: &mut u8, at: egui::Pos2) -> egui::FullOutput {
        let button = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(ctx, p, color, vec![egui::Event::PointerMoved(at)]);
        frame(ctx, p, color, vec![button(true)]);
        frame(ctx, p, color, vec![button(false)]);
        frame(ctx, p, color, Vec::new())
    }

    /// The swatch opens the grid of the palette's 176 colors (16 by 11),
    /// whose click picks one and closes it; the combo opens it as well.
    #[test]
    fn the_swatch_grid_picks_a_color() {
        let named = crate::panels::tests::named_rect;
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let p = palette();
        let mut color = 3;
        frame(&ctx, &p, &mut color, Vec::new());
        let out = frame(&ctx, &p, &mut color, Vec::new());
        assert!(named(&out, "Skin 37").is_none(), "closed");
        let swatch = named(&out, "Skin swatch").expect("the swatch");
        let out = click(&ctx, &p, &mut color, swatch.center());
        let cells: Vec<egui::Rect> =
            (0..COLORS).filter_map(|i| named(&out, &format!("Skin {i}"))).collect();
        assert_eq!(cells.len(), usize::from(COLORS), "the grid, open");
        // Sixteen a row, eleven rows.
        let rows =
            |r: usize| cells[r * 16..r * 16 + 16].iter().all(|c| c.top() == cells[r * 16].top());
        assert!((0..11).all(rows));
        assert!(cells[16].top() > cells[0].top() && cells[1].left() > cells[0].left());
        let out = click(&ctx, &p, &mut color, cells[37].center());
        assert_eq!(color, 37, "picked");
        assert!(named(&out, "Skin 37").is_none(), "closed again");
        // The combo opens it too.
        let combo = named(&out, "Skin color").expect("the combo");
        let out = click(&ctx, &p, &mut color, combo.center());
        let cell = named(&out, "Skin 100").expect("open from the combo");
        click(&ctx, &p, &mut color, cell.center());
        assert_eq!(color, 100);
    }
}
