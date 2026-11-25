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

/// The redesign's viewer boards in the 5.x shell (M6 part B): "Model with
/// ASCII Editor", "Creature and Colors" (the Overlays popover drawn open),
/// the 3440 by 1440 window (two-column Right Dock, three editor groups,
/// the Overlays unfolded), a 1280 by 800 window (the Left Dock folded, the
/// Right Dock tabbed) and the least window the shell lays out (800 by
/// 600: under 796 the Right Dock folds too). Written as `m6-*.png`.
#[test]
#[ignore]
fn shell() {
    use egui_kittest::kittest::Queryable;
    mg_testkit::gpu::hold();
    let root = mg_testkit::corpus!();
    let board = egui::vec2(1600.0, 900.0);

    let (mut v, rs) = viewer(root.clone());
    v.actions.push(Action::OpenResource(ResKey::parse("plc_a01", ResType::MDL).unwrap()));
    let mut h = sized(v, rs, board);
    h.run_steps(4);
    h.state_mut().actions.push(Action::Decompile);
    h.run_steps(4);
    shoot(&mut h, "m6-model-ascii");
    h.state_mut().actions.push(Action::CloseBuffer);
    h.state_mut().actions.push(Action::OpenCreature(mgv_stage::subject::CreatureLook::new(6)));
    h.run_steps(6);
    h.get_by_label(mgv_ui::view3d::HEADER).get_by_label("Overlays").click();
    shoot(&mut h, "m6-creature-colors");
    drop(h);

    let (mut v, rs) = viewer(root.clone());
    v.actions.push(Action::OpenResource(ResKey::parse("c_golemerald", ResType::MDL).unwrap()));
    v.actions.push(Action::Decompile);
    let mut h = sized(v, rs, egui::vec2(3440.0, 1440.0));
    h.run_steps(6);
    shoot(&mut h, "m6-wide-3440");
    drop(h);

    for (name, size) in
        [("m6-narrow-1280", egui::vec2(1280.0, 800.0)), ("m6-least-800", egui::vec2(800.0, 600.0))]
    {
        let (mut v, rs) = viewer(root.clone());
        v.actions.push(Action::OpenResource(ResKey::parse("plc_a01", ResType::MDL).unwrap()));
        v.actions.push(Action::Decompile);
        let mut h = sized(v, rs, size);
        h.run_steps(6);
        shoot(&mut h, name);
    }
}
