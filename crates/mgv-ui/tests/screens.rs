//! Screenshots of the window for looking at its layout, rendered with wgpu
//! into `target/test-output/screens/`. Not checks; run by hand:
//! `cargo test -p mgv-ui --test screens -- --ignored`.

use std::path::PathBuf;

use egui_kittest::Harness;
use mg_core::ResType;
use mg_resman::ResKey;
use mgv_ui::{Action, NoDialogs, Settings, Viewer};

fn out_dir() -> PathBuf {
    let d = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/test-output/screens");
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn shoot(h: &mut Harness<'_, Viewer>, name: &str) {
    // Steps rather than runs: animated views keep repainting.
    h.run_steps(4);
    let image = h.render().expect("render");
    image.save(out_dir().join(format!("{name}.png"))).unwrap();
}

fn viewer(root: PathBuf) -> (Viewer, egui_wgpu::RenderState) {
    let rs = egui_kittest::wgpu::create_render_state(
        egui_kittest::wgpu::default_wgpu_setup(),
        egui_wgpu::RendererOptions::PREDICTABLE,
    );
    let settings = Settings { game_root: Some(root), use_user_dir: false, ..Settings::default() };
    let mut v = Viewer::new(settings, Box::new(NoDialogs));
    v.set_render_state(rs.clone());
    (v, rs)
}

#[test]
#[ignore]
fn window() {
    mg_testkit::gpu::hold();
    let root = mg_testkit::corpus!();
    let (mut v, rs) = viewer(root);
    v.actions.push(Action::OpenResource(ResKey::parse("c_golemerald", ResType::MDL).unwrap()));
    let mut h = Harness::builder()
        .with_size(egui::vec2(1500.0, 900.0))
        .renderer(egui_kittest::wgpu::WgpuTestRenderer::from_render_state(rs))
        .build_ui_state(|ui, v: &mut Viewer| v.ui(ui), v);
    shoot(&mut h, "binary-model");
    h.state_mut().actions.push(Action::Decompile);
    shoot(&mut h, "decompiled");
    h.state_mut()
        .actions
        .push(Action::OpenResource(ResKey::parse("plc_a01", ResType::MDL).unwrap()));
    shoot(&mut h, "placeable-after-decompile");
    h.state_mut().actions.push(Action::CloseBuffer);
    h.state_mut()
        .actions
        .push(Action::OpenResource(ResKey::parse("plc_a01", ResType::MDL).unwrap()));
    shoot(&mut h, "placeable");
    h.state_mut().actions.push(Action::OpenResource(
        ResKey::parse("c_wererat", ResType::DDS)
            .unwrap_or(ResKey::parse("c_wererat", ResType::TGA).unwrap()),
    ));
    shoot(&mut h, "texture");
}

#[test]
#[ignore]
fn light_theme() {
    mg_testkit::gpu::hold();
    let root = mg_testkit::corpus!();
    let (mut v, rs) = viewer(root);
    v.settings.theme = mg_uikit::Theme::Light;
    v.actions.push(Action::OpenResource(ResKey::parse("c_golemerald", ResType::MDL).unwrap()));
    v.actions.push(Action::Decompile);
    let mut h = Harness::builder()
        .with_size(egui::vec2(1500.0, 900.0))
        .renderer(egui_kittest::wgpu::WgpuTestRenderer::from_render_state(rs))
        .build_ui_state(|ui, v: &mut Viewer| v.ui(ui), v);
    shoot(&mut h, "light-decompiled");
    h.state_mut()
        .actions
        .push(Action::OpenResource(ResKey::parse("plc_a01", ResType::MDL).unwrap()));
    shoot(&mut h, "light-placeable");
}

#[test]
#[ignore]
fn overlays() {
    mg_testkit::gpu::hold();
    let root = mg_testkit::corpus!();
    let (mut v, rs) = viewer(root);
    v.actions.push(Action::OpenResource(ResKey::parse("tcn01_a01_01", ResType::WOK).unwrap()));
    let mut h = Harness::builder()
        .with_size(egui::vec2(1500.0, 900.0))
        .renderer(egui_kittest::wgpu::WgpuTestRenderer::from_render_state(rs))
        .build_ui_state(|ui, v: &mut Viewer| v.ui(ui), v);
    shoot(&mut h, "walkmesh-tile");
    h.state_mut()
        .actions
        .push(Action::OpenResource(ResKey::parse("plc_a01", ResType::MDL).unwrap()));
    h.state_mut().settings.overlays = mgv_ui::settings::Overlays {
        walkmesh: true,
        wireframe: true,
        normals: false,
        skeleton: true,
        casters: false,
    };
    shoot(&mut h, "overlays-placeable");
    h.state_mut().actions.push(Action::OpenCreature(mgv_stage::subject::CreatureLook::new(6)));
    h.state_mut().settings.overlays = mgv_ui::settings::Overlays {
        walkmesh: false,
        wireframe: false,
        normals: false,
        skeleton: true,
        casters: false,
    };
    shoot(&mut h, "overlays-skeleton");
}

#[test]
#[ignore]
fn plt_colors() {
    use egui_kittest::kittest::Queryable;
    mg_testkit::gpu::hold();
    let root = mg_testkit::corpus!();
    let (mut v, rs) = viewer(root);
    v.actions.push(Action::OpenResource(ResKey::parse("a_halforc", ResType::MDL).unwrap()));
    let mut h = Harness::builder()
        .with_size(egui::vec2(1500.0, 900.0))
        .renderer(egui_kittest::wgpu::WgpuTestRenderer::from_render_state(rs))
        .build_ui_state(|ui, v: &mut Viewer| v.ui(ui), v);
    h.run_steps(3);
    h.state_mut().actions.push(Action::SetColors(Some([12, 23, 8, 30, 90, 100, 60, 70, 5, 6])));
    shoot(&mut h, "plt-colors");
    h.get_by_label("Cloth 1 color").click();
    shoot(&mut h, "plt-colors-open");
}

/// The README's pictures (`docs/images/`, converted by hand).
#[test]
#[ignore]
fn readme() {
    use egui_kittest::kittest::Queryable;
    mg_testkit::gpu::hold();
    let root = mg_testkit::corpus!();
    let (mut v, rs) = viewer(root);
    v.actions.push(Action::OpenResource(ResKey::parse("c_drgred", ResType::MDL).unwrap()));
    let mut h = Harness::builder()
        .with_size(egui::vec2(1600.0, 940.0))
        .renderer(egui_kittest::wgpu::WgpuTestRenderer::from_render_state(rs))
        .build_ui_state(|ui, v: &mut Viewer| v.ui(ui), v);
    h.run_steps(6);
    h.state_mut().actions.push(Action::Decompile);
    h.run_steps(30);
    shoot(&mut h, "readme-dragon");
    h.state_mut().actions.push(Action::CloseBuffer);
    let mut look = mgv_stage::subject::CreatureLook::new(4);
    look.body = 8;
    look.colors = [3, 20, 0, 0];
    h.state_mut().actions.push(Action::OpenCreature(look));
    h.run_steps(6);
    h.state_mut().actions.push(Action::Frame);
    h.run_steps(40);
    h.get_by_label("Hair color color").click();
    shoot(&mut h, "readme-creature");
    h.state_mut()
        .actions
        .push(Action::OpenResource(ResKey::parse("plc_a01", ResType::MDL).unwrap()));
    h.state_mut().settings.overlays = mgv_ui::settings::Overlays {
        walkmesh: true,
        wireframe: true,
        normals: false,
        skeleton: false,
        casters: false,
    };
    h.run_steps(10);
    shoot(&mut h, "readme-overlays");
}

/// A window of `size` on the game's corpus.
fn sized(v: Viewer, rs: egui_wgpu::RenderState, size: egui::Vec2) -> Harness<'static, Viewer> {
    Harness::builder()
        .with_size(size)
        .renderer(egui_kittest::wgpu::WgpuTestRenderer::from_render_state(rs))
        .build_ui_state(|ui, v: &mut Viewer| v.ui(ui), v)
}

