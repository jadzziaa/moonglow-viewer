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
    };
    shoot(&mut h, "overlays-placeable");
    h.state_mut().actions.push(Action::OpenCreature(mgv_stage::subject::CreatureLook::new(6)));
    h.state_mut().settings.overlays = mgv_ui::settings::Overlays {
        walkmesh: false,
        wireframe: false,
        normals: false,
        skeleton: true,
    };
    shoot(&mut h, "overlays-skeleton");
}
