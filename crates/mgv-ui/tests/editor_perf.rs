//! The ASCII editor on the game's largest models: frame times when idle
//! and per keystroke (printed; run by hand with `-- --ignored --nocapture`).

use std::time::Instant;

use egui_kittest::Harness;
use mg_core::ResType;
use mg_resman::ResKey;
use mgv_ui::{Action, NoDialogs, Settings, Tab, Viewer};

#[test]
#[ignore]
fn large_models() {
    mg_testkit::gpu::hold();
    let root = mg_testkit::corpus!();
    let settings = Settings { game_root: Some(root), use_user_dir: false, ..Settings::default() };
    let v = Viewer::new(settings, Box::new(NoDialogs));
    let mut h = Harness::builder()
        .with_size(egui::vec2(1400.0, 900.0))
        .build_ui_state(|ui, v: &mut Viewer| v.ui(ui), v);
    for name in ["plc_a01", "c_wererat", "a_ba"] {
        // No GPU here: the editor alone.
        let key = ResKey::parse(name, ResType::MDL).unwrap();
        let data = h.state().lib.get(&key).unwrap().into_owned();
        let text = mgv_mdl::decompile(&data).unwrap();
        let lines = text.lines().count();
        h.state_mut().buffer = Some(mgv_ui::Buffer::new(text, None, None, true));
        h.state_mut().focus(Tab::Editor);
        let t = Instant::now();
        h.step();
        let first = t.elapsed();
        let t = Instant::now();
        for _ in 0..5 {
            h.step();
        }
        let idle = t.elapsed() / 5;
        // Keystrokes, each within the pause before the view reloads.
        let t = Instant::now();
        for i in 0..5 {
            let now = h.ctx.input(|i| i.time);
            let b = h.state_mut().buffer.as_mut().unwrap();
            b.editor.cursor = mgv_ui::code::Pos::new(40 + i, 2);
            b.editor.insert("x");
            b.edited(now + 10.0);
            h.step();
        }
        let key_frame = t.elapsed() / 5;
        // The reload after the pause (text, diagnostics, outline; no GPU
        // here, so not the model upload).
        let t = Instant::now();
        h.state_mut().apply_buffer();
        let apply = t.elapsed();
        eprintln!(
            "{name}: {lines} lines; first frame {first:?}, idle {idle:?}, keystroke {key_frame:?}, \
             reload after the pause {apply:?}"
        );
        let _ = Action::Frame;
    }
}
