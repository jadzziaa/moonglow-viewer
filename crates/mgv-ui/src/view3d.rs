//! The 3D view: the stage drawn into a texture the window shows, turned by
//! dragging (left: orbit; right or middle: pan; wheel: zoom; double-click:
//! frame), with the ground grid, axes, overlays and the selected node drawn
//! over it: in 3D, tested against the scene's depths (faint where hidden),
//! the skeleton and the selection over everything, and names and node
//! markers painted on top.

use glam::{Mat4, Vec3, Vec4};
use mgv_stage::camera::View;
use mgv_stage::overlay::Overlay;
use mgv_stage::{Stage, Viewport};

use crate::{Action, Selection, Viewer};

/// The window's GPU and what draws with it.
pub struct Gfx {
    pub render_state: egui_wgpu::RenderState,
    pub stage: Stage,
    pub viewport: Viewport,
    texture: Option<(egui::TextureId, (u32, u32))>,
}

impl std::fmt::Debug for Gfx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Gfx").field("stage", &self.stage).finish_non_exhaustive()
    }
}

impl Gfx {
    pub fn new(render_state: egui_wgpu::RenderState) -> Gfx {
        let gpu = mg_render::Gpu::new(render_state.device.clone(), render_state.queue.clone());
        let viewport = Viewport::new(&gpu);
        Gfx { render_state, stage: Stage::new(gpu), viewport, texture: None }
    }
}

/// Projects world points into a screen rectangle.
pub(crate) struct Projector {
    view_proj: Mat4,
    rect: egui::Rect,
}

impl Projector {
    pub(crate) fn new(camera: &mg_render::Camera, rect: egui::Rect) -> Projector {
        let aspect = rect.width() / rect.height().max(1.0);
        Projector { view_proj: camera.projection(aspect) * camera.view(), rect }
    }

    fn clip(&self, p: Vec3) -> Vec4 {
        self.view_proj * p.extend(1.0)
    }

    fn to_screen(&self, c: Vec4) -> egui::Pos2 {
        let n = c.truncate() / c.w;
        egui::pos2(
            self.rect.left() + (n.x + 1.0) * 0.5 * self.rect.width(),
            self.rect.top() + (1.0 - n.y) * 0.5 * self.rect.height(),
        )
    }

    /// A point on screen, if in front of the camera.
    pub(crate) fn point(&self, p: Vec3) -> Option<egui::Pos2> {
        let c = self.clip(p);
        (c.w > 1e-4).then(|| self.to_screen(c))
    }
}

/// The 3D view tab.
pub fn ui(app: &mut Viewer, ui: &mut egui::Ui) {
    toolbar(app, ui);
    let Some(g) = &mut app.gfx else {
        ui.centered_and_justified(|ui| ui.label("No GPU: the 3D view is off."));
        return;
    };
    let size = ui.available_size().max(egui::vec2(32.0, 32.0));
    let ppp = ui.ctx().pixels_per_point();
    let px = ((size.x * ppp).round() as u32, (size.y * ppp).round() as u32);
    let px = (px.0.clamp(16, 8192), px.1.clamp(16, 8192));

    let camera = app.camera.camera();
    let scene = g.stage.scene(camera.view());
    let gpu = g.stage.gpu().clone();
    g.viewport.draw(&gpu, app.lib.resman(), &scene, &camera, px);
    let mut overlay = Overlay::default();
    if app.settings.show_grid {
        grid(&mut overlay, app.camera.distance);
    }
    if app.settings.show_axes {
        axes(&mut overlay, app.camera.distance * 0.15);
    }
    let walkmesh_doc =
        app.doc.as_ref().is_some_and(|d| d.opened.kind == mgv_library::Kind::Walkmesh);
    let o = app.settings.overlays;
    if o.walkmesh || walkmesh_doc {
        let materials = mgv_stage::posed::surface_materials(&app.lib);
        let mut meshes = g.stage.posed_walkmeshes();
        meshes.extend(g.stage.posed(true).into_iter().filter(|m| m.walkmesh));
        walkmeshes(&mut overlay, &meshes, &materials);
    }
    if o.wireframe || o.normals {
        let meshes = g.stage.posed(false);
        if o.wireframe {
            wireframe(&mut overlay, &meshes);
        }
        if o.normals {
            normals(&mut overlay, &meshes, app.camera.distance * 0.02);
        }
    }
    if o.skeleton {
        skeleton(&mut overlay, &g.stage);
    }
    if let Some(sel) = app.selection {
        selected_box(&mut overlay, &g.stage, sel);
    }
    g.viewport.draw_overlay(&gpu, &overlay, &camera);
    let targets = g.viewport.current().expect("drawn above");
    let id = match g.texture {
        Some((id, s)) if s == px => id,
        Some((id, _)) => {
            g.render_state.renderer.write().update_egui_texture_from_wgpu_texture(
                &gpu.device,
                &targets.color_view,
                wgpu::FilterMode::Linear,
                id,
            );
            g.texture = Some((id, px));
            id
        }
        None => {
            let id = g.render_state.renderer.write().register_native_texture(
                &gpu.device,
                &targets.color_view,
                wgpu::FilterMode::Linear,
            );
            g.texture = Some((id, px));
            id
        }
    };
    let response = ui.add(
        egui::Image::new(egui::load::SizedTexture::new(id, size))
            .sense(egui::Sense::click_and_drag()),
    );
    let rect = response.rect;
    let proj = Projector::new(&camera, rect);
    let painter = ui.painter_at(rect);
    if o.skeleton {
        skeleton_markers(&painter, &proj, &g.stage);
    }
    if let Some(sel) = app.selection {
        selected(&painter, &proj, &g.stage, sel);
    }

    // Camera.
    if response.dragged_by(egui::PointerButton::Primary) {
        let d = response.drag_delta();
        app.camera.orbit(d.x, d.y);
    }
    if response.dragged_by(egui::PointerButton::Secondary)
        || response.dragged_by(egui::PointerButton::Middle)
    {
        let d = response.drag_delta();
        app.camera.pan(d.x, d.y);
    }
    if response.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            app.camera.zoom(scroll);
        }
        if ui.input(|i| i.key_pressed(egui::Key::F)) {
            app.actions.push(Action::Frame);
        }
    }
    if response.double_clicked() {
        app.actions.push(Action::Frame);
    } else if response.clicked()
        && let Some(at) = response.interact_pointer_pos()
    {
        let picked = pick(&g.stage, &proj, &camera, rect, at);
        app.select(picked, false);
    }
}

