//! Where resources come from: `cargo run -p mgv-library --example which -- NAME.EXT…`.

fn main() {
    let lib = mgv_library::Library::detect();
    for a in std::env::args().skip(1) {
        let key = mg_resman::ResKey::from_filename(&a);
        let origin = key.and_then(|k| lib.origin(&k).map(str::to_string));
        println!("{a}: {key:?} from {origin:?}");
    }
}