/// The redesign's "Model with ASCII Editor" board in the 5.x shell (M6
/// part B), then "Creature and Colors" with the Overlays popover drawn
/// open, at 1600 by 900. Written as `m6-*.png`, as are the shots below.
#[test]
#[ignore]
fn shell_board() {
    use egui_kittest::kittest::Queryable;
    mg_testkit::gpu::hold();
    let (mut v, rs) = viewer(mg_testkit::corpus!());
    v.actions.push(Action::OpenResource(ResKey::parse("plc_a01", ResType::MDL).unwrap()));
    let mut h = sized(v, rs, egui::vec2(1600.0, 900.0));
    h.run_steps(4);
    h.state_mut().actions.push(Action::Decompile);
    h.run_steps(4);
    shoot(&mut h, "m6-model-ascii");
    h.state_mut().actions.push(Action::CloseBuffer);
    h.state_mut().actions.push(Action::OpenCreature(mgv_stage::subject::CreatureLook::new(6)));
    h.run_steps(6);
    h.get_by_label(mgv_ui::view3d::HEADER).get_by_label("Overlays").click();
    shoot(&mut h, "m6-creature-colors");
}

/// A decompiled model in a window `size` big, shot as `name`.
fn decompiled_at(name: &str, size: egui::Vec2) {
    mg_testkit::gpu::hold();
    let (mut v, rs) = viewer(mg_testkit::corpus!());
    v.actions.push(Action::OpenResource(ResKey::parse("c_golemerald", ResType::MDL).unwrap()));
    v.actions.push(Action::Decompile);
    let mut h = sized(v, rs, size);
    h.run_steps(6);
    shoot(&mut h, name);
}

/// The 3440 by 1440 window: the two-column Right Dock, three editor
/// groups, the Overlays unfolded and the toggles' words.
#[test]
#[ignore]
fn shell_wide() {
    decompiled_at("m6-wide-3440", egui::vec2(3440.0, 1440.0));
}