fn toolbar(app: &mut Viewer, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        if ui.button("Frame").on_hover_text("F, or double-click").clicked() {
            app.actions.push(Action::Frame);
        }
        egui::ComboBox::from_id_salt("view").selected_text("View").show_ui(ui, |ui| {
            for (v, name) in [
                (View::ThreeQuarter, "Three-quarter"),
                (View::Front, "Front"),
                (View::Back, "Back"),
                (View::Left, "Left"),
                (View::Right, "Right"),
                (View::Top, "Top"),
                (View::Bottom, "Bottom"),
            ] {
                if ui.selectable_label(false, name).clicked() {
                    app.actions.push(Action::SetView(v));
                }
            }
        });
        ui.checkbox(&mut app.settings.show_grid, "Grid");
        ui.checkbox(&mut app.settings.show_axes, "Axes");
        ui.menu_button("Overlays", |ui| {
            let o = &mut app.settings.overlays;
            ui.checkbox(&mut o.walkmesh, "Walkmesh")
                .on_hover_text("Faces colored by surface material: green walkable, red not");
            ui.checkbox(&mut o.wireframe, "Wireframe");
            ui.checkbox(&mut o.normals, "Normals");
            ui.checkbox(&mut o.skeleton, "Nodes")
                .on_hover_text("The node tree: bones, hooks, emitters and lights");
        });
        if let Some(g) = &mut app.gfx {
            ui.checkbox(&mut g.stage.model_lights, "Model lights")
                .on_hover_text("Light the scene with the model's own light nodes");
            let mut bg = g.stage.lighting.background;
            if ui.color_edit_button_rgb(&mut bg).on_hover_text("Background").changed() {
                g.stage.lighting.background = bg;
            }
        }
    });
}

/// A colour for the overlay (gamma space, alpha).
fn rgba(c: egui::Color32) -> [f32; 4] {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    [r, g, b, a].map(|v| f32::from(v) / 255.0)
}

/// The ground grid: 1 m cells over a 10 m tile, finer or coarser with
/// distance.
fn grid(overlay: &mut Overlay, distance: f32) {
    let step: f32 = if distance > 60.0 {
        10.0
    } else if distance < 2.0 {
        0.1
    } else {
        1.0
    };
    let half = (step * 10.0).max(5.0);
    let n = (half / step).round() as i32;
    let weak = rgba(egui::Color32::from_white_alpha(40));
    let strong = rgba(egui::Color32::from_white_alpha(80));
    for i in -n..=n {
        let v = i as f32 * step;
        let color = if i == 0 { strong } else { weak };
        overlay.line(Vec3::new(v, -half, 0.0), Vec3::new(v, half, 0.0), color);
        overlay.line(Vec3::new(-half, v, 0.0), Vec3::new(half, v, 0.0), color);
    }
}

