//! The stage: authored models (no game needed) and the game's own.

use std::path::PathBuf;

use glam::{Mat4, Vec3};
use mg_resman::GameInstall;
use mgv_library::Library;
use mgv_stage::camera::View;
use mgv_stage::{OrbitCamera, Placement, PlayMode, Stage, Viewport};

/// A one-triangle model with a 1 s animation moving its mesh up 1 m, and a
/// `hook` dummy at (0, 0, 2).
const MOVER: &str = "newmodel mover\nsetsupermodel mover NULL\nclassification character\n\
beginmodelgeom mover\n\
node dummy mover\n  parent NULL\nendnode\n\
node trimesh tri\n  parent mover\n  diffuse 1 1 1\n  verts 3\n    0 0 0\n    1 0 0\n    0 1 0\n\
  faces 1\n    0 1 2 1 0 0 0 0\nendnode\n\
node dummy hook\n  parent mover\n  position 0 0 2\nendnode\n\
endmodelgeom mover\n\
newanim rise mover\n  length 1\n  transtime 0\n\
  node dummy mover\n    parent NULL\n  endnode\n\
  node trimesh tri\n    parent mover\n    positionkey\n      0 0 0 0\n      1 0 0 1\n    endlist\n  endnode\n\
doneanim rise mover\n\
donemodel mover\n";

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("mgv-stage-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn gpu() -> Option<mg_render::Gpu> {
    mg_testkit::gpu::hold();
    let gpu = mg_render::Gpu::headless();
    if gpu.is_none() {
        eprintln!("skipped: no GPU adapter");
    }
    gpu
}

