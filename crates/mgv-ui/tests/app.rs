//! The window's flows, driven through egui_kittest: opening, hot reload
//! from the editor and from disk, decompiling and saving.

use std::path::{Path, PathBuf};

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
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

/// The window at the board's size (proposal section 9.17, "Model with
/// ASCII Editor"): both side docks open, the Bottom Dock under the editor
/// area.
const BOARD: egui::Vec2 = egui::vec2(1600.0, 900.0);

fn harness(game: bool, dialogs: Box<dyn Dialogs>) -> Option<Harness<'static, Viewer>> {
    harness_sized(game, dialogs, BOARD)
}

fn harness_sized(
    game: bool,
    dialogs: Box<dyn Dialogs>,
    size: egui::Vec2,
) -> Option<Harness<'static, Viewer>> {
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
            .with_size(size)
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
            .all(|d| d.severity < mg_mdl::lint::Severity::Warning)
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

/// A click in the resource list opens what it names; a loop of one frame
/// (placeables' `default`) shows as a pose, not on a slider thrown from
/// end to end every frame.
#[test]
fn a_click_opens_and_one_frame_loops_are_a_pose() {
    let Some(mut h) = harness(false, Box::new(Answer(None))) else { return };
    let dir = scratch("click");
    std::fs::write(dir.join("tri.mdl"), MODEL).unwrap();
    let posed = MODEL.replace("donemodel tri", "")
        + "newanim default other\n  length 0.0333333\n  transtime 0\n\
           node dummy other\n    parent NULL\n  endnode\ndoneanim default other\n\
           newanim spin other\n  length 1\n  transtime 0\n\
           node dummy other\n    parent NULL\n  endnode\ndoneanim spin other\n\
           donemodel other\n";
    std::fs::write(dir.join("other.mdl"), posed.replace(" tri", " other")).unwrap();
    h.state_mut().actions.push(Action::Open(dir.join("tri.mdl")));
    h.run_steps(3);
    assert_eq!(h.state().doc.as_ref().unwrap().name(), "tri");
    h.get_by_label("other.mdl").click();
    h.run_steps(3);
    assert_eq!(h.state().doc.as_ref().unwrap().name(), "other", "one click opens it");
    // The arrows then step through the list (other, tri) and open as they go.
    h.key_press(egui::Key::ArrowDown);
    h.run_steps(3);
    assert_eq!(h.state().doc.as_ref().unwrap().name(), "tri", "down: the next");
    h.key_press(egui::Key::ArrowUp);
    h.run_steps(3);
    assert_eq!(h.state().doc.as_ref().unwrap().name(), "other", "up: back");
    // It plays its one-frame default.
    assert!(h.query_by_label_contains("A single pose").is_some());
    let base = h.state().doc.as_ref().unwrap().shown.as_ref().unwrap().base;
    let set = |h: &mut Harness<'_, Viewer>, anim: &str| {
        let g = h.state_mut().gfx.as_mut().unwrap();
        g.stage.actor_mut(base).unwrap().player.play(Some(anim), mgv_stage::PlayMode::Loop);
    };
    set(&mut h, "spin");
    h.run_steps(2);
    assert!(h.query_by_label_contains("A single pose").is_none(), "a second's loop slides");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// WASD drives the camera once the view has the keyboard.
