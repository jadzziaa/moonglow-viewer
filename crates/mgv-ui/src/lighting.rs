//! The Lighting panel: a neutral studio light, an `environment.2da` preset
//! by day or night, or an area's settings typed in.

use egui::Ui;
use mgv_stage::Lighting;
use mgv_stage::lighting::{AreaSettings, Environment};

use crate::Viewer;

/// Which light.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RigKind {
    #[default]
    Studio,
    /// An `environment.2da` row.
    Preset(usize),
    Custom,
}

/// The panel's state.
#[derive(Debug, Default)]
pub struct Rig {
    pub kind: RigKind,
    pub night: bool,
    pub fog: bool,
    pub custom: AreaSettings,
    presets: Option<(u64, Vec<Environment>)>,
    /// What the stage's lighting was made from (to know when to remake it).
    applied: Option<(RigKind, bool, bool, AreaSettings, u64)>,
}

impl Rig {
    /// The lighting for the current choice.
    fn lighting(&self, app_lib: &mgv_library::Library) -> Lighting {
        let presets = self.presets.as_ref().map(|(_, p)| p.as_slice()).unwrap_or(&[]);
        let settings = match self.kind {
            RigKind::Studio => return Lighting::studio(),
            RigKind::Preset(row) => match presets.iter().find(|e| e.row == row) {
                Some(e) => {
                    if self.night {
                        e.night
                    } else {
                        e.day
                    }
                }
                None => return Lighting::studio(),
            },
            RigKind::Custom => self.custom,
        };
        Lighting::area(&AreaSettings { fog: self.fog, ..settings }, app_lib)
    }
}

/// Keeps the stage's lighting in step with the panel (each frame).
pub(crate) fn update(app: &mut Viewer) {
    let generation = app.lib.generation();
    let rig = &mut app.rig;
    if rig.presets.as_ref().is_none_or(|(g, _)| *g != generation) {
        rig.presets = Some((generation, mgv_stage::lighting::environments(&app.lib)));
    }
    let key = (rig.kind, rig.night, rig.fog, rig.custom, generation);
    if rig.applied == Some(key) {
        return;
    }
    let lighting = rig.lighting(&app.lib);
    if let Some(g) = &mut app.gfx {
        // Keep a studio background the user picked.
        let background = (rig.kind == RigKind::Studio).then_some(g.stage.lighting.background);
        g.stage.lighting = lighting;
        if let Some(bg) = background {
            g.stage.lighting.background = bg;
        }
    }
    rig.applied = Some(key);
}

fn rgb(ui: &mut Ui, label: &str, c: &mut [u8; 3]) {
    ui.label(label);
    let mut v = egui::Color32::from_rgb(c[0], c[1], c[2]);
    if ui.color_edit_button_srgba(&mut v).changed() {
        *c = [v.r(), v.g(), v.b()];
    }
    ui.end_row();
}

pub(crate) fn ui(app: &mut Viewer, ui: &mut Ui) {
    let rig = &mut app.rig;
    let presets: Vec<(usize, String)> = rig
        .presets
        .as_ref()
        .map(|(_, p)| p.iter().map(|e| (e.row, e.label.clone())).collect())
        .unwrap_or_default();
    let name = |k: RigKind| match k {
        RigKind::Studio => "Studio".to_string(),
        RigKind::Custom => "Custom area".to_string(),
        RigKind::Preset(r) => presets
            .iter()
            .find(|(row, _)| *row == r)
            .map_or_else(|| format!("Preset {r}"), |(_, l)| l.clone()),
    };
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        egui::ComboBox::from_id_salt("rig").selected_text(name(rig.kind)).height(400.0).show_ui(
            ui,
            |ui| {
                ui.selectable_value(&mut rig.kind, RigKind::Studio, "Studio")
                    .on_hover_text("A neutral daylight for looking at models");
                ui.separator();
                for (row, label) in &presets {
                    ui.selectable_value(&mut rig.kind, RigKind::Preset(*row), label)
                        .on_hover_text("environment.2da: the area wizard's lighting");
                }
                ui.separator();
                ui.selectable_value(&mut rig.kind, RigKind::Custom, "Custom area");
            },
        );
        if rig.kind != RigKind::Studio {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut rig.night, false, "Day");
                ui.selectable_value(&mut rig.night, true, "Night");
                ui.checkbox(&mut rig.fog, "Fog");
            });
        }
        if rig.kind == RigKind::Custom {
            let c = &mut rig.custom;
            egui::Grid::new("custom-light").num_columns(2).show(ui, |ui| {
                rgb(ui, "Ambient", &mut c.ambient);
                rgb(ui, "Diffuse", &mut c.diffuse);
                rgb(ui, "Fog color", &mut c.fog_color);
                ui.label("Fog amount");
                ui.add(egui::Slider::new(&mut c.fog_amount, 0..=15));
                ui.end_row();
                ui.label("Fog clip distance");
                ui.add(egui::Slider::new(&mut c.fog_clip, 5.0..=200.0));
                ui.end_row();
                ui.label("Main light 1");
                ui.add(egui::DragValue::new(&mut c.main_lights[0]).range(0..=31)).on_hover_text(
                    "lightcolor.2da row for tiles' main light 1 (0: the model's own)",
                );
                ui.end_row();
                ui.label("Main light 2");
                ui.add(egui::DragValue::new(&mut c.main_lights[1]).range(0..=31));
                ui.end_row();
            });
        }
        if let Some(g) = &mut app.gfx {
            ui.separator();
            ui.checkbox(&mut g.stage.model_lights, "The model's own lights");
            let l = &g.stage.lighting;
            let f = |v: glam::Vec3| format!("{:.2} {:.2} {:.2}", v.x, v.y, v.z);
            ui.weak(format!(
                "Ambient {} · diffuse {} (linear)",
                f(l.area.ambient),
                f(l.area.diffuse)
            ));
        }
    });
    // Starting a custom area from the preset in use.
    if rig.kind == RigKind::Custom
        && rig.custom == AreaSettings::default()
        && let Some(RigKind::Preset(r)) = rig.applied.map(|a| a.0)
        && let Some((_, ps)) = &rig.presets
        && let Some(e) = ps.iter().find(|e| e.row == r)
    {
        rig.custom = if rig.night { e.night } else { e.day };
    }
}
