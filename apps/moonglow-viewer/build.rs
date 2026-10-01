//! Windows: the icon and version information in the executable.

fn main() {
    println!("cargo:rerun-if-changed=../../packaging/icons/moonglow-viewer.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("../../packaging/icons/moonglow-viewer.ico")
        .set("ProductName", "Moonglow Viewer")
        .set("FileDescription", "Moonglow Viewer")
        .set("LegalCopyright", "GPL-3.0-only");
    if let Err(e) = res.compile() {
        println!("cargo:warning=no Windows resources (icon, version): {e}");
    }
}
