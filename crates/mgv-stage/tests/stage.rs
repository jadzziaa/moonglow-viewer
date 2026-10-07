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

#[test]
fn a_new_animation_blends_in_over_its_transtime() {
    let Some(gpu) = gpu() else { return };
    let dir = scratch("transition");
    let two = MOVER.replace("donemodel mover", "")
        + "newanim lift mover\n  length 1\n  transtime 0.5\n\
  node dummy mover\n    parent NULL\n  endnode\n\
  node trimesh tri\n    parent mover\n    positionkey\n      0 0 0 2\n      1 0 0 2\n    endlist\n  endnode\n\
doneanim lift mover\ndonemodel mover\n";
    std::fs::write(dir.join("mover.mdl"), two).unwrap();
    let mut lib = Library::open(None).unwrap();
    lib.open_file(&dir.join("mover.mdl")).unwrap();
    let mut stage = Stage::new(gpu);
    let id = stage.add_model(&lib, "mover", Mat4::IDENTITY).unwrap();
    let tri = stage.actor(id).unwrap().model.model.node("tri").unwrap();
    let height = |s: &Stage| s.actor(id).unwrap().pose()[tri].w_axis.z;
    stage.actor_mut(id).unwrap().player.play(Some("rise"), PlayMode::Once);
    stage.step(&lib, 0.0);
    stage.step(&lib, 2.0);
    assert!((height(&stage) - 1.0).abs() < 1e-3, "{}", height(&stage));
    // From 1 m to lift's 2 m over half a second.
    stage.actor_mut(id).unwrap().player.play(Some("lift"), PlayMode::Loop);
    stage.step(&lib, 0.25);
    assert!((height(&stage) - 1.5).abs() < 1e-3, "halfway: {}", height(&stage));
    stage.step(&lib, 0.5);
    assert!((height(&stage) - 2.0).abs() < 1e-3, "{}", height(&stage));
    // No transtime: at once.
    stage.actor_mut(id).unwrap().player.play(Some("rise"), PlayMode::Once);
    stage.step(&lib, 0.25);
    assert!((height(&stage) - 0.25).abs() < 1e-3, "{}", height(&stage));
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Overlays are tested against the scene's depths: hidden parts faint,
/// others full, x-ray ones over everything.
#[test]
fn overlays_show_faintly_where_the_scene_hides_them() {
    use mgv_stage::overlay::{HIDDEN_ALPHA, Overlay};
    let Some(gpu) = gpu() else { return };
    let dir = scratch("overlay");
    std::fs::write(dir.join("mover.mdl"), MOVER).unwrap();
    let mut lib = Library::open(None).unwrap();
    lib.open_file(&dir.join("mover.mdl")).unwrap();
    let mut stage = Stage::new(gpu.clone());
    stage.add_model(&lib, "mover", Mat4::IDENTITY).unwrap();
    stage.settle(&lib, 0.0, 30.0);
    stage.lighting.background = [0.0; 3];
    let mut cam = OrbitCamera::default();
    cam.set_view(View::Top);
    cam.frame(Vec3::new(-1.0, -1.0, 0.0), Vec3::new(2.0, 2.0, 0.0));
    let camera = cam.camera();
    let mut vp = Viewport::new(&gpu);
    let size = 128;
    vp.draw(&gpu, lib.resman(), &stage.scene(camera.view()), &camera, (size, size));
    // A green band under the triangle (z −0.5) across it, and a red one
    // over everything at y 0.6.
    let band = |o: &mut Overlay, y: f32, z: f32, color: [f32; 4], xray: bool| {
        let (a, b, c, d) = (
            Vec3::new(-1.0, y - 0.05, z),
            Vec3::new(2.0, y - 0.05, z),
            Vec3::new(2.0, y + 0.05, z),
            Vec3::new(-1.0, y + 0.05, z),
        );
        if xray {
            for k in -4..=4 {
                let dy = k as f32 * 0.01;
                o.xray_line(a + Vec3::Y * (0.05 + dy), b + Vec3::Y * (0.05 + dy), color);
            }
        } else {
            o.triangle(a, b, c, color);
            o.triangle(a, c, d, color);
        }
    };
    let mut overlay = Overlay::default();
    band(&mut overlay, 0.2, -0.5, [0.0, 1.0, 0.0, 1.0], false);
    band(&mut overlay, 0.6, 0.5, [1.0, 0.0, 0.0, 1.0], true);
    // The grid's kind: a blue line under the triangle at y 0.3, hidden there.
    for k in -4..=4 {
        let y = 0.3 + k as f32 * 0.01;
        overlay.culled_line(
            Vec3::new(-1.0, y, -0.5),
            Vec3::new(2.0, y, -0.5),
            [0.0, 0.0, 1.0, 1.0],
        );
    }
    vp.draw_overlay(&gpu, &overlay, &camera);
    let img = gpu.read_rgba(&vp.current().unwrap().color);
    let view_proj = camera.projection(1.0) * camera.view();
    let px = |p: Vec3| {
        let n = view_proj.project_point3(p);
        let (x, y) =
            (((n.x + 1.0) * 0.5 * size as f32) as u32, ((1.0 - n.y) * 0.5 * size as f32) as u32);
        let i = ((y * size + x) * 4) as usize;
        [img.data[i], img.data[i + 1], img.data[i + 2]]
    };
    let outside = px(Vec3::new(-0.5, 0.2, -0.5));
    let under = px(Vec3::new(0.3, 0.2, -0.5));
    let tri = px(Vec3::new(0.3, 0.4, 0.0));
    eprintln!("band outside {outside:?}, under the triangle {under:?}, triangle {tri:?}");
    assert_eq!(outside, [0, 255, 0], "full where nothing hides it");
    let faint = |c: u8, over: u8| {
        (f32::from(over) * (1.0 - HIDDEN_ALPHA) + f32::from(c) * HIDDEN_ALPHA).round()
    };
    assert!((f32::from(under[1]) - faint(255, tri[1])).abs() <= 2.0, "faint under the triangle");
    assert!((f32::from(under[0]) - faint(0, tri[0])).abs() <= 2.0);
    assert_eq!(px(Vec3::new(0.2, 0.6, 0.0))[0], 255, "x-ray over the triangle");
    assert_eq!(px(Vec3::new(-0.5, 0.3, -0.5)), [0, 0, 255], "culled lines show in the open");
    assert_eq!(px(Vec3::new(0.3, 0.3, -0.5)), tri, "and not at all behind the model");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Views are taken from the model's front: placeables face −Y (their doors,
/// locks, seats and glass are there), creatures +Y.
#[test]
fn views_are_taken_from_the_models_front() {
    use mg_core::ResType;
    use mgv_stage::headless::Shot;
    use mgv_stage::subject;
    let root = mg_testkit::corpus!();
    let Some(gpu) = gpu() else { return };
    let lib = Library::open(Some(GameInstall::new(root, None, "en"))).unwrap();
    let mut stage = Stage::new(gpu);
    let mut front = |name: &str| {
        let key = mg_resman::ResKey::parse(name, ResType::MDL).unwrap();
        subject::show(&mut stage, &lib, &lib.open_resource(key).unwrap()).unwrap();
        let eye = Shot::default().camera(&stage).camera().eye;
        (stage.front.to_degrees().round(), eye)
    };
    let (armoire, eye) = front("plc_a01");
    assert_eq!(armoire, -90.0);
    assert!(eye.y < 0.0, "the default view shows its doors: {eye}");
    // A standing mirror's glass faces −Y (its use point is behind it).
    assert_eq!(front("px2_d04").0, -90.0);
    assert_eq!(front("c_badger").0, 90.0);
    assert_eq!(front("tcn01_a01_01").0, 90.0, "tiles as they are");
}

/// A 2 m panel facing −Y with no ambient colour (as many exported models
/// have): out of the sun it is black whatever the area's ambient light.
const PANEL: &str = "newmodel panel\nsetsupermodel panel NULL\nclassification character\n\
beginmodelgeom panel\n\
node dummy panel\n  parent NULL\nendnode\n\
node trimesh face\n  parent panel\n  ambient 0 0 0\n  diffuse 1 1 1\n  verts 4\n\
    -1 0 0\n    1 0 0\n    1 0 2\n    -1 0 2\n\
  faces 2\n    0 1 2 1 0 0 0 0\n    0 2 3 1 0 0 0 0\nendnode\n\
endmodelgeom panel\n\
donemodel panel\n";

/// The key light lights what the camera sees: a panel with no ambient
/// colour, facing away from the studio sun, is black without it and lit
/// with it.
#[test]
fn the_key_light_lights_the_side_in_view() {
    use mgv_stage::headless::{self, Shot};
    let Some(gpu) = gpu() else { return };
    let dir = scratch("keylight");
    std::fs::write(dir.join("panel.mdl"), PANEL).unwrap();
    let mut lib = Library::open(None).unwrap();
    lib.open_file(&dir.join("panel.mdl")).unwrap();
    let mut stage = Stage::new(gpu.clone());
    stage.add_model(&lib, "panel", Mat4::IDENTITY).unwrap();
    let mut vp = Viewport::new(&gpu);
    // From −Y, level: the panel fills the middle of the picture; the sun
    // shines on its back.
    let mut centre = |key_light: f32| {
        let shot = Shot {
            size: (64, 64),
            yaw: Some(-90.0),
            pitch: Some(0.0),
            background: Some([0.5, 0.5, 0.5]),
            key_light,
            ..Shot::default()
        };
        let img = headless::still(&mut stage, &mut vp, &lib, &shot);
        let p = &img.data[(32 * 64 + 32) * 4..][..3];
        p.iter().map(|&c| u32::from(c)).sum::<u32>() / 3
    };
    let dark = centre(0.0);
    let lit = centre(0.3);
    assert!(dark < 20, "out of the sun, no ambient: black ({dark})");
    assert!(lit > 100, "the key light reaches it ({lit})");
    assert!(centre(0.6) > lit, "stronger with more");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// An object with PLT colours takes the PLT of a name that is also a TGA:
/// a dwarf's head is coloured by its skin, not the TGA's gray.
#[test]
fn coloured_parts_take_the_plt() {
    use mg_render::{Assets, colored_name};
    let root = mg_testkit::corpus!();
    let lib = Library::open(Some(GameInstall::new(root, None, "en"))).unwrap();
    let texture = |name: &str| Assets::texture(lib.resman(), name);
    let pixels = |colors: [u8; 10]| {
        let t = texture(&colored_name("pmd0_head001", colors)).unwrap();
        t.texture.to_rgba().data
    };
    assert_ne!(pixels([0; 10]), pixels([20; 10]), "the skin colour shows");
    // Without colours, the toolset's lookup as before (the TGA).
    assert!(texture("pmd0_head001").is_some());
}

/// A body part whose texture names nothing takes the texture of its own
/// name; names that resolve, and models that are not body parts, are left.
#[test]
fn parts_fall_back_to_their_own_names_texture() {
    use mgv_stage::textures::part_fallbacks;
    let root = mg_testkit::corpus!();
    let lib = Library::open(Some(GameInstall::new(root, None, "en"))).unwrap();
    let of = |name: &str| part_fallbacks(lib.resman(), &lib.model(name).unwrap(), name);
    // pfh0_belt063 names `beltmerged`, which does not exist.
    let belt = of("pfh0_belt063");
    assert_eq!(belt.get("beltmerged").map(String::as_str), Some("pfh0_belt063"), "{belt:?}");
    // A dwarf's hand names a human's texture, which exists: kept.
    assert!(of("pmd0_handl001").is_empty());
    assert!(of("plc_a01").is_empty());
}

/// A PLT on a model shows its colors: a base model's stand-in body does,
/// a placeable has none.
#[test]
fn plt_models_are_told_apart_and_take_colors() {
    use mgv_stage::textures::uses_plt;
    let root = mg_testkit::corpus!();
    let mut lib = Library::open(Some(GameInstall::new(root, None, "en"))).unwrap();
    let of = |name: &str| uses_plt(lib.resman(), &lib.model(name).unwrap(), None);
    assert!(of("a_halforc"));
    assert!(!of("plc_a01"));
    // The belt's own texture, a PLT, through its replacement only.
    let belt = lib.model("pfh0_belt063").unwrap();
    assert!(!uses_plt(lib.resman(), &belt, None));
    let renamed = mgv_stage::textures::part_fallbacks(lib.resman(), &belt, "pfh0_belt063");
    assert!(uses_plt(lib.resman(), &belt, Some(&renamed)));

    let Some(gpu) = gpu() else { return };
    let mut stage = Stage::new(gpu.clone());
    let mut viewport = Viewport::new(&gpu);
    let opened = lib.open_input("a_halforc").unwrap();
    let shown = mgv_stage::subject::show(&mut stage, &lib, &opened).unwrap();
    let shot = mgv_stage::headless::Shot { size: (128, 128), ..Default::default() };
    let plain = mgv_stage::headless::still(&mut stage, &mut viewport, &lib, &shot);
    stage.actor_mut(shown.base).unwrap().look.colors = Some([60; 10]);
    let colored = mgv_stage::headless::still(&mut stage, &mut viewport, &lib, &shot);
    assert_ne!(plain.data, colored.data, "the colors show");
    // A shot's colors are for what has none: the same picture from them,
    // and what has its own keeps them.
    let with = mgv_stage::headless::Shot { plt_colors: Some([60; 10]), ..shot.clone() };
    stage.actor_mut(shown.base).unwrap().look.colors = None;
    let by_shot = mgv_stage::headless::still(&mut stage, &mut viewport, &lib, &with);
    assert_eq!(by_shot.data, colored.data);
    stage.actor_mut(shown.base).unwrap().look.colors = Some([0; 10]);
    let own = mgv_stage::headless::still(&mut stage, &mut viewport, &lib, &shot);
    let own_with = mgv_stage::headless::still(&mut stage, &mut viewport, &lib, &with);
    assert_eq!(own_with.data, own.data, "its own colors stay");
    assert_ne!(own.data, colored.data);
}
