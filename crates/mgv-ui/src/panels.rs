//! The side panels: the resource browser, the node outliner, the
//! inspector, the animation timeline and the log.

use egui::Ui;
use mg_core::ResType;
use mg_mdl::{MeshExtra, NodeKind};
use mg_resman::ResKey;
use mgv_library::Kind;
use mgv_stage::{ActorId, PlayMode};

use crate::{Action, Level, Selection, Viewer};

/// Which resources the browser lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Filter {
    #[default]
    Models,
    Textures,
    Blueprints,
    Walkmeshes,
    Materials,
    /// `appearance.2da` rows, as creatures.
    Creatures,
    All,
}

impl Filter {
    const ALL: [Filter; 7] = [
        Filter::Models,
        Filter::Creatures,
        Filter::Textures,
        Filter::Blueprints,
        Filter::Walkmeshes,
        Filter::Materials,
        Filter::All,
    ];

    fn name(self) -> &'static str {
        match self {
            Filter::Models => "Models",
            Filter::Textures => "Textures",
            Filter::Blueprints => "Blueprints",
            Filter::Walkmeshes => "Walkmeshes",
            Filter::Materials => "Materials and TXI",
            Filter::Creatures => "Creatures (appearance)",
            Filter::All => "Everything shown",
        }
    }

    fn admits(self, t: ResType) -> bool {
        let k = Kind::of(t);
        match self {
            Filter::Models => k == Kind::Model,
            Filter::Textures => matches!(k, Kind::Texture | Kind::Plt),
            Filter::Blueprints => k == Kind::Blueprint,
            Filter::Walkmeshes => k == Kind::Walkmesh,
            Filter::Materials => matches!(k, Kind::Material | Kind::Txi),
            Filter::Creatures => false,
            Filter::All => !matches!(k, Kind::Other | Kind::Archive | Kind::TwoDa),
        }
    }
}

/// What a listing was made for: the library's generation, the query and the
/// filter.
type Listed = (u64, String, Filter);

/// The resource browser's state.
#[derive(Debug, Default)]
pub struct Browser {
    pub query: String,
    pub filter: Filter,
    /// The listing and what it was made for (generation, query, filter).
    listing: Option<(Listed, Vec<(ResKey, String)>)>,
    selected: Option<ResKey>,
}

impl Browser {
    /// Lists again on the next frame.
    pub fn invalidate(&mut self) {
        self.listing = None;
    }
}

pub(crate) fn browser(app: &mut Viewer, ui: &mut Ui) {
    let b = &mut app.browser;
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut b.query).hint_text("Search names").desired_width(140.0),
        );
        egui::ComboBox::from_id_salt("filter").selected_text(b.filter.name()).show_ui(ui, |ui| {
            for f in Filter::ALL {
                ui.selectable_value(&mut b.filter, f, f.name());
            }
        });
    });
    let want = (app.lib.generation(), b.query.to_ascii_lowercase(), b.filter);
    if b.listing.as_ref().is_none_or(|(k, _)| *k != want) {
        let rm = app.lib.resman();
        let layers = rm.layers();
        let items: Vec<(ResKey, String)> = rm
            .entries()
            .into_iter()
            .filter(|(k, _)| want.2.admits(k.restype))
            .filter(|(k, _)| {
                want.1.is_empty() || k.resref.to_lowercase().to_string().contains(&want.1)
            })
            .map(|(k, layer)| (k, layers[layer].label.clone()))
            .collect();
        b.listing = Some((want, items));
    }
    if b.filter == Filter::Creatures {
        creatures(app, ui);
        return;
    }
    let items = &b.listing.as_ref().expect("made above").1;
    ui.weak(format!("{} resources", items.len()));
    let row = ui.text_style_height(&egui::TextStyle::Body) + 2.0;
    let mut open = None;
    egui::ScrollArea::vertical().auto_shrink([false, false]).show_rows(
        ui,
        row,
        items.len(),
        |ui, range| {
            for (key, layer) in &items[range] {
                let selected = b.selected == Some(*key);
                let r = ui
                    .selectable_label(selected, key.to_string())
                    .on_hover_text(format!("from {layer}\ndouble-click to open"));
                if r.clicked() {
                    b.selected = Some(*key);
                }
                if r.double_clicked() {
                    open = Some(*key);
                }
            }
        },
    );
    if let Some(k) = open {
        app.actions.push(Action::OpenResource(k));
    }
}