#[test]
fn wasd_moves_the_camera() {
    let Some(mut h) = harness(false, Box::new(Answer(None))) else { return };
    let dir = scratch("wasd");
    std::fs::write(dir.join("tri.mdl"), MODEL).unwrap();
    h.state_mut().actions.push(Action::Open(dir.join("tri.mdl")));
    h.run_steps(3);
    h.get_by_role(egui::accesskit::Role::Image).click();
    h.run_steps(2);
    let before = h.state().camera.target;
    h.key_down(egui::Key::W);
    h.run_steps(5);
    h.key_up(egui::Key::W);
    h.run_steps(1);
    let camera = h.state().camera;
    let moved = camera.target - before;
    // Forward: along the ground, away from the eye.
    let ahead = glam::Vec3::new(-camera.yaw.cos(), -camera.yaw.sin(), 0.0);
    assert!(moved.length() > 1e-3 && moved.normalize().dot(ahead) > 0.99, "{moved}");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A model with PLT textures opened on its own has colors to set (a
/// creature would give them); a model without has none.
#[test]
fn bare_plt_models_take_colors() {
    let Some(mut h) = harness(true, Box::new(Answer(None))) else { return };
    let look = |h: &Harness<'_, Viewer>| {
        let v = h.state();
        let base = v.doc.as_ref().unwrap().shown.as_ref().unwrap().base;
        v.gfx.as_ref().unwrap().stage.actor(base).unwrap().look.colors
    };
    h.state_mut()
        .actions
        .push(Action::OpenResource(ResKey::parse("a_halforc", ResType::MDL).unwrap()));
    h.run_steps(3);
    assert!(h.query_by_label("PLT colors").is_some());
    assert_eq!(look(&h), None);
    let mut colors = [0; 10];
    colors[2] = 40;
    h.state_mut().actions.push(Action::SetColors(Some(colors)));
    h.run_steps(2);
    assert_eq!(look(&h), Some(colors));
    // The chooser: its swatches by name, from the game's palette.
    // (The Inspector shares its column with Lighting, and the PLT colors
    // may be scrolled out of its sight: chosen through accesskit, which
    // reaches a control out of sight, as a user's scroll would.)
    h.get_by_label("Metal 2 color").click_accesskit();
    h.run_steps(2);
    h.get_by_label("Metal 2 17").click_accesskit();
    h.run_steps(2);
    colors[3] = 17;
    assert_eq!(look(&h), Some(colors));
    h.get_by_label("Reset colors").click_accesskit();
    h.run_steps(2);
    assert_eq!(look(&h), None);
    h.state_mut()
        .actions
        .push(Action::OpenResource(ResKey::parse("plc_a01", ResType::MDL).unwrap()));
    h.run_steps(3);
    assert!(h.query_by_label("PLT colors").is_none());
}

/// The game's shaders draw the view by default, compiled while the frames
/// go on (Moonglow's shader drawing until they are ready); the toolbar's
/// Shaders switch turns them off, and the setting keeps it.
#[test]
fn the_game_s_shaders_draw_the_view_and_switch_off() {
    let Some(mut h) = harness(true, Box::new(Answer(None))) else { return };
    let drawn = |h: &Harness<'_, Viewer>| h.state().gfx.as_ref().unwrap().viewport.drawn();
    assert!(h.state().settings.game_shaders(), "on by default");
    // (The model alone: the Shadow switch's ground is unlit, Moonglow's.)
    h.state_mut().settings.model_shadows = Some(false);
    h.state_mut()
        .actions
        .push(Action::OpenResource(ResKey::parse("plc_a01", ResType::MDL).unwrap()));
    h.run_steps(3);
    let start = std::time::Instant::now();
    while drawn(&h).game_pending > 0 || drawn(&h).game == 0 {
        assert!(start.elapsed().as_secs() < 60, "never drawn by them: {:?}", drawn(&h));
        std::thread::sleep(std::time::Duration::from_millis(20));
        h.run_steps(1);
    }
    let d = drawn(&h);
    assert_eq!(d.game, d.meshes, "every mesh by the game's programs: {d:?}");
    h.get_by_label(mgv_ui::view3d::HEADER).get_by_label("Shaders").click();
    h.run_steps(2);
    assert_eq!(h.state().settings.game_shaders, Some(false));
    assert_eq!(drawn(&h).game, 0, "Moonglow's own shader");
    assert!(drawn(&h).meshes > 0);
}

/// The model casts its shadow on a ground under it unless the toolbar's
/// Shadow switch is turned off; the setting keeps it.
#[test]
fn the_shadow_switch() {
    let Some(mut h) = harness(true, Box::new(Answer(None))) else { return };
    assert!(h.state().settings.model_shadows(), "on by default");
    h.state_mut()
        .actions
        .push(Action::OpenResource(ResKey::parse("plc_a01", ResType::MDL).unwrap()));
    h.run_steps(3);
    assert!(h.state().gfx.as_ref().unwrap().stage.shadow);
    h.get_by_label(mgv_ui::view3d::HEADER).get_by_label("Shadow").click();
    h.run_steps(2);
    assert_eq!(h.state().settings.model_shadows, Some(false));
    assert!(!h.state().gfx.as_ref().unwrap().stage.shadow);
}

/// The Top Bar's buttons do what their menu items do, and are disabled
/// where those are: Save without an ASCII model, Open asking for a file.
#[test]
fn the_top_bar_s_buttons_are_the_menus() {
    let dir = scratch("top-bar");
    let file = dir.join("tri.mdl");
    std::fs::write(&file, MODEL).unwrap();
    let Some(mut h) = harness(false, Box::new(Answer(Some(file.clone())))) else { return };
    h.run_steps(2);
    let disabled = |h: &Harness<'_, Viewer>, name: &str| {
        h.get_by_label("Top Bar").get_by_label(name).accesskit_node().is_disabled()
    };
    assert!(disabled(&h, "Save"), "nothing to save");
    assert!(!disabled(&h, "Open"));
    // Open asks for a file (the dialog answers with the model).
    h.get_by_label("Top Bar").get_by_label("Open").click();
    h.run_steps(3);
    assert_eq!(h.state().doc.as_ref().unwrap().name(), "tri");
    assert!(!disabled(&h, "Save"));
    // Save writes the edits.
    let edited = MODEL.replace("node dummy hook", "node dummy grip");
    let now = h.ctx.input(|i| i.time);
    h.state_mut().buffer.as_mut().unwrap().set_text(&edited, now);
    h.run_steps(2);
    h.get_by_label("Top Bar").get_by_label("Save").click();
    h.run_steps(2);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), edited);
    // Decompile says, as the menu's does, that an ASCII model does not.
    h.get_by_label("Top Bar").get_by_label("Decompile").click();
    h.run_steps(2);
    assert!(h.state().log.iter().any(|l| l.text == "Only compiled models decompile."));
    // Compile and Compile and View start the compiler on the editor's text.
    for name in ["Compile", "Compile and View"] {
        let before = h.state().log.len();
        h.get_by_label("Top Bar").get_by_label(name).click();
        h.run_steps(2);
        assert!(h.state().log[before..].iter().any(|l| l.text.starts_with("Compiling tri")));
    }
    // (The compiles run on, their files landing in the scratch folder:
    // waited for, so that the folder goes after them.)
    let done = |h: &Harness<'_, Viewer>| {
        let log = &h.state().log;
        log.iter()
            .filter(|l| l.text.starts_with("Compiled tri") || l.text.starts_with("Compiling tri:"))
            .count()
    };
    for _ in 0..500 {
        if done(&h) >= 2 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
        h.run_steps(1);
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The 3D View's Overlays popover: its rows toggle the overlays and leave
/// it open.
#[test]
fn the_overlays_popover_toggles() {
    let Some(mut h) = harness(false, Box::new(Answer(None))) else { return };
    h.run_steps(2);
    let header = mgv_ui::view3d::HEADER;
    h.get_by_label(header).get_by_label("Overlays").click();
    h.run_steps(2);
    h.get_by_label("Wireframe").click();
    h.run_steps(2);
    assert!(h.state().settings.overlays.wireframe);
    h.get_by_label("Walkmesh").click();
    h.run_steps(2);
    assert!(h.state().settings.overlays.walkmesh, "still open after a row");
    h.get_by_label("Wireframe").click();
    h.run_steps(2);
    assert!(!h.state().settings.overlays.wireframe);
    // Grid and Axes are the header's toggles.
    let grid = h.state().settings.show_grid;
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    h.get_by_label(header).get_by_label("Grid").click();
    h.run_steps(2);
    assert_eq!(h.state().settings.show_grid, !grid);
}

/// The ASCII tab shows the modified dot (no " *" in its title) while its
/// edits are unsaved; its header's Save takes it away. The diagnostics
/// strip under the text counts the errors and the warnings.
#[test]
fn the_ascii_tab_s_dot_and_the_diagnostics_counts() {
    use mgv_ui::editor::{DIAGNOSTICS, HEADER, counts, plural};
    let dir = scratch("dot");
    let file = dir.join("tri.mdl");
    std::fs::write(&file, MODEL).unwrap();
    let Some(mut h) = harness(false, Box::new(Answer(None))) else { return };
    h.state_mut().actions.push(Action::Open(file.clone()));
    h.run_steps(3);
    // (The tab's button, named by its title: a label reads "ASCII" too.)
    let tab = |h: &Harness<'_, Viewer>| {
        h.query_by_role_and_label(egui::accesskit::Role::Button, "ASCII").is_some()
    };
    assert!(tab(&h), "the tab, named by its title");
    assert!(h.query_by_label("Modified").is_none(), "nothing edited");
    let strip = |h: &Harness<'_, Viewer>, what: &str| {
        h.get_by_label(DIAGNOSTICS).query_by_label(what).is_some()
    };
    assert!(strip(&h, "0 errors") && strip(&h, "0 warnings"));
    // A node without a name, and an endnode without a node.
    let broken = MODEL.replace("endmodelgeom tri", "node\nendnode\nendnode\nendmodelgeom tri");
    let now = h.ctx.input(|i| i.time);
    h.state_mut().buffer.as_mut().unwrap().set_text(&broken, now);
    h.run_steps(30);
    assert!(h.query_by_label("Modified").is_some(), "the dot");
    assert!(h.query_by_label("ASCII *").is_none());
    let (e, w) = counts(&h.state().buffer.as_ref().unwrap().diagnostics);
    assert!(e > 0 && w > 0);
    assert!(strip(&h, &plural(e, "error")) && strip(&h, &plural(w, "warning")));
    h.get_by_label(HEADER).get_by_label("Save").click();
    h.run_steps(2);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), broken);
    assert!(h.query_by_label("Modified").is_none(), "saved");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The Status Bar: the document and its origin at the right, the game
