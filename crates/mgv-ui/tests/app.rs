//! The window's flows, driven through egui_kittest: opening, hot reload
//! from the editor and from disk, decompiling and saving.

use std::path::{Path, PathBuf};

use egui_kittest::Harness;
use mg_core::ResType;
use mg_resman::ResKey;
use mgv_ui::{Action, Dialogs, Settings, Viewer};

/// A one-triangle model with a `hook` dummy.
const MODEL: &str = "newmodel tri\nsetsupermodel tri NULL\nclassification character\n\
beginmodelgeom tri\n\
node dummy tri\n  parent NULL\nendnode\n\
node trimesh plane\n  parent tri\n  verts 3\n    0 0 0\n    1 0 0\n    0 1 0\n\
  faces 1\n    0 1 2 1 0 0 0 0\nendnode\n\
node dummy hook\n  parent tri\n  position 0 0 2\nendnode\n\
endmodelgeom tri\ndonemodel tri\n";

fn scratch(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../target/test-output/ui-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Dialogs that answer with a fixed path.
struct Answer(Option<PathBuf>);

impl Dialogs for Answer {
    fn open_file(&mut self, _: &str, _: &[(&str, &[&str])]) -> Option<PathBuf> {
        self.0.clone()
    }
    fn save_file(&mut self, _: &str, _: &Path) -> Option<PathBuf> {
        self.0.clone()
    }
    fn pick_folder(&mut self, _: &str) -> Option<PathBuf> {
        self.0.clone()
    }
}

fn harness(game: bool, dialogs: Box<dyn Dialogs>) -> Option<Harness<'static, Viewer>> {
    mg_testkit::gpu::hold();
    let root = if game { Some(mg_testkit::nwn_root()?) } else { None };
    // CI runners may have no GPU adapter at all.
    if mg_render::Gpu::headless().is_none() {
        eprintln!("skipped: no GPU adapter");
        return None;
    }
    let rs = egui_kittest::wgpu::create_render_state(
        egui_kittest::wgpu::default_wgpu_setup(),
        egui_wgpu::RendererOptions::PREDICTABLE,
    );
    let settings = Settings {
        game_root: root.or_else(|| Some(PathBuf::from("/nonexistent"))),
        use_user_dir: false,
        ..Settings::default()
    };
    let mut v = Viewer::new(settings, dialogs);
    v.set_render_state(rs.clone());
    Some(
        Harness::builder()
            .with_size(egui::vec2(1200.0, 800.0))
            .renderer(egui_kittest::wgpu::WgpuTestRenderer::from_render_state(rs))
            .build_ui_state(|ui, v: &mut Viewer| v.ui(ui), v),
    )
}

fn base_model(h: &Harness<'_, Viewer>) -> std::sync::Arc<mg_mdl::Model> {
    let v = h.state();
    let base = v.doc.as_ref().unwrap().shown.as_ref().unwrap().base;
    v.gfx.as_ref().unwrap().stage.actor(base).unwrap().model.model.clone()
}