/// A 1280 by 800 window: the Left Dock folded, the Right Dock tabbed.
#[test]
#[ignore]
fn shell_narrow() {
    decompiled_at("m6-narrow-1280", egui::vec2(1280.0, 800.0));
}

/// The least window the shell lays out with a Right Dock (800 by 600;
/// under 796 it folds too).
#[test]
#[ignore]
fn shell_least() {
    decompiled_at("m6-least-800", egui::vec2(800.0, 600.0));
}

/// Steps until the galleries' pictures asked for are all made (a few a
/// frame; the game's programs compiled meanwhile), two minutes at most.
fn pictures_made(h: &mut Harness<'_, Viewer>) {
    let started = std::time::Instant::now();
    h.run_steps(2);
    while h.state().pictures.waiting() > 0 && started.elapsed().as_secs() < 120 {
        h.step();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    h.run_steps(2);
}

/// The board "Resources as a Gallery" (M6 part C): the Creatures filter
/// in Gallery mode in the Left Dock, a creature shown, at 1600 by 900.
#[test]
#[ignore]
fn gallery_panel() {
    mg_testkit::gpu::hold();
    let (mut v, rs) = viewer(mg_testkit::corpus!());
    v.settings.resources_view = mgv_ui::settings::ResourcesView::Gallery;
    v.browser.filter = mgv_ui::panels::Filter::Creatures;
    v.browser.chosen_row = Some(190);
    v.actions.push(Action::OpenCreature(mgv_stage::subject::CreatureLook::new(190)));
    let mut h = sized(v, rs, egui::vec2(1600.0, 900.0));
    pictures_made(&mut h);
    shoot(&mut h, "m6-gallery-panel");
    // The models, and their pictures.
    h.state_mut().browser.filter = mgv_ui::panels::Filter::Models;
    pictures_made(&mut h);
    shoot(&mut h, "m6-gallery-panel-models");
}

/// The Gallery tab (Open as Tab) at `size`: `filter`'s pictures beside
/// the 3D View, shot as `name`.
fn gallery_tab_at(name: &str, size: egui::Vec2, filter: mgv_ui::panels::Filter) {
    mg_testkit::gpu::hold();
    let (mut v, rs) = viewer(mg_testkit::corpus!());
    v.browser.filter = filter;
    v.open_gallery_tab();
    v.actions.push(Action::OpenCreature(mgv_stage::subject::CreatureLook::new(49)));
    let mut h = sized(v, rs, size);
    pictures_made(&mut h);
    shoot(&mut h, name);
}

/// The board "Gallery Tab at 3440×1440": the creatures as a tab beside
/// the 3D View; and the models' at 1600 by 900.
#[test]
#[ignore]
fn gallery_tab() {
    use mgv_ui::panels::Filter;
    gallery_tab_at("m6-gallery-tab", egui::vec2(1600.0, 900.0), Filter::Models);
    gallery_tab_at("m6-gallery-tab-3440", egui::vec2(3440.0, 1440.0), Filter::Creatures);
}

/// M6 part D at 1600 by 900: the Timeline's transport row over its scrub
/// bar and ruler (an animated creature model), the Texture's header and
/// caption line, the Inspector's Creature grid with Skin color's color grid
/// open, and the Lighting panel dragged onto the Bottom Dock (its drop
/// outlined, then landed).
#[test]
#[ignore]
fn part_d() {
    use egui_kittest::kittest::Queryable;
    mg_testkit::gpu::hold();
    let (mut v, rs) = viewer(mg_testkit::corpus!());
    v.actions.push(Action::OpenResource(ResKey::parse("c_golemerald", ResType::MDL).unwrap()));
    let mut h = sized(v, rs, egui::vec2(1600.0, 900.0));
    h.run_steps(6);
    h.state_mut().focus(mgv_ui::Tab::Timeline);
    shoot(&mut h, "m6-d-timeline");
    h.state_mut().actions.push(Action::OpenResource(
        ResKey::parse("c_wererat", ResType::DDS)
            .unwrap_or(ResKey::parse("c_wererat", ResType::TGA).unwrap()),
    ));
    shoot(&mut h, "m6-d-texture");
    h.state_mut().actions.push(Action::OpenCreature(mgv_stage::subject::CreatureLook::new(6)));
    h.run_steps(6);
    h.state_mut().focus(mgv_ui::Tab::Inspector);
    h.run_steps(2);
    h.get_by_label("Skin color swatch").click();
    shoot(&mut h, "m6-d-creature");
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    let lighting = h.get_by_label("Lighting panel").rect();
    let bottom = h.get_by_label("Bottom Dock").rect();
    let from = egui::pos2(lighting.left() + 40.0, lighting.top() + 12.0);
    let to = bottom.center() + egui::vec2(0.0, 40.0);
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    h.hover_at(from);
    h.input_mut().events.push(button(from, true));
    h.step();
    for i in 1..=8 {
        h.hover_at(from + (to - from) * (i as f32 / 8.0));
        h.step();
    }
    shoot(&mut h, "m6-d-dragging");
    h.input_mut().events.push(button(to, false));
    shoot(&mut h, "m6-d-dragged");
}