/// `appearance.2da` rows; double-click shows the creature.
fn creatures(app: &mut Viewer, ui: &mut Ui) {
    let q = app.browser.query.to_ascii_lowercase();
    let Ok(t) = app.lib.game().table("appearance") else {
        ui.weak("No appearance.2da (no game folder?)");
        return;
    };
    let rows: Vec<(usize, String)> = (0..t.len())
        .filter_map(|r| {
            let label = t.get(r, "LABEL").filter(|l| *l != "****")?;
            t.get(r, "RACE").filter(|v| *v != "****")?;
            let text = format!("{r} {label}");
            (q.is_empty() || text.to_ascii_lowercase().contains(&q)).then_some((r, text))
        })
        .collect();
    ui.weak(format!("{} creatures", rows.len()));
    let row = ui.text_style_height(&egui::TextStyle::Body) + 2.0;
    let mut open = None;
    egui::ScrollArea::vertical().auto_shrink([false, false]).show_rows(
        ui,
        row,
        rows.len(),
        |ui, range| {
            for (r, text) in &rows[range] {
                if ui
                    .selectable_label(false, text)
                    .on_hover_text("double-click to show")
                    .double_clicked()
                {
                    open = Some(*r);
                }
            }
        },
    );
    if let Some(r) = open {
        app.actions.push(Action::OpenCreature(mgv_stage::subject::CreatureLook::new(r as u16)));
    }
}

/// The node tree of everything on the stage.
pub(crate) fn outliner(app: &mut Viewer, ui: &mut Ui) {
    let Some(g) = &app.gfx else { return };
    if g.stage.is_empty() {
        ui.weak("Nothing shown.");
        return;
    }
    let mut picked = None;
    egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
        for (i, a) in g.stage.actors().iter().enumerate() {
            let actor = ActorId(i);
            let model = &a.model.model;
            let label = if i == 0 { a.name.clone() } else { format!("+ {}", a.name) };
            egui::CollapsingHeader::new(label).id_salt(("actor", i)).default_open(i == 0).show(
                ui,
                |ui| {
                    // Roots of the model's tree, then children under them.
                    for (n, node) in
                        model.nodes.iter().enumerate().filter(|(_, n)| n.parent.is_none())
                    {
                        let _ = node;
                        tree(ui, model, actor, n, app.selection, &mut picked);
                    }
                },
            );
        }
    });
    if let Some(sel) = picked {
        app.select(Some(sel), false);
    }
}

fn tree(
    ui: &mut Ui,
    model: &mg_mdl::Model,
    actor: ActorId,
    n: usize,
    selection: Option<Selection>,
    picked: &mut Option<Selection>,
) {
    let node = &model.nodes[n];
    let sel = Selection { actor, node: n };
    let text = format!("{} ({})", node.name, node.kind.type_name());
    let selected = selection == Some(sel);
    if node.children.is_empty() {
        if ui.selectable_label(selected, text).clicked() {
            *picked = Some(sel);
        }
        return;
    }
    let id = ui.make_persistent_id(("node", actor.0, n));
    egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, n == 0)
        .show_header(ui, |ui| {
            if ui.selectable_label(selected, text).clicked() {
                *picked = Some(sel);
            }
        })
        .body(|ui| {
            for &c in &node.children {
                tree(ui, model, actor, c, selection, picked);
            }
        });
}

