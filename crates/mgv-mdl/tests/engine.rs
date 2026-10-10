//! The game's own compiler on the native decompiler's ASCII (L3): each
//! sampled model compiles in the game and reads back as the original.
//!
//! The game runs only in the toolset's sandbox (bubblewrap, a scratch user
//! folder, no network, no Steam) on an off-screen display, never the
//! desktop:
//!
//!   DISPLAY=$(~/Projects/moonglow-toolset/tools/aurora/headless.sh start) \
//!     cargo test -p mgv-mdl --test engine -- --ignored --nocapture
//!
//! (`MOONGLOW_TOOLSET` names the toolset checkout if it is elsewhere.)

mod common;

use std::path::PathBuf;

use mg_core::ResType;
use mg_mdl::Model;
use mg_resman::{GameInstall, ResKey, ResMan};
use mgv_mdl::tools::EngineCompiler;

#[test]
#[ignore]
fn the_games_compiler_takes_native_ascii() {
    let root = mg_testkit::corpus!();
    let display = std::env::var("DISPLAY").unwrap_or_default();
    assert!(!display.is_empty() && display != ":0", "an off-screen DISPLAY, never the desktop");
    let toolset = std::env::var_os("MOONGLOW_TOOLSET").map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(std::env::var_os("HOME").unwrap()).join("Projects/moonglow-toolset")
    });
    let launcher = toolset.join("tools/nwclient/run-client.sh");
    assert!(launcher.is_file(), "{}", launcher.display());
    let scratch =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/test-output/engine-compile");
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).unwrap();
    let scratch = scratch.canonicalize().unwrap();
    let rm = ResMan::for_game(&GameInstall::new(&root, None, "en")).unwrap();
    let mut compiler = EngineCompiler::new(&root, scratch.join("user"), None).unwrap();
    compiler.launcher = Some((launcher, vec![scratch.to_string_lossy().into_owned()]));
    compiler.timeout = std::time::Duration::from_secs(90);
    let mut failed = Vec::new();
    // plc_k01 keys self-illumination in its animations.
    for name in ["plc_a01", "tcn01_a01_01", "t_door09", "vim_magknock", "plc_k01"] {
        let data = rm.get(&ResKey::parse(name, ResType::MDL).unwrap()).unwrap();
        let original = Model::read(&data).unwrap();
        let ascii = mg_mdl::decompile(&data).unwrap();
        let start = std::time::Instant::now();
        match compiler.compile(&ascii, name) {
            Ok(binary) => {
                let back = Model::read(&binary).unwrap();
                let d = common::compare(&original, &back, false);
                eprintln!(
                    "{name}: compiled in {:.1} s, {} differences",
                    start.elapsed().as_secs_f32(),
                    d.len()
                );
                for x in d.iter().take(5) {
                    eprintln!("  {x}");
                }
                // The native compiler makes the same model of the text.
                let native = mg_mdl::compile::compile(ascii.as_bytes(), &Default::default())
                    .map(|c| Model::read(&c.binary).unwrap());
                let n = match &native {
                    Ok(m) => common::compare(&back, m, true),
                    Err(e) => vec![format!("not compiled natively: {e}")],
                };
                for x in n.iter().take(5) {
                    eprintln!("  native against the game's: {x}");
                }
                if !d.is_empty() || !n.is_empty() {
                    failed.push(name);
                }
            }
            Err(e) => {
                eprintln!("{name}: {e}");
                failed.push(name);
            }
        }
    }
    assert!(failed.is_empty(), "{failed:?}");
}