#[test]
fn editing_the_ascii_reloads_the_view() {
    let Some(mut h) = harness(false, Box::new(Answer(None))) else { return };
    let dir = scratch("edit");
    let file = dir.join("tri.mdl");
    std::fs::write(&file, MODEL).unwrap();
    h.state_mut().actions.push(Action::Open(file.clone()));
    h.run_steps(3);
    assert!(h.state().buffer.is_some(), "an ASCII model opens in the editor");
    assert!(base_model(&h).node("hook").is_some());

    // An edit: rename the dummy. The view follows after the pause; the
    // file on disk does not change.
    let edited = MODEL.replace("node dummy hook", "node dummy grip");
    let now = h.ctx.input(|i| i.time);
    h.state_mut().buffer.as_mut().unwrap().set_text(&edited, now);
    h.run_steps(30);
    assert!(base_model(&h).node("grip").is_some(), "hot reload");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), MODEL, "not saved");
    assert!(h.state().buffer.as_ref().unwrap().is_dirty());
    assert!(h.state().title().contains('*'));

    // Save writes it.
    h.state_mut().actions.push(Action::Save);
    h.run_steps(2);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), edited);
    assert!(!h.state().buffer.as_ref().unwrap().is_dirty());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_file_changed_on_disk_reloads() {
    let Some(mut h) = harness(false, Box::new(Answer(None))) else { return };
    if !h.state().watcher.available() {
        eprintln!("skipped: no file notifications");
        return;
    }
    let dir = scratch("disk");
    let file = dir.join("tri.mdl");
    std::fs::write(&file, MODEL).unwrap();
    h.state_mut().actions.push(Action::Open(file.clone()));
    h.run_steps(3);
    std::fs::write(&file, MODEL.replace("node dummy hook", "node dummy handle")).unwrap();
    let mut reloaded = false;
    for _ in 0..100 {
        h.run_steps(2);
        std::thread::sleep(std::time::Duration::from_millis(20));
        if base_model(&h).node("handle").is_some() {
            reloaded = true;
            break;
        }
    }
    assert!(reloaded, "the view follows the file");
    assert!(h.state().buffer.as_ref().unwrap().text().contains("node dummy handle"));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn decompiling_a_game_model_edits_it_in_place() {
    let Some(mut h) = harness(true, Box::new(Answer(None))) else {
        eprintln!("skipped: no game install");
        return;
    };
    let key = ResKey::parse("plc_a01", ResType::MDL).unwrap();
    h.state_mut().actions.push(Action::OpenResource(key));
    h.run_steps(3);
    let nodes = base_model(&h).nodes.len();
    h.state_mut().actions.push(Action::Decompile);
    h.run_steps(3);
    let v = h.state();
    let b = v.buffer.as_ref().expect("decompiled into the editor");
    assert!(b.text().starts_with("#MAXMODEL ASCII"));
    assert!(b.is_dirty(), "not saved anywhere yet");
    assert_eq!(v.lib.origin(&key), Some("editor buffers"), "the view shows the ASCII");
    assert_eq!(base_model(&h).nodes.len(), nodes);
    assert!(
        v.buffer
            .as_ref()
            .unwrap()
            .diagnostics
            .iter()
            .all(|d| d.severity < mgv_mdl::Severity::Warning)
    );

    // Opening something else keeps the edits (the user closes them first).
    h.state_mut()
        .actions
        .push(Action::OpenResource(ResKey::parse("c_wererat", ResType::MDL).unwrap()));
    h.run_steps(2);
    assert!(h.state().buffer.is_some());
    assert_eq!(h.state().doc.as_ref().unwrap().name(), "plc_a01");
    h.state_mut().actions.push(Action::CloseBuffer);
    h.run_steps(2);
    assert_eq!(h.state().lib.origin(&key).map(|o| o.starts_with("key:")), Some(true));
}

#[test]
fn saving_a_decompiled_model_asks_where() {
    let dir = scratch("save");
    let target = dir.join("plc_a01.mdl");
    let Some(mut h) = harness(true, Box::new(Answer(Some(target.clone())))) else { return };
    h.state_mut()
        .actions
        .push(Action::OpenResource(ResKey::parse("plc_a01", ResType::MDL).unwrap()));
    h.state_mut().actions.push(Action::Decompile);
    h.state_mut().actions.push(Action::Save);
    h.run_steps(3);
    let text = std::fs::read_to_string(&target).expect("saved where the dialog said");
    assert!(text.contains("newmodel plc_a01") || text.contains("newmodel PLC_A01"));
    assert!(!h.state().buffer.as_ref().unwrap().is_dirty());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn creatures_and_effects() {
    let Some(mut h) = harness(true, Box::new(Answer(None))) else { return };
    h.state_mut().actions.push(Action::OpenCreature(mgv_stage::subject::CreatureLook::new(6)));
    h.run_steps(3);
    assert_eq!(h.state().doc.as_ref().unwrap().name(), "appearance 6");
    let actors = h.state().gfx.as_ref().unwrap().stage.actors().len();
    assert!(actors > 5, "a part-based human: {actors} actors");
    // Globe of invulnerability (a ground model) and mind-affecting (head).
    h.state_mut().actions.push(Action::ApplyEffect(4));
    h.state_mut().actions.push(Action::ApplyEffect(7));
    h.run_steps(3);
    assert_eq!(h.state().effects.applied.len(), 2);
    let after = h.state().gfx.as_ref().unwrap().stage.actors().len();
    assert_eq!(after, actors + 2);
    h.state_mut().actions.push(Action::RemoveEffect(0));
    h.run_steps(2);
    assert_eq!(h.state().effects.applied.len(), 1);
    // A new look keeps nothing of the old.
    let mut look = mgv_stage::subject::CreatureLook::new(6);
    look.gender = 1;
    h.state_mut().actions.push(Action::OpenCreature(look));
    h.run_steps(2);
    assert!(h.state().effects.applied.is_empty());
}

#[test]
fn materials_and_tilesets() {
    let Some(mut h) = harness(true, Box::new(Answer(None))) else { return };
    h.state_mut()
        .actions
        .push(Action::OpenResource(ResKey::parse("tti01_icefloor", ResType::MTR).unwrap()));
    h.run_steps(3);
    let m = base_model(&h);
    assert_eq!(m.nodes.len(), 2, "a sphere wearing the material");
    assert_eq!(m.nodes[1].mesh().unwrap().material.as_deref(), Some("tti01_icefloor"));
    h.state_mut().actions.push(Action::OpenResource(ResKey::parse("tcn01", ResType::SET).unwrap()));
    h.run_steps(3);
    let v = h.state();
    assert_eq!(v.doc.as_ref().unwrap().opened.kind, mgv_library::Kind::Tileset);
    assert!(v.log.iter().any(|l| l.text.contains("tiles are listed")));
}