/// What the open thing is, and the selected node.
pub(crate) fn inspector(app: &mut Viewer, ui: &mut Ui) {
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        let Some(doc) = &app.doc else {
            ui.weak("Nothing open.");
            return;
        };
        egui::Grid::new("doc").num_columns(2).striped(true).show(ui, |ui| {
            ui.label("Name");
            ui.label(doc.name());
            ui.end_row();
            ui.label("Kind");
            ui.label(format!("{:?}", doc.opened.kind));
            ui.end_row();
            if let Some(p) = &doc.opened.path {
                ui.label("File");
                ui.label(p.display().to_string());
                ui.end_row();
            }
            if let Some(k) = doc.opened.key
                && let Some(o) = app.lib.origin(&k)
            {
                ui.label("From");
                ui.label(o);
                ui.end_row();
            }
            if doc.opened.kind == Kind::Model {
                ui.label("Format");
                ui.label(if mg_mdl::is_binary(&doc.opened.data) { "compiled" } else { "ASCII" });
                ui.end_row();
            }
        });
        if let Some(mut look) = doc.creature {
            ui.separator();
            let before = look;
            egui::Grid::new("look").num_columns(2).show(ui, |ui| {
                ui.label("Gender");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut look.gender, 0, "male");
                    ui.selectable_value(&mut look.gender, 1, "female");
                });
                ui.end_row();
                let drag = |ui: &mut Ui, label: &str, v: &mut u8, max: u8| {
                    ui.label(label);
                    ui.add(egui::DragValue::new(v).range(0..=max));
                    ui.end_row();
                };
                drag(ui, "Phenotype", &mut look.phenotype, 255);
                let [skin, hair, tattoo1, tattoo2] = &mut look.colors;
                drag(ui, "Skin color", skin, 175);
                drag(ui, "Hair color", hair, 175);
                drag(ui, "Tattoo 1", tattoo1, 175);
                drag(ui, "Tattoo 2", tattoo2, 175);
                ui.label("Head");
                ui.add(egui::DragValue::new(&mut look.head).range(1..=255));
                ui.end_row();
                ui.label("Body parts");
                ui.add(egui::DragValue::new(&mut look.body).range(1..=255));
                ui.end_row();
                ui.label("Wings");
                ui.add(egui::DragValue::new(&mut look.wings).range(0..=255));
                ui.end_row();
                ui.label("Tail");
                ui.add(egui::DragValue::new(&mut look.tail).range(0..=255));
                ui.end_row();
            });
            if look != before {
                app.actions.push(Action::OpenCreature(look));
            }
        }
        match doc.opened.kind {
            Kind::Material => material(ui, &doc.opened.data),
            Kind::Tileset => {
                if let Some(k) = tileset(ui, app.lib.game().language.codepage(), &doc.opened.data) {
                    app.actions.push(Action::OpenResource(k));
                }
            }
            _ => {}
        }
        let Some(g) = &app.gfx else { return };
        let Some(base) = doc.shown.as_ref().map(|s| s.base) else { return };
        let Some(a) = g.stage.actor(base) else { return };
        let m = &a.model.model;
        ui.separator();
        egui::Grid::new("model").num_columns(2).striped(true).show(ui, |ui| {
            ui.label("Model");
            ui.label(&m.name);
            ui.end_row();
            ui.label("Classification");
            ui.label(format!("{:?}", m.classification));
            ui.end_row();
            ui.label("Supermodel");
            ui.label(m.supermodel.as_deref().unwrap_or("none"));
            ui.end_row();
            ui.label("Nodes");
            ui.label(m.nodes.len().to_string());
            ui.end_row();
            let (faces, verts) = m
                .nodes
                .iter()
                .filter_map(|n| n.mesh())
                .fold((0, 0), |(f, v), mesh| (f + mesh.faces.len(), v + mesh.vertices.len()));
            ui.label("Faces / vertices");
            ui.label(format!("{faces} / {verts}"));
            ui.end_row();
            ui.label("Animations");
            ui.label(format!("{} ({} own)", a.animations.0.len(), m.animations.len()));
            ui.end_row();
        });
        let Some(sel) = app.selection else { return };
        let Some(sa) = g.stage.actor(sel.actor) else { return };
        let Some(node) = sa.model.model.nodes.get(sel.node) else { return };
        ui.separator();
        ui.heading(&node.name);
        egui::Grid::new("node").num_columns(2).striped(true).show(ui, |ui| {
            let row = |ui: &mut Ui, k: &str, v: String| {
                ui.label(k);
                ui.label(v);
                ui.end_row();
            };
            row(ui, "Type", node.kind.type_name().into());
            row(
                ui,
                "Parent",
                node.parent.map_or("none".into(), |p| sa.model.model.nodes[p].name.clone()),
            );
            row(ui, "Position", fmt3(node.position));
            let aa = mgv_mdl::write::axis_angle(node.orientation);
            row(
                ui,
                "Orientation",
                format!("{} rotated {:.1}°", fmt3([aa[0], aa[1], aa[2]]), aa[3].to_degrees()),
            );
            if node.scale != 1.0 {
                row(ui, "Scale", format!("{}", node.scale));
            }
            for c in &node.controllers {
                let v: Vec<String> = c.row(0).iter().map(|x| format!("{x:.4}")).collect();
                row(ui, &c.name, v.join(" "));
            }
            match &node.kind {
                NodeKind::Mesh(mesh) => {
                    row(
                        ui,
                        "Faces / vertices",
                        format!("{} / {}", mesh.faces.len(), mesh.vertices.len()),
                    );
                    for (i, t) in mesh.textures.iter().enumerate() {
                        if let Some(t) = t {
                            let found = mg_render::Assets::texture(app.lib.resman(), t).is_some();
                            let label =
                                if i == 0 { "Bitmap".to_string() } else { format!("Texture {i}") };
                            row(
                                ui,
                                &label,
                                format!("{t}{}", if found { "" } else { " (not found)" }),
                            );
                        }
                    }
                    if let Some(mtr) = &mesh.material {
                        row(ui, "Material", mtr.clone());
                    }
                    row(ui, "Render / shadow", format!("{} / {}", mesh.render, mesh.shadow));
                    if mesh.transparency_hint != 0 {
                        row(ui, "Transparency hint", mesh.transparency_hint.to_string());
                    }
                    match &mesh.extra {
                        MeshExtra::Skin(s) => row(ui, "Bones", s.bones.len().to_string()),
                        MeshExtra::Dangly(d) => row(
                            ui,
                            "Dangly",
                            format!(
                                "displacement {} tightness {} period {}",
                                d.displacement, d.tightness, d.period
                            ),
                        ),
                        MeshExtra::Anim(am) => {
                            row(ui, "Sample period", am.sample_period.to_string())
                        }
                        MeshExtra::Aabb(t) => row(ui, "AABB entries", t.len().to_string()),
                        MeshExtra::None => {}
                    }
                }
                NodeKind::Emitter(e) => {
                    row(
                        ui,
                        "Update / render / blend",
                        format!("{} / {} / {}", e.update, e.render, e.blend),
                    );
                    row(ui, "Texture", e.texture.clone().unwrap_or_else(|| "none".into()));
                    if let Some(c) = &e.chunk {
                        row(ui, "Chunk", c.clone());
                    }
                    row(ui, "Grid", format!("{} × {}", e.xgrid, e.ygrid));
                    let flags: Vec<&str> = mg_mdl::EMITTER_FLAGS
                        .iter()
                        .filter(|(_, b)| e.flags & b != 0)
                        .map(|(n, _)| *n)
                        .collect();
                    row(ui, "Flags", flags.join(", "));
                }
                NodeKind::Light(l) => {
                    row(ui, "Priority", l.priority.to_string());
                    row(ui, "Ambient only", l.ambient_only.to_string());
                    row(ui, "Shadow", l.shadow.to_string());
                }
                NodeKind::Reference(r) => {
                    row(ui, "Model", r.model.clone().unwrap_or_else(|| "none".into()));
                }
                NodeKind::Dummy | NodeKind::Camera => {}
            }
        });
    });
}