/// What the game's compiler keeps of EE fields reads back the same: an
/// authored model with `materialname`, `renderhint`, tangents and a Bézier
/// key, compiled in the game, then decompiled natively and compiled again.
#[test]
#[ignore]
fn ee_fields_survive_the_games_compiler() {
    let root = mg_testkit::corpus!();
    let display = std::env::var("DISPLAY").unwrap_or_default();
    assert!(!display.is_empty() && display != ":0", "an off-screen DISPLAY, never the desktop");
    let toolset = std::env::var_os("MOONGLOW_TOOLSET").map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(std::env::var_os("HOME").unwrap()).join("Projects/moonglow-toolset")
    });
    let scratch =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/test-output/engine-ee");
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).unwrap();
    let scratch = scratch.canonicalize().unwrap();
    let mut compiler = EngineCompiler::new(&root, scratch.join("user"), None).unwrap();
    compiler.launcher = Some((
        toolset.join("tools/nwclient/run-client.sh"),
        vec![scratch.to_string_lossy().into_owned()],
    ));
    compiler.timeout = std::time::Duration::from_secs(90);
    let ascii = "newmodel mgvee\nsetsupermodel mgvee NULL\nclassification Character\n\
        beginmodelgeom mgvee\nnode dummy mgvee\n  parent NULL\nendnode\n\
        node trimesh quad\n  parent mgvee\n  bitmap mgvtex\n  texture1 mgvtex_n\n\
          materialname mgvmat\n  renderhint NormalAndSpecMapped\n\
          verts 4\n    0 0 0\n    1 0 0\n    1 1 0\n    0 1 0\n\
          tverts 4\n    0 0 0\n    1 0 0\n    1 1 0\n    0 1 0\n\
          tangents 4\n    0 1 0 -1\n    0 1 0 -1\n    0 1 0 -1\n    0 1 0 -1\n\
          faces 2\n    0 1 2 1 0 1 2 1\n    0 2 3 1 0 2 3 1\nendnode\n\
        node emitter em\n  parent mgvee\n  update Fountain\n  render Normal\n  blend Normal\n\
          texture mgvtex\n  spawntype -1\nendnode\nendmodelgeom mgvee\n\
        newanim go mgvee\n  length 2\n  transtime 0.25\n  animroot mgvee\n\
          node dummy mgvee\n    parent NULL\n  endnode\n\
          node trimesh quad\n    parent mgvee\n    positionbezierkey 2\n\
            0 0 0 0 0.1 0.2 0.3 0.4 0.5 0.6\n      2 1 1 1 0.7 0.8 0.9 1.1 1.2 1.3\n    endlist\n\
          endnode\ndoneanim go mgvee\ndonemodel mgvee\n";
    let first = compiler.compile(ascii, "mgvee").expect("the game compiles it");
    let check = |binary: &[u8], what: &str| {
        let m = Model::read(binary).unwrap();
        let quad = m.nodes[m.node("quad").unwrap()].mesh().unwrap();
        assert_eq!(quad.material.as_deref(), Some("mgvmat"), "{what}");
        assert_eq!(quad.renderhint.as_deref(), Some("normalandspecmapped"), "{what}");
        assert_eq!(quad.tangents, [[0.0, 1.0, 0.0, -1.0]; 4], "{what}");
        let key = &m.animation("go").unwrap().nodes[1].controllers[0];
        assert!(key.is_bezier(), "{what}");
        assert_eq!(key.handles[6..], [0.7, 0.8, 0.9, 1.1, 1.2, 1.3], "{what}");
        let mg_mdl::NodeKind::Emitter(e) = &m.nodes[m.node("em").unwrap()].kind else {
            panic!("{what}: emitter")
        };
        assert_eq!(e.spawntype, u32::MAX, "{what}");
        m
    };
    let original = check(&first, "compiled");
    // Decompiled natively and compiled again: the same model.
    let again = compiler.compile(&mg_mdl::decompile(&first).unwrap(), "mgvee").unwrap();
    let back = check(&again, "decompiled and compiled again");
    let d = common::compare(&original, &back, false);
    assert!(d.is_empty(), "{d:?}");
}