/// X (red), Y (green: creatures face +Y, most placeables −Y) and Z (blue,
/// up).
fn axes(overlay: &mut Overlay, len: f32) {
    for (dir, color) in [
        (Vec3::X, egui::Color32::from_rgb(0xE5, 0x48, 0x4D)),
        (Vec3::Y, egui::Color32::from_rgb(0x4C, 0xC3, 0x5A)),
        (Vec3::Z, egui::Color32::from_rgb(0x4A, 0x8E, 0xE8)),
    ] {
        overlay.line(Vec3::ZERO, dir * len, rgba(color));
    }
}

/// A node's world transform as of the last step.
pub(crate) fn node_world(stage: &Stage, sel: Selection) -> Option<Mat4> {
    let a = stage.actor(sel.actor)?;
    Some(a.world() * *a.pose().get(sel.node)?)
}

const SELECTED: egui::Color32 = egui::Color32::from_rgb(0xFF, 0xB0, 0x2E);

/// The selected mesh's box, over everything.
fn selected_box(overlay: &mut Overlay, stage: &Stage, sel: Selection) {
    let Some(a) = stage.actor(sel.actor) else { return };
    let Some(world) = node_world(stage, sel) else { return };
    let Some(m) = a.model.meshes.iter().find(|m| m.node == sel.node) else { return };
    let corner = |i: usize| {
        world.transform_point3(Vec3::new(
            if i & 1 == 0 { m.min.x } else { m.max.x },
            if i & 2 == 0 { m.min.y } else { m.max.y },
            if i & 4 == 0 { m.min.z } else { m.max.z },
        ))
    };
    let edges = [
        (0, 1),
        (2, 3),
        (4, 5),
        (6, 7),
        (0, 2),
        (1, 3),
        (4, 6),
        (5, 7),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ];
    for (i, j) in edges {
        overlay.xray_line(corner(i), corner(j), rgba(SELECTED));
    }
}

/// The selected node's name, and a cross where it is if it has no mesh.
fn selected(painter: &egui::Painter, proj: &Projector, stage: &Stage, sel: Selection) {
    let Some(a) = stage.actor(sel.actor) else { return };
    let Some(world) = node_world(stage, sel) else { return };
    let color = SELECTED;
    let stroke = egui::Stroke::new(1.5, color);
    if !a.model.meshes.iter().any(|m| m.node == sel.node)
        && let Some(p) = proj.point(world.w_axis.truncate())
    {
        let r = 6.0;
        painter.line_segment([p - egui::vec2(r, 0.0), p + egui::vec2(r, 0.0)], stroke);
        painter.line_segment([p - egui::vec2(0.0, r), p + egui::vec2(0.0, r)], stroke);
    }
    let name = &a.model.model.nodes[sel.node].name;
    if let Some(p) = proj.point(world.w_axis.truncate()) {
        painter.text(
            p + egui::vec2(8.0, -8.0),
            egui::Align2::LEFT_BOTTOM,
            name,
            egui::FontId::proportional(13.0),
            color,
        );
    }
}

/// What a click picks: the nearest drawn face under the pointer (its
/// mesh's node), else the node nearest the click on screen within 16
/// points (dummies, hooks, emitters, lights).
fn pick(
    stage: &Stage,
    proj: &Projector,
    camera: &mg_render::Camera,
    rect: egui::Rect,
    at: egui::Pos2,
) -> Option<Selection> {
    // The pointer's ray: through the near and far planes.
    let ndc = glam::Vec2::new(
        (at.x - rect.left()) / rect.width() * 2.0 - 1.0,
        1.0 - (at.y - rect.top()) / rect.height() * 2.0,
    );
    let aspect = rect.width() / rect.height().max(1.0);
    let inv = (camera.projection(aspect) * camera.view()).inverse();
    let near = inv.project_point3(ndc.extend(0.0));
    let far = inv.project_point3(ndc.extend(1.0));
    let meshes = stage.posed(false);
    if let Some((i, _)) = mgv_stage::posed::ray_hit(&meshes, near, far - near)
        && let mgv_stage::posed::Owner::Actor(actor) = meshes[i].owner
    {
        return Some(Selection { actor, node: meshes[i].node });
    }
    let mut best: Option<(f32, Selection)> = None;
    for (i, a) in stage.actors().iter().enumerate().filter(|(_, a)| a.visible) {
        let actor = mgv_stage::ActorId(i);
        for (node, n) in a.model.model.nodes.iter().enumerate() {
            if n.parent.is_none() || n.mesh().is_some() {
                continue;
            }
            let Some(pose) = a.pose().get(node) else { continue };
            let Some(p) = proj.point((a.world() * *pose).w_axis.truncate()) else { continue };
            let d = p.distance(at);
            if d < 16.0 && best.is_none_or(|(b, _)| d < b) {
                best = Some((d, Selection { actor, node }));
            }
        }
    }
    best.map(|(_, s)| s)
}