/// An MTR's slots, parameters and flags.
fn material(ui: &mut Ui, data: &[u8]) {
    const SLOTS: [&str; 6] =
        ["diffuse", "normal", "specular", "roughness", "height", "self-illumination"];
    let m = mg_image::mtr::Mtr::parse(data);
    ui.separator();
    egui::Grid::new("mtr").num_columns(2).striped(true).show(ui, |ui| {
        for (i, t) in m.textures.iter().enumerate() {
            let Some(t) = t else { continue };
            ui.label(format!(
                "Texture {i}{}",
                SLOTS.get(i).map_or(String::new(), |s| format!(" ({s})"))
            ));
            ui.label(t);
            ui.end_row();
        }
        ui.label("Render hint");
        ui.label(format!("{:?}", m.renderhint));
        ui.end_row();
        for (name, p) in &m.params {
            ui.label(name);
            ui.label(match p {
                mg_image::mtr::Param::Float(v) => {
                    v.iter().map(|x| format!("{x}")).collect::<Vec<_>>().join(" ")
                }
                mg_image::mtr::Param::Int(i) => i.to_string(),
            });
            ui.end_row();
        }
        if let (Some(vs), Some(fs)) = (&m.shader_vs, &m.shader_fs) {
            ui.label("Custom shaders");
            ui.label(format!("{vs} / {fs} (not run: the viewer draws with its own)"));
            ui.end_row();
        }
        for (flag, on) in [("Transparent", m.transparency), ("Two-sided", m.twosided)] {
            if on {
                ui.label(flag);
                ui.label("yes");
                ui.end_row();
            }
        }
    });
}