/// folder (none here), the 3D view's hints as key caps, and the Bottom
/// Dock's switches (the switch of the tab in front closes the dock).
#[test]
fn the_status_bar() {
    let dir = scratch("status");
    let file = dir.join("tri.mdl");
    std::fs::write(&file, MODEL).unwrap();
    let Some(mut h) = harness(false, Box::new(Answer(None))) else { return };
    h.run_steps(2);
    let bar = |h: &Harness<'_, Viewer>, what: &str| {
        h.get_by_label("Status Bar").query_by_label(what).is_some()
    };
    assert!(bar(&h, "Open a model (Ctrl+O) or pick one under Resources"));
    assert!(bar(&h, "No game folder: File › Game Folder…"));
    h.state_mut().actions.push(Action::Open(file.clone()));
    h.run_steps(3);
    assert!(bar(&h, "tri"));
    let origin = h.state().doc.as_ref().unwrap().opened.path.clone().unwrap();
    assert!(bar(&h, &origin.display().to_string()));
    assert!(h.get_by_label("Status Bar").query_by_label_contains("Orbit").is_some());
    // The Log's switch brings the Log to the front; again, closes the dock.
    assert!(h.state().shell.bottom_open);
    h.get_by_label("Status Bar").get_by_label("Log").click();
    h.run_steps(2);
    assert_eq!(h.state().shell.bottom_tab, mgv_ui::Tab::Log);
    h.get_by_label("Status Bar").get_by_label("Log").click();
    h.run_steps(2);
    assert!(!h.state().shell.bottom_open);
    // The Log out of sight, its last line is in the Status Bar.
    let last = h.state().log.last().unwrap().text.clone();
    assert!(bar(&h, &last));
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A narrow window (under 1500) folds the Left Dock to its strip; its
/// chevron brings it back, and Resources opens what it lists.
#[test]
fn a_narrow_window_folds_the_left_dock() {
    let dir = scratch("narrow");
    std::fs::write(dir.join("tri.mdl"), MODEL).unwrap();
    std::fs::write(dir.join("other.mdl"), MODEL.replace(" tri", " other")).unwrap();
    let size = egui::vec2(1200.0, 800.0);
    let Some(mut h) = harness_sized(false, Box::new(Answer(None)), size) else { return };
    h.state_mut().actions.push(Action::Open(dir.join("tri.mdl")));
    h.run_steps(3);
    assert!(h.state().shell.left_folded());
    assert!(h.query_by_label("other.mdl").is_none());
    h.get_by_label("Show the Left Dock").click();
    h.run_steps(3);
    assert!(!h.state().shell.left_folded());
    h.get_by_label("other.mdl").click();
    h.run_steps(3);
    assert_eq!(h.state().doc.as_ref().unwrap().name(), "other");
    // View › Reset Layout folds it again, as the width says.
    h.state_mut().actions.push(Action::ResetLayout);
    h.run_steps(2);
    assert!(h.state().shell.left_folded());
    std::fs::remove_dir_all(&dir).unwrap();
}