/// What the native compiler derives against what the game's compiler
/// does, on a text that leaves it to the compiler: normals from smoothing
/// groups (a cube, one group for its sides and none for its top) and
/// tangents for a render hint.
#[test]
#[ignore]
fn the_native_compiler_derives_what_the_games_does() {
    let root = mg_testkit::corpus!();
    let display = std::env::var("DISPLAY").unwrap_or_default();
    assert!(!display.is_empty() && display != ":0", "an off-screen DISPLAY, never the desktop");
    let toolset = std::env::var_os("MOONGLOW_TOOLSET").map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(std::env::var_os("HOME").unwrap()).join("Projects/moonglow-toolset")
    });
    let launcher = toolset.join("tools/nwclient/run-client.sh");
    let scratch =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/test-output/engine-derive");
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).unwrap();
    let scratch = scratch.canonicalize().unwrap();
    let mut compiler = EngineCompiler::new(&root, scratch.join("user"), None).unwrap();
    compiler.launcher = Some((launcher, vec![scratch.to_string_lossy().into_owned()]));
    compiler.timeout = std::time::Duration::from_secs(90);
    // A cube: its four sides share smoothing group 1 (rounded corners),
    // its top and bottom have groups of their own (flat).
    let verts = "0 0 0\n1 0 0\n1 1 0\n0 1 0\n0 0 1\n1 0 1\n1 1 1\n0 1 1";
    let tverts = "0 0 0\n1 0 0\n1 1 0\n0 1 0";
    let faces = "0 1 5 1 0 1 2 0\n0 5 4 1 0 2 3 0\n1 2 6 1 0 1 2 0\n1 6 5 1 0 2 3 0\n\
                 2 3 7 1 0 1 2 0\n2 7 6 1 0 2 3 0\n3 0 4 1 0 1 2 0\n3 4 7 1 0 2 3 0\n\
                 4 5 6 2 0 1 2 0\n4 6 7 2 0 2 3 0\n0 2 1 4 0 2 1 0\n0 3 2 4 0 3 2 0";
    let text = format!(
        "newmodel zz_mgvderive\nsetsupermodel zz_mgvderive NULL\nclassification character\n\
         setanimationscale 1\nbeginmodelgeom zz_mgvderive\n\
         node dummy zz_mgvderive\n  parent NULL\nendnode\n\
         node trimesh cube\n  parent zz_mgvderive\n  bitmap wood\n  \
         renderhint NormalAndSpecMapped\n  verts 8\n{verts}\n  tverts 4\n{tverts}\n  \
         faces 12\n{faces}\nendnode\n\
         endmodelgeom zz_mgvderive\ndonemodel zz_mgvderive\n"
    );
    let theirs = Model::read(&compiler.compile(&text, "zz_mgvderive").unwrap()).unwrap();
    let ours = Model::read(
        &mg_mdl::compile::compile(text.as_bytes(), &Default::default()).unwrap().binary,
    )
    .unwrap();
    let (a, b) = (theirs.nodes[1].mesh().unwrap(), ours.nodes[1].mesh().unwrap());
    eprintln!(
        "vertices: the game's {} ours {}; tangents {} and {}",
        a.vertices.len(),
        b.vertices.len(),
        a.tangents.len(),
        b.tangents.len()
    );
    eprintln!("shininess: the game's {} ours {}", a.shininess, b.shininess);
    let d = common::compare(&theirs, &ours, true);
    for x in d.iter().take(12) {
        eprintln!("  {x}");
    }
    // Corner by corner: normals and tangents as the game's.
    let (mut normal, mut tangent, mut corners) = (0.0f32, 0.0f32, 0);
    for (fa, fb) in a.faces.iter().zip(&b.faces) {
        for c in 0..3 {
            let (va, vb) = (fa.vertices[c] as usize, fb.vertices[c] as usize);
            let dist = |x: [f32; 3], y: [f32; 3]| {
                (0..3).map(|k| (x[k] - y[k]).powi(2)).sum::<f32>().sqrt()
            };
            normal = normal.max(dist(a.normals[va], b.normals[vb]));
            if let (Some(ta), Some(tb)) = (a.tangents.get(va), b.tangents.get(vb)) {
                tangent = tangent.max(dist([ta[0], ta[1], ta[2]], [tb[0], tb[1], tb[2]]));
                tangent = tangent.max((ta[3] - tb[3]).abs());
            }
            corners += 1;
        }
    }
    eprintln!("{corners} corners: normals off by at most {normal:.4}, tangents by {tangent:.4}");
    assert!(d.is_empty(), "{d:?}");
    assert_eq!(a.faces.len(), b.faces.len());
    assert!(normal < 0.02, "normals differ by {normal}");
    assert!(a.tangents.is_empty() || tangent < 0.02, "tangents differ by {tangent}");
}
