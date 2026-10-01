//! The Effects panel: `visualeffects.2da` rows to apply to what is shown,
//! and the ones applied, each removable (its cessation plays).

use egui::Ui;
use mgv_stage::vfx::{Applied, VisualEffect};

use crate::{Action, Viewer};

/// The panel's state.
#[derive(Debug, Default)]
pub struct Effects {
    pub query: String,
    /// The table and the library generation it was read at.
    table: Option<(u64, Vec<VisualEffect>)>,
    /// Effects on the stage now.
    pub applied: Vec<Applied>,
    /// The target's size for ground models; `None`: the target's own.
    pub size: Option<u32>,
}

pub(crate) fn ui(app: &mut Viewer, ui: &mut Ui) {
    let generation = app.lib.generation();
    if app.effects.table.as_ref().is_none_or(|(g, _)| *g != generation) {
        app.effects.table = Some((generation, mgv_stage::vfx::table(&app.lib)));
    }
    let own_size = app.doc.as_ref().map_or(3, |d| d.size(&app.lib));
    let e = &mut app.effects;
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut e.query)
                .hint_text("Search effects")
                .desired_width(180.0),
        );
        let size_name = |s: u32| match s {
            0..=2 => "small",
            3 => "medium",
            4 => "large",
            _ => "huge",
        };
        let current = e
            .size
            .map_or_else(|| format!("target's ({})", size_name(own_size)), |s| size_name(s).into());
        egui::ComboBox::from_id_salt("vfx-size")
            .selected_text(current)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut e.size, None, "target's own");
                for (s, n) in [(1, "small"), (3, "medium"), (4, "large"), (5, "huge")] {
                    ui.selectable_value(&mut e.size, Some(s), n);
                }
            })
            .response
            .on_hover_text("Ground effects have a model per creature size");
        if ui.add_enabled(!e.applied.is_empty(), egui::Button::new("Clear")).clicked() {
            app.actions.push(Action::ClearEffects);
        }
    });
    let mut apply = None;
    let mut remove = None;
    ui.columns(2, |cols| {
        let q = e.query.to_ascii_lowercase();
        let list: Vec<&VisualEffect> = e
            .table
            .as_ref()
            .map(|(_, t)| {
                t.iter()
                    .filter(|v| {
                        q.is_empty()
                            || v.label.to_ascii_lowercase().contains(&q)
                            || v.row.to_string() == q
                    })
                    .collect()
            })
            .unwrap_or_default();
        let row = cols[0].text_style_height(&egui::TextStyle::Body) + 4.0;
        egui::ScrollArea::vertical().id_salt("vfx-list").auto_shrink([false, false]).show_rows(
            &mut cols[0],
            row,
            list.len(),
            |ui, range| {
                for v in &list[range] {
                    let models: Vec<&str> = [v.head.as_deref(), v.impact.as_deref()]
                        .into_iter()
                        .chain(v.root.iter().map(Option::as_deref))
                        .flatten()
                        .collect();
                    let r = ui
                        .selectable_label(false, format!("{} {} ({})", v.row, v.label, v.kind))
                        .on_hover_text(format!("{}\ndouble-click to apply", models.join(", ")));
                    if r.double_clicked() {
                        apply = Some(v.row);
                    }
                }
            },
        );
        let ui = &mut cols[1];
        if e.applied.is_empty() {
            ui.weak("Double-click an effect to apply it to what is shown.");
        }
        for (i, a) in e.applied.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!("{} ({})", a.effect.label, a.actors.len()));
                if ui.small_button("Remove").on_hover_text("Plays its cessation").clicked() {
                    remove = Some(i);
                }
            });
        }
    });
    if let Some(r) = apply {
        app.actions.push(Action::ApplyEffect(r));
    }
    if let Some(i) = remove {
        app.actions.push(Action::RemoveEffect(i));
    }
}
