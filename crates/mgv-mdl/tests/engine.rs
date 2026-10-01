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
        let ascii = mgv_mdl::decompile(&data).unwrap();
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
                if !d.is_empty() {
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
