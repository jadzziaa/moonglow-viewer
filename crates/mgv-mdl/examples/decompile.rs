//! Decompiles a game model (by name) or a binary .mdl file to stdout:
//! `cargo run -p mgv-mdl --example decompile -- NAME|FILE`.

fn main() {
    let arg = std::env::args().nth(1).expect("a model name or file");
    let data = if arg.ends_with(".mdl") {
        std::fs::read(&arg).expect("readable file")
    } else {
        let root = mg_testkit::nwn_root().expect("no game install");
        let rm = mg_resman::ResMan::for_game(&mg_resman::GameInstall::new(root, None, "en"))
            .expect("game resources");
        rm.get_named(&arg, mg_core::ResType::MDL).expect("model").into_owned()
    };
    match mg_mdl::decompile(&data) {
        Ok(text) => print!("{text}"),
        Err(e) => eprintln!("{arg}: {e}"),
    }
}