/// A tileset's tiles; the one clicked is returned (its model).
fn tileset(ui: &mut Ui, codepage: mg_core::Codepage, data: &[u8]) -> Option<ResKey> {
    let set = match mg_set::Tileset::parse(data, codepage) {
        Ok(s) => s,
        Err(e) => {
            ui.colored_label(ui.visuals().error_fg_color, e.to_string());
            return None;
        }
    };
    ui.separator();
    ui.label(format!(
        "{} tiles, {} groups, {} terrains, {} crossers",
        set.tiles.len(),
        set.groups.len(),
        set.terrains.len(),
        set.crossers.len()
    ));
    for w in set.warnings.iter().take(5) {
        ui.weak(w);
    }
    let mut picked = None;
    for (i, t) in set.tiles.iter().enumerate() {
        let corners: Vec<&str> = t.corners.iter().map(|(c, _)| c.as_str()).collect();
        let r = ui
            .selectable_label(false, format!("{i}: {}", t.model.to_ascii_lowercase()))
            .on_hover_text(format!(
                "corners {}\n{} doors{}\nclick to show",
                corners.join(", "),
                t.doors.len(),
                t.walkmesh.as_deref().map_or(String::new(), |w| format!(", walkmesh {w}"))
            ));
        if r.clicked() {
            picked = ResKey::parse(&t.model, ResType::MDL);
        }
    }
    picked
}

fn fmt3(v: [f32; 3]) -> String {
    format!("{:.4} {:.4} {:.4}", v[0], v[1], v[2])
}

/// Playing the base model's animations.
pub(crate) fn timeline(app: &mut Viewer, ui: &mut Ui) {
    let Some(g) = &mut app.gfx else { return };
    let Some(base) = app.doc.as_ref().and_then(|d| d.shown.as_ref()).map(|s| s.base) else {
        ui.weak("Nothing shown.");
        return;
    };
    let Some(a) = g.stage.actor_mut(base) else { return };
    let names: Vec<String> = a.animations.names().map(str::to_string).collect();
    let current = a.player.animation.clone();
    let length = current.as_deref().and_then(|n| a.animations.find(n)).map_or(0.0, |x| x.length);
    let events: Vec<(f32, String)> = current
        .as_deref()
        .and_then(|n| a.animations.find(n))
        .map(|x| x.events.clone())
        .unwrap_or_default();
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt("anim")
            .selected_text(current.clone().unwrap_or_else(|| "(rest pose)".into()))
            .height(400.0)
            .show_ui(ui, |ui| {
                if ui.selectable_label(current.is_none(), "(rest pose)").clicked() {
                    a.player.play(None, PlayMode::Loop);
                }
                for n in &names {
                    if ui.selectable_label(current.as_deref() == Some(n), n).clicked() {
                        let mode = a.player.mode;
                        a.player.play(Some(n), mode);
                    }
                }
            });
        if ui.button(if a.player.playing { "Pause" } else { "Play" }).clicked() {
            a.player.playing = !a.player.playing;
        }
        if ui.button("Restart").on_hover_text("From the start, emitters too").clicked() {
            a.player.time = 0.0;
            a.restart_particles();
        }
        ui.selectable_value(&mut a.player.mode, PlayMode::Loop, "Loop");
        ui.selectable_value(&mut a.player.mode, PlayMode::Once, "Once");
        ui.add(egui::Slider::new(&mut a.player.speed, 0.05..=4.0).logarithmic(true).text("speed"));
    });
    if length > 0.0 {
        let mut t = if a.player.mode == PlayMode::Loop {
            a.player.time.rem_euclid(length)
        } else {
            a.player.time.min(length)
        };
        let r = ui.add(egui::Slider::new(&mut t, 0.0..=length).text("s").fixed_decimals(2));
        if r.changed() {
            a.player.time = t;
        }
        // Events under the slider.
        let rect = r.rect;
        let width = ui.spacing().slider_width;
        let painter = ui.painter();
        for (et, name) in &events {
            let x = rect.left() + width * (et / length).clamp(0.0, 1.0);
            painter.line_segment(
                [egui::pos2(x, rect.bottom()), egui::pos2(x, rect.bottom() + 6.0)],
                egui::Stroke::new(2.0, egui::Color32::from_rgb(0xE8, 0xA3, 0x3D)),
            );
            let _ = name;
        }
        if !events.is_empty() {
            let list: Vec<String> = events.iter().map(|(t, n)| format!("{n} {t:.2}s")).collect();
            ui.weak(format!("Events: {}", list.join(", ")));
        }
    }
}

pub(crate) fn log(app: &mut Viewer, ui: &mut Ui) {
    egui::ScrollArea::vertical().auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
        for l in &app.log {
            let color = match l.level {
                Level::Info => ui.visuals().text_color(),
                Level::Warning => ui.visuals().warn_fg_color,
                Level::Error => ui.visuals().error_fg_color,
            };
            ui.colored_label(color, &l.text);
        }
    });
}
