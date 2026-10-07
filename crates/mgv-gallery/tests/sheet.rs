//! Contact sheets rendered from the game's models.

use mg_resman::GameInstall;
use mgv_gallery::sheet::{Options, render};
use mgv_gallery::{Item, What};
use mgv_library::Library;
use mgv_stage::headless::Shot;

fn item(name: &str) -> Item {
    Item { id: name.into(), label: name.into(), group: None, what: What::Model(name.into()) }
}

/// A tile per item with a strip under it; what cannot be shown keeps its
/// tile and is reported.
#[test]
fn a_sheet_has_a_tile_per_item() {
    let root = mg_testkit::corpus!();
    mg_testkit::gpu::hold();
    let Some(gpu) = mg_render::Gpu::headless() else {
        eprintln!("skipped: no GPU adapter");
        return;
    };
    let mut lib = Library::open(Some(GameInstall::new(root, None, "en"))).unwrap();
    let items = [item("plc_a01"), item("c_golemerald"), item("no_such_model")];
    let shot = Shot { size: (96, 96), ..Default::default() };
    let labelled = Options { shot: shot.clone(), columns: 2, labels: true };
    let sheet = render(&mut lib, &gpu, &items, &labelled, &mut |_, _, _| {});
    assert_eq!(sheet.image.width, 192);
    assert!(sheet.image.height > 192, "two rows and their strips: {}", sheet.image.height);
    assert_eq!(sheet.failed.len(), 1);
    assert_eq!(sheet.failed[0].0, "no_such_model");
    let again = render(&mut lib, &gpu, &items, &labelled, &mut |_, _, _| {});
    assert_eq!(again.image.data, sheet.image.data, "the same bytes for the same input");
    let bare = Options { shot, columns: 0, labels: false };
    let plain = render(&mut lib, &gpu, &items, &bare, &mut |_, _, _| {});
    assert_eq!((plain.image.width, plain.image.height), (192, 192), "as square as three make");
}