/// A colour per surface material: greens for what creatures walk on, reds
/// for what they do not, each material its own shade.
fn surface_color(material: u32, materials: &[(String, bool)]) -> egui::Color32 {
    let walk = materials.get(material as usize).is_none_or(|(_, w)| *w);
    let shade = (material.wrapping_mul(53) % 90) as u8;
    if walk {
        egui::Color32::from_rgba_unmultiplied(40 + shade / 2, 150 + shade, 70, 90)
    } else {
        egui::Color32::from_rgba_unmultiplied(170 + shade / 2, 40 + shade / 3, 50, 90)
    }
}

/// Walkmesh faces, filled by surface material, and their edges.
fn walkmeshes(
    overlay: &mut Overlay,
    meshes: &[mgv_stage::posed::Posed],
    materials: &[(String, bool)],
) {
    for m in meshes {
        for (f, mat) in m.faces.iter().zip(&m.materials) {
            let [Some(a), Some(b), Some(c)] = f.map(|i| m.positions.get(i as usize).copied())
            else {
                continue;
            };
            let fill = surface_color(*mat, materials);
            overlay.triangle(a, b, c, rgba(fill));
            let edge = rgba(fill.gamma_multiply(2.0).to_opaque().gamma_multiply(0.8));
            for (p, q) in [(a, b), (b, c), (c, a)] {
                overlay.line(p, q, edge);
            }
        }
    }
}

/// The most edges drawn as a wireframe.
const MAX_EDGES: usize = 1_000_000;

fn wireframe(overlay: &mut Overlay, meshes: &[mgv_stage::posed::Posed]) {
    let color = rgba(egui::Color32::from_white_alpha(90));
    let mut drawn = 0;
    for m in meshes {
        let mut edges = std::collections::HashSet::new();
        for f in &m.faces {
            for (a, b) in [(f[0], f[1]), (f[1], f[2]), (f[2], f[0])] {
                edges.insert((a.min(b), a.max(b)));
            }
        }
        for (a, b) in edges {
            if drawn >= MAX_EDGES {
                return;
            }
            let (Some(p), Some(q)) = (m.positions.get(a as usize), m.positions.get(b as usize))
            else {
                continue;
            };
            overlay.line(*p, *q, color);
            drawn += 1;
        }
    }
}

fn normals(overlay: &mut Overlay, meshes: &[mgv_stage::posed::Posed], len: f32) {
    let color = rgba(egui::Color32::from_rgb(0x5B, 0xC8, 0xF0));
    for m in meshes {
        for (p, n) in m.positions.iter().zip(&m.normals) {
            overlay.line(*p, *p + *n * len, color);
        }
    }
}

/// Every node joined to its parent, over everything.
fn skeleton(overlay: &mut Overlay, stage: &Stage) {
    let color = rgba(egui::Color32::from_rgba_unmultiplied(255, 214, 102, 170));
    for a in stage.actors().iter().filter(|a| a.visible) {
        let at = |i: usize| a.pose().get(i).map(|m| (a.world() * *m).w_axis.truncate());
        for (i, n) in a.model.model.nodes.iter().enumerate() {
            if let (Some(p), Some(q)) = (at(i), n.parent.and_then(at)) {
                overlay.xray_line(q, p, color);
            }
        }
    }
}

/// The nodes as dots: lights, emitters, meshes and others in their colours.
fn skeleton_markers(painter: &egui::Painter, proj: &Projector, stage: &Stage) {
    for a in stage.actors().iter().filter(|a| a.visible) {
        for (i, n) in a.model.model.nodes.iter().enumerate() {
            let Some(p) = a.pose().get(i).map(|m| (a.world() * *m).w_axis.truncate()) else {
                continue;
            };
            let Some(sp) = proj.point(p) else { continue };
            let color = match n.kind {
                mg_mdl::NodeKind::Light(_) => egui::Color32::from_rgb(255, 240, 150),
                mg_mdl::NodeKind::Emitter(_) => egui::Color32::from_rgb(120, 220, 255),
                mg_mdl::NodeKind::Mesh(_) => egui::Color32::from_rgb(200, 200, 200),
                _ => egui::Color32::from_rgb(255, 180, 60),
            };
            painter.circle_filled(sp, 2.5, color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection() {
        let cam = mg_render::Camera::orbit(Vec3::ZERO, 10.0, 0.0, 0.0);
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(200.0, 100.0));
        let p = Projector::new(&cam, rect);
        let centre = p.point(Vec3::ZERO).unwrap();
        assert!((centre.x - 100.0).abs() < 1e-3 && (centre.y - 50.0).abs() < 1e-3);
        // Behind the camera (the eye is at +X 10).
        assert!(p.point(Vec3::new(20.0, 0.0, 0.0)).is_none());
    }
}