#[test]
fn animation_play_once_holds_the_last_frame() {
    let Some(gpu) = gpu() else { return };
    let dir = scratch("once");
    std::fs::write(dir.join("mover.mdl"), MOVER).unwrap();
    let mut lib = Library::open(None).unwrap();
    lib.open_file(&dir.join("mover.mdl")).unwrap();
    let mut stage = Stage::new(gpu);
    let id = stage.add_model(&lib, "mover", Mat4::IDENTITY).unwrap();
    let tri = stage.actor(id).unwrap().model.model.node("tri").unwrap();
    let height = |s: &Stage| s.actor(id).unwrap().pose()[tri].w_axis.z;

    stage.actor_mut(id).unwrap().player.play(Some("rise"), PlayMode::Once);
    stage.step(&lib, 0.0);
    assert!(height(&stage).abs() < 1e-4);
    stage.step(&lib, 0.5);
    assert!((height(&stage) - 0.5).abs() < 1e-3, "{}", height(&stage));
    stage.step(&lib, 2.0);
    assert!((height(&stage) - 1.0).abs() < 1e-3, "held at the end: {}", height(&stage));

    // Looping wraps instead.
    stage.actor_mut(id).unwrap().player.play(Some("rise"), PlayMode::Loop);
    stage.step(&lib, 1.25);
    assert!((height(&stage) - 0.25).abs() < 1e-3, "{}", height(&stage));

    // Paused: time stands still.
    stage.actor_mut(id).unwrap().player.playing = false;
    stage.step(&lib, 0.5);
    assert!((height(&stage) - 0.25).abs() < 1e-3);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn sequences_move_on_to_the_next_animation() {
    let Some(gpu) = gpu() else { return };
    let dir = scratch("sequence");
    let two = MOVER.replace("donemodel mover", "")
        + "newanim sink mover\n  length 2\n  transtime 0\n\
  node dummy mover\n    parent NULL\n  endnode\n\
  node trimesh tri\n    parent mover\n    positionkey\n      0 0 0 0\n      2 0 0 -2\n    endlist\n  endnode\n\
doneanim sink mover\ndonemodel mover\n";
    std::fs::write(dir.join("mover.mdl"), two).unwrap();
    let mut lib = Library::open(None).unwrap();
    lib.open_file(&dir.join("mover.mdl")).unwrap();
    let mut stage = Stage::new(gpu);
    let id = stage.add_model(&lib, "mover", Mat4::IDENTITY).unwrap();
    stage.actor_mut(id).unwrap().player.sequence(&["rise", "sink"]);
    stage.step(&lib, 1.5);
    let a = stage.actor(id).unwrap();
    assert_eq!(a.current().map(|(n, _)| n), Some("sink"));
    assert!((a.current().unwrap().1 - 0.5).abs() < 1e-4);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn attached_actors_follow_their_node() {
    let Some(gpu) = gpu() else { return };
    let dir = scratch("attach");
    std::fs::write(dir.join("mover.mdl"), MOVER).unwrap();
    std::fs::write(dir.join("rider.mdl"), MOVER.replace("mover", "rider")).unwrap();
    let mut lib = Library::open(None).unwrap();
    lib.open_file(&dir.join("mover.mdl")).unwrap();
    let mut stage = Stage::new(gpu);
    let base = stage.add_model(&lib, "mover", Mat4::from_translation(Vec3::X * 10.0)).unwrap();
    let hook = stage.actor(base).unwrap().model.model.node("hook");
    let rider = lib.model("rider").unwrap();
    let actor = stage.actor_for(
        &lib,
        "rider",
        rider,
        Placement::On { parent: base, node: hook, scale: 2.0, follow: true },
    );
    let rider = stage.add(actor);
    stage.step(&lib, 0.0);
    let w = stage.actor(rider).unwrap().world();
    assert!((w.w_axis.truncate() - Vec3::new(10.0, 0.0, 2.0)).length() < 1e-4, "{w}");
    assert!((w.x_axis.length() - 2.0).abs() < 1e-4);
    let (min, max) = stage.bounds().unwrap();
    assert!(min.x >= 10.0 - 1e-4 && max.x <= 12.0 + 1e-4, "{min} {max}");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn reload_keeps_the_animation() {
    let Some(gpu) = gpu() else { return };
    let dir = scratch("reload");
    std::fs::write(dir.join("mover.mdl"), MOVER).unwrap();
    let mut lib = Library::open(None).unwrap();
    lib.open_file(&dir.join("mover.mdl")).unwrap();
    let mut stage = Stage::new(gpu);
    let id = stage.add_model(&lib, "mover", Mat4::IDENTITY).unwrap();
    stage.actor_mut(id).unwrap().player.play(Some("rise"), PlayMode::Loop);
    stage.step(&lib, 0.25);
    assert_eq!(stage.reload(&lib), 0, "nothing changed");

    // An unsaved edit adds a node.
    let key = mg_resman::ResKey::parse("mover", mg_core::ResType::MDL).unwrap();
    let edited = MOVER.replace(
        "endmodelgeom mover",
        "node dummy extra\n  parent mover\nendnode\nendmodelgeom mover",
    );
    lib.set_buffer(key, Some(edited.into_bytes().into()));
    assert_eq!(stage.reload(&lib), 1);
    let a = stage.actor(id).unwrap();
    assert!(a.model.model.node("extra").is_some());
    assert_eq!(a.player.animation.as_deref(), Some("rise"));
    assert!((a.player.time - 0.25).abs() < 1e-6);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn renders_offscreen() {
    let Some(gpu) = gpu() else { return };
    let dir = scratch("render");
    std::fs::write(dir.join("mover.mdl"), MOVER).unwrap();
    let mut lib = Library::open(None).unwrap();
    lib.open_file(&dir.join("mover.mdl")).unwrap();
    let mut stage = Stage::new(gpu.clone());
    stage.add_model(&lib, "mover", Mat4::IDENTITY).unwrap();
    stage.settle(&lib, 0.0, 30.0);
    let mut cam = OrbitCamera::default();
    cam.set_view(View::Top);
    let (min, max) = stage.bounds().unwrap();
    cam.frame(min, max);
    let mut vp = Viewport::new(&gpu);
    let camera = cam.camera();
    let scene = stage.scene(camera.view());
    let img = vp.image(&gpu, lib.resman(), &scene, &camera, (64, 64));
    assert_eq!((img.width, img.height), (64, 64));
    // The triangle covers a fifth of the frame (framed by its bounding
    // sphere); the rest is background.
    let bg = stage.lighting.background.map(|c| (c * 255.0).round() as i32);
    let lit = img
        .data
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| (0..3).any(|i| (i32::from(p[i]) - bg[i]).abs() > 20))
        .count();
    let share = lit as f32 / (64.0 * 64.0);
    assert!((0.1..0.6).contains(&share), "{share}");
    let png = mgv_stage::render::png_bytes(&img);
    assert_eq!(&png[1..4], b"PNG");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A game placeable with emitters and an explosion: particles appear once
/// it has run a while, and two runs give the same image.
#[test]
fn game_emitters_render_the_same_twice() {
    let root = mg_testkit::corpus!();
    let Some(gpu) = gpu() else { return };
    let lib = Library::open(Some(GameInstall::new(root, None, "en"))).unwrap();
    let render = || {
        let mut stage = Stage::new(gpu.clone());
        let id = stage.add_model(&lib, "plc_a01", Mat4::IDENTITY).unwrap();
        let anims: Vec<String> =
            stage.actor(id).unwrap().animations.names().map(str::to_string).collect();
        assert!(!anims.is_empty());
        stage.settle(&lib, 1.0, 30.0);
        let mut cam = OrbitCamera::default();
        let (min, max) = stage.bounds().unwrap();
        cam.frame(min, max);
        let camera = cam.camera();
        let scene = stage.scene(camera.view());
        let mut vp = Viewport::new(&gpu);
        vp.image(&gpu, lib.resman(), &scene, &camera, (128, 128))
    };
    let (a, b) = (render(), render());
    assert!(a.data == b.data, "deterministic");
}

/// Every `visualeffects.2da` row with a model applies to a stand-in human
/// and steps a second without error; most models are found.
#[test]
fn every_visual_effect_applies() {
    let root = mg_testkit::corpus!();
    let Some(gpu) = gpu() else { return };
    let lib = Library::open(Some(GameInstall::new(root, None, "en"))).unwrap();
    let effects = mgv_stage::vfx::table(&lib);
    assert!(effects.len() > 400, "{} effects", effects.len());
    let mut stage = Stage::new(gpu);
    let look = mgv_stage::subject::CreatureLook::new(6);
    let mut missing = Vec::new();
    for chunk in effects.chunks(40) {
        let shown = mgv_stage::subject::show_creature(&mut stage, &lib, &look).unwrap();
        for e in chunk {
            for size in [1, 3, 5] {
                let applied = mgv_stage::vfx::apply(&mut stage, &lib, shown.base, e, size).unwrap();
                missing.extend(applied.missing.iter().map(|m| format!("{}: {m}", e.label)));
            }
        }
        stage.settle(&lib, 1.0, 10.0);
        assert!(stage.bounds_with_particles().is_some());
    }
    missing.sort();
    missing.dedup();
    eprintln!("{} effects; models not found: {missing:?}", effects.len());
    assert!(missing.len() < 20, "{} missing", missing.len());
}
