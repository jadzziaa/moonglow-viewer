//! Gallery mode (the UI redesign's M6, part C; proposal section 9.17 and
//! the boards "Resources as a Gallery" and "Gallery Tab at 3440×1440"):
//! the Resources panel's list as pictures in the kit's cells
//! (`mg_uikit::gallery`), in the panel or as the editor area's Gallery tab.
//!
//! Only the rows in sight are laid out and drawn (the kit's
//! [`GalleryGrid`]), and only their pictures and the next row's are asked
//! for. The pictures are made after the frame ([`Pictures::make`], over the
//! kit's `PaintedPictures`), a few at a time ([`PER_FRAME`], [`TIME`]), on
//! the caller's thread and the window's GPU device: a stage
//! and a viewport of their own on it, drawn as `mgv gallery` draws
//! ([`headless::thumbnail_shot`]), the game's programs never waited for. A
//! texture's picture is the texture itself, made on the CPU. Pictures are
//! kept by what they are of and let go of when the game's data is read
//! again ([`Pictures::forget`]).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use egui::{ColorImage, Ui};
use mg_core::ResType;
use mg_image::Rgba;
use mg_resman::ResKey;
use mg_uikit::Icon;
use mg_uikit::gallery::{self as kit, GalleryCell, GalleryGrid, GridLayout, Picture};
use mg_uikit::pictures::{
    Budget, Painted, PaintedPictures, Picture as Cached, PicturePainter, Retry,
};
use mgv_library::{Kind, Library};
use mgv_stage::headless;
use mgv_stage::subject::{self, CreatureLook};
use mgv_stage::{Stage, Viewport};

use crate::{Action, Viewer};

/// The Resources header's button that opens the Gallery tab.
pub const OPEN_AS_TAB: &str = "Open as Tab";

/// What a picture is of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Pictured {
    Resource(ResKey),
    /// An `appearance.2da` row, as a bare creature (the Creatures filter).
    Creature(u16),
}

impl Pictured {
    /// Whether it has a picture to make: models, blueprints, materials (a
    /// sphere wearing one), creatures, and textures (the texture itself).
    /// Walkmeshes, TXI and the rest show their kind's icon.
    pub fn has_picture(self) -> bool {
        match self {
            Pictured::Creature(_) => true,
            Pictured::Resource(k) => matches!(
                Kind::of(k.restype),
                Kind::Model | Kind::Blueprint | Kind::Material | Kind::Texture | Kind::Plt
            ),
        }
    }

    /// The kind's icon, shown while the picture is made and where there is
    /// none.
    pub fn icon(self) -> Icon {
        let Pictured::Resource(k) = self else { return Icon::Creature };
        match k.restype {
            ResType::UTC => Icon::Creature,
            ResType::UTP => Icon::Placeable,
            ResType::UTD => Icon::Door,
            ResType::UTI => Icon::Item,
            t => match Kind::of(t) {
                Kind::Model => Icon::Model,
                Kind::Texture | Kind::Plt => Icon::Texture,
                Kind::Walkmesh => Icon::Walkmesh,
                Kind::Material => Icon::Shaders,
                _ => Icon::File,
            },
        }
    }
}

/// The pictures' side in pixels (the toolset's thumbnails' 224).
pub const SIZE: u32 = 224;
/// The most pictures made in a frame, and how long they are made for: once
/// it has passed, the rest wait for the next frame (one is made in every
/// frame, however long it takes).
pub const PER_FRAME: usize = 8;
pub const TIME: Duration = Duration::from_millis(4);
/// The pictures kept at most of those not shown in this frame or the
/// last: past it, those longest unseen are let go (never one shown).
pub const KEPT: usize = 512;
/// How long a picture waits for the game's programs (a cold shader cache
/// compiles many) before it is taken as it is drawn (Moonglow's shader for
/// what still waits): the kit's `Retry::give_up`.
pub const WAIT_AT_MOST: Duration = Duration::from_secs(20);
/// How long a picture waiting for the game's programs waits before it is
/// drawn again: the kit's `Retry::again`.
pub const AGAIN: Duration = Duration::from_millis(50);

/// What makes the pictures: the window's GPU in the app
/// ([`ViewerPainter`]), anything at all in a test. The kit's
/// [`PicturePainter`] paints them, with the library of the frame
/// ([`Painting`]).
pub trait Painter {
    /// `what`'s picture, [`SIZE`] pixels at most a side; [`Painted::Later`]
    /// while a program of the game's it needs is being made. `as_drawn`:
    /// take it as it is drawn, whatever still waits.
    fn paint(&mut self, lib: &Library, what: Pictured, as_drawn: bool) -> Painted;
    /// The game's data was read again: let go of what was kept of it.
    fn forget(&mut self) {}
}

/// A [`Painter`] with the frame's library: the kit's [`PicturePainter`].
/// Once a picture waits for the game's programs, those after it in the
/// frame wait too without being drawn (they likely want the same
/// programs), unless asked for as drawn.
struct Painting<'a> {
    painter: &'a mut dyn Painter,
    lib: &'a Library,
    /// The failures' reasons, kept for [`Pictures::error`].
    reasons: &'a mut HashMap<Pictured, String>,
    /// Pictures painted in this frame.
    painted: usize,
    /// One painted in this frame waits for the game's programs.
    waiting: bool,
}

impl PicturePainter<Pictured> for Painting<'_> {
    fn paint(&mut self, what: Pictured, as_drawn: bool) -> Painted {
        if self.waiting && !as_drawn {
            return Painted::Later;
        }
        self.painted += 1;
        let painted = self.painter.paint(self.lib, what, as_drawn);
        match &painted {
            Painted::Later => self.waiting = true,
            Painted::Failed(reason) => {
                self.reasons.insert(what, reason.clone());
            }
            Painted::Ready(_) | Painted::Drawn(_) => {}
        }
        painted
    }
}

/// The galleries' pictures (the kit's [`PaintedPictures`]): asked for
/// while they are in sight ([`Pictures::picture`], [`Pictures::want`]),
/// made after the frame ([`Pictures::make`]).
pub struct Pictures {
    cache: PaintedPictures<Pictured>,
    /// Asked for in this frame, made or not (a cell in sight is in the
    /// grid's `ahead` too: asked once).
    asked: HashSet<Pictured>,
    /// Why the pictures made with none have none (the kit gives a reason
    /// only while asking for the picture).
    reasons: HashMap<Pictured, String>,
    /// The library's layers when the pictures were made.
    layers: u64,
    /// What makes them; made with the first picture asked for, on the
    /// window's GPU (set by a test to one of its own).
    pub painter: Option<Box<dyn Painter>>,
    /// The pictures made after the last frame (painted, or found none).
    pub painted: usize,
}

impl Default for Pictures {
    fn default() -> Pictures {
        let mut cache = PaintedPictures::default();
        cache.retry = Retry { again: AGAIN, give_up: WAIT_AT_MOST };
        Pictures {
            cache,
            asked: HashSet::new(),
            reasons: HashMap::new(),
            layers: 0,
            painter: None,
            painted: 0,
        }
    }
}

impl std::fmt::Debug for Pictures {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pictures").field("made", &self.kept()).finish_non_exhaustive()
    }
}

/// The library's layers and the folders over the game, as a number: when
/// it changes, what a name stands for may have changed.
fn layers_of(lib: &Library) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for l in lib.resman().layers() {
        l.label.hash(&mut h);
    }
    lib.opened_folder().hash(&mut h);
    h.finish()
}

impl Pictures {
    /// A new frame: nothing asked for yet. Where the library's layers
    /// changed (a folder opened, one added), every picture is let go.
    pub(crate) fn begin_frame(&mut self, lib: &Library) {
        self.cache.begin_frame();
        self.asked.clear();
        let layers = layers_of(lib);
        if layers != self.layers {
            self.layers = layers;
            self.forget();
        }
    }

    /// `what`'s picture as a cell shows it, asked for.
    pub fn picture(&mut self, what: Pictured) -> Picture {
        if !what.has_picture() {
            // (The kind's icon, dimmed, with no caption: nothing is made.)
            return Picture::Making;
        }
        self.asked.insert(what);
        match self.cache.get(what) {
            Cached::Ready(id) => Picture::Ready(id),
            Cached::Failed(_) => Picture::Missing,
            Cached::Waiting => Picture::Making,
        }
    }

    /// Why `what` has no picture, where it was made with none.
    pub fn error(&self, what: Pictured) -> Option<&str> {
        self.reasons.get(&what).filter(|_| self.is_made(what)).map(String::as_str)
    }

    /// Asks for `what`'s picture in this frame (a row about to come into
    /// sight).
    pub fn want(&mut self, what: Pictured) {
        if what.has_picture() && self.asked.insert(what) {
            let _ = self.cache.get(what);
        }
    }

    /// The pictures asked for in this frame, made or not.
    pub fn asked(&self) -> usize {
        self.asked.len()
    }

    /// The pictures asked for in this frame and still to make.
    pub fn waiting(&self) -> usize {
        self.cache.pictures.wanted().len()
    }

    /// The pictures kept.
    pub fn kept(&self) -> usize {
        self.cache.pictures.len()
    }

    /// Whether `what`'s picture is made (with something to draw or not).
    pub fn is_made(&self, what: Pictured) -> bool {
        self.cache.pictures.get(&what).is_some()
    }

    /// Lets go of every picture (the game's data read again: the same
    /// names may look different now).
    pub fn forget(&mut self) {
        // (Every texture is the kit's own: none of the painter's to free.)
        let _ = self.cache.forget();
        self.reasons.clear();
        if let Some(p) = &mut self.painter {
            p.forget();
        }
    }

    /// Lets go of one picture (its model edited).
    pub fn forget_one(&mut self, what: Pictured) {
        let _ = self.cache.forget_one(what);
        self.reasons.remove(&what);
    }

    /// After a frame: makes the pictures it asked for and lacks, for
    /// [`TIME`] and at most [`PER_FRAME`], and asks for another frame if
    /// there are more (after [`AGAIN`] where they wait for the game's
    /// programs). Past [`KEPT`], those longest unseen are let go of.
    /// `gpu`: the window's device (the painter's, made with the first
    /// picture).
    pub(crate) fn make(
        &mut self,
        ctx: &egui::Context,
        lib: &Library,
        gpu: Option<&mg_render::Gpu>,
    ) {
        if !self.cache.pictures.wanted().is_empty() && self.painter.is_none() {
            self.painter = Some(Box::new(ViewerPainter::new(gpu, ctx)));
        }
        let budget = Budget { time: TIME, most: PER_FRAME };
        let painted = match self.painter.as_deref_mut() {
            Some(painter) => {
                let reasons = &mut self.reasons;
                let mut painting = Painting { painter, lib, reasons, painted: 0, waiting: false };
                // (Every texture is the kit's own: none of the painter's
                // to free.)
                let _ = self.cache.pump(ctx, &mut painting, budget, KEPT);
                painting.painted
            }
            // (Nothing asked for, nothing to paint: the waits and the
            // pictures out of sight let go of all the same.)
            None => {
                let _ = self.cache.pump(ctx, &mut |_, _| Painted::Later, budget, KEPT);
                0
            }
        };
        self.painted = painted;
        let made = &self.cache.pictures;
        self.reasons.retain(|k, _| made.get(k).is_some());
    }
}

// ------------------------------------------------------------------ painting

/// The app's painter: models, blueprints, materials and creatures drawn
/// on a stage and a viewport of its own on the window's GPU device (as
/// `mgv gallery` draws them); textures read and shrunk on the CPU.
pub struct ViewerPainter {
    gpu: Option<(Stage, Viewport)>,
    /// The PLT palettes (read with the first PLT).
    palettes: Option<Vec<Option<Rgba>>>,
    /// Pictures drawn, counted: the viewport's textures are let go of
    /// every [`TEXTURES_KEPT_FOR`] (a gallery scrolled through would keep
    /// every texture of every model).
    drawn: usize,
}

/// How many pictures the painter's textures are kept for.
const TEXTURES_KEPT_FOR: usize = 64;

impl std::fmt::Debug for ViewerPainter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ViewerPainter").field("drawn", &self.drawn).finish_non_exhaustive()
    }
}

impl ViewerPainter {
    /// On the window's device `gpu` (none: textures alone have pictures);
    /// a frame is asked of `ctx` when a program of the game's is made.
    pub fn new(gpu: Option<&mg_render::Gpu>, ctx: &egui::Context) -> ViewerPainter {
        let gpu = gpu.map(|g| {
            let mut viewport = Viewport::new(g);
            let ctx = ctx.clone();
            viewport.renderer().set_game_waker(move || ctx.request_repaint());
            (Stage::new(g.clone()), viewport)
        });
        ViewerPainter { gpu, palettes: None, drawn: 0 }
    }

    /// `Ok(None)`: not yet, a program of the game's being made.
    fn model(
        &mut self,
        lib: &Library,
        what: Pictured,
        as_drawn: bool,
    ) -> Result<Option<ColorImage>, String> {
        let Some((stage, viewport)) = &mut self.gpu else {
            return Err("No GPU".into());
        };
        match what {
            Pictured::Resource(key) => {
                let opened = lib.open_resource(key).map_err(|e| e.to_string())?;
                subject::show(stage, lib, &opened).map_err(|e| e.to_string())?;
            }
            Pictured::Creature(row) => {
                let look = CreatureLook::new(row);
                subject::show_creature(stage, lib, &look).map_err(|e| e.to_string())?;
            }
        }
        self.drawn += 1;
        if self.drawn.is_multiple_of(TEXTURES_KEPT_FOR) {
            viewport.clear_textures();
        }
        // (As drawn, it is always read back: the kit's "Not drawn within
        // 20 s" never comes of it.)
        let shot = headless::thumbnail_shot(SIZE);
        match headless::still_unwaited(stage, viewport, lib, &shot, as_drawn) {
            Some(image) => drawn_picture(&image).map(Some),
            None => Ok(None),
        }
    }

    fn texture(&mut self, lib: &Library, key: ResKey) -> Result<ColorImage, String> {
        let data = lib.get(&key).ok_or_else(|| format!("{key} not found"))?;
        let image = if Kind::of(key.restype) == Kind::Plt {
            let plt = mg_image::plt::Plt::read(&data).map_err(|e| e.to_string())?;
            let palettes = self.palettes.get_or_insert_with(|| palettes(lib));
            let refs: [Option<&Rgba>; 10] =
                std::array::from_fn(|i| palettes.get(i).and_then(Option::as_ref));
            plt.colorize(&refs, [0; 10]).top_down()
        } else {
            texture_pixels(key.restype, &data)?
        };
        color_image(&shrink(&image, SIZE))
    }
}

impl Painter for ViewerPainter {
    fn paint(&mut self, lib: &Library, what: Pictured, as_drawn: bool) -> Painted {
        match what {
            Pictured::Resource(k) if matches!(Kind::of(k.restype), Kind::Texture | Kind::Plt) => {
                painted(self.texture(lib, k).map(Some))
            }
            _ => painted(self.model(lib, what, as_drawn)),
        }
    }

    fn forget(&mut self) {
        self.palettes = None;
        if let Some((stage, viewport)) = &mut self.gpu {
            stage.clear();
            viewport.clear_textures();
        }
    }
}

/// A picture made as the kit takes it: `Ok(None)` later, `Err` none
/// ("No picture", and why).
fn painted(made: Result<Option<ColorImage>, String>) -> Painted {
    match made {
        Ok(Some(image)) => Painted::Ready(image),
        Ok(None) => Painted::Later,
        Err(reason) => Painted::Failed(reason),
    }
}

/// The game's ten PLT palettes.
fn palettes(lib: &Library) -> Vec<Option<Rgba>> {
    mg_image::plt::PALETTES
        .iter()
        .map(|n| {
            let r = mg_core::ResRef::from_str(n).ok()?;
            let (t, d) = lib.resman().texture(r)?;
            mg_image::read(t, &d).ok().map(|t| t.to_rgba())
        })
        .collect()
}

/// A TGA's or DDS's pixels (rows top first) at its smallest mip level
/// still [`SIZE`] or more a side (the content deciding which it is, as the
/// Texture tab reads it).
fn texture_pixels(restype: ResType, data: &[u8]) -> Result<Rgba, String> {
    let t = if data.starts_with(b"DDS ") { ResType::DDS } else { restype };
    let other = if t == ResType::DDS { ResType::TGA } else { ResType::DDS };
    let tex = mg_image::read(t, data)
        .or_else(|_| mg_image::read(other, data))
        .map_err(|e| e.to_string())?;
    if tex.mips.is_empty() {
        return Err("no pixels".into());
    }
    let level = (0..tex.mips.len())
        .rev()
        .find(|l| {
            let (w, h) = tex.level_dims(*l);
            w.max(h) >= SIZE
        })
        .unwrap_or(0);
    let (w, h) = tex.level_dims(level);
    let data = match tex.format {
        mg_image::Format::Rgba8 => tex.mips[level].clone(),
        f => mg_image::bc::decode(f, w, h, &tex.mips[level]),
    };
    let mut image = Rgba { width: w, height: h, data };
    if !tex.has_alpha {
        for px in image.data.as_chunks_mut::<4>().0 {
            px[3] = 255;
        }
    }
    Ok(image.top_down())
}

/// `image` made at most `side` a side, by averaging blocks of pixels.
pub(crate) fn shrink(image: &Rgba, side: u32) -> Rgba {
    let (w, h) = (image.width, image.height);
    let f = w.max(h).div_ceil(side.max(1)).max(1);
    if f == 1 || image.data.len() != (w * h * 4) as usize {
        return image.clone();
    }
    let (nw, nh) = (w.div_ceil(f), h.div_ceil(f));
    let mut out = Rgba::new(nw, nh);
    for y in 0..nh {
        for x in 0..nw {
            let mut sum = [0u32; 4];
            let mut n = 0;
            for sy in y * f..((y + 1) * f).min(h) {
                for sx in x * f..((x + 1) * f).min(w) {
                    let i = ((sy * w + sx) * 4) as usize;
                    for (c, s) in sum.iter_mut().enumerate() {
                        *s += u32::from(image.data[i + c]);
                    }
                    n += 1;
                }
            }
            let o = ((y * nw + x) * 4) as usize;
            for (c, s) in sum.iter().enumerate() {
                out.data[o + c] = (s / n.max(1)) as u8;
            }
        }
    }
    out
}

/// An image (rows top first) as egui's.
/// A model's or creature's picture, unless nothing was drawn in it (every
/// pixel the background: a model whose meshes all have `render 0`, as the
/// base game's `a_ba_coat`): then "No picture", as the kit's rule is.
fn drawn_picture(image: &Rgba) -> Result<ColorImage, String> {
    let mut pixels = image.data.as_chunks::<4>().0.iter();
    let first = pixels.next();
    if first.is_some_and(|p| pixels.all(|q| q == p)) {
        return Err(NOTHING_DRAWN.into());
    }
    color_image(image)
}

/// Why a picture with nothing drawn in it is none.
pub const NOTHING_DRAWN: &str = "nothing to draw";

fn color_image(image: &Rgba) -> Result<ColorImage, String> {
    let (w, h) = (image.width as usize, image.height as usize);
    if w == 0 || h == 0 || image.data.len() != w * h * 4 {
        return Err("an empty picture".into());
    }
    Ok(ColorImage::from_rgba_unmultiplied([w, h], &image.data))
}

// ------------------------------------------------------------------ the grid

/// What a gallery shows: the Resources panel's list as it is filtered.
#[derive(Clone, Debug)]
pub(crate) enum Cells {
    /// Resources and the layers they are from; `detail`: the type under
    /// each name (where the filter lists more than one).
    Resources { items: Arc<Vec<(ResKey, String)>>, detail: bool },
    /// `appearance.2da` rows and their labels.
    Creatures(Arc<Vec<(usize, String)>>),
}

impl Cells {
    pub(crate) fn len(&self) -> usize {
        match self {
            Cells::Resources { items, .. } => items.len(),
            Cells::Creatures(rows) => rows.len(),
        }
    }

    pub(crate) fn pictured(&self, i: usize) -> Pictured {
        match self {
            Cells::Resources { items, .. } => Pictured::Resource(items[i].0),
            Cells::Creatures(rows) => Pictured::Creature(rows[i].0 as u16),
        }
    }

    /// The name under the picture.
    fn name(&self, i: usize) -> String {
        match self {
            Cells::Resources { items, .. } => {
                let key = items[i].0.to_string();
                key.rsplit_once('.').map_or(key.clone(), |(n, _)| n.to_string())
            }
            Cells::Creatures(rows) => rows[i].1.clone(),
        }
    }

    /// The detail line: the type, a row's number.
    fn detail(&self, i: usize) -> Option<String> {
        match self {
            Cells::Resources { items, detail: true } => {
                items[i].0.to_string().rsplit_once('.').map(|(_, t)| t.to_string())
            }
            Cells::Resources { .. } => None,
            Cells::Creatures(rows) => Some(format!("row {}", rows[i].0)),
        }
    }

    fn has_detail(&self) -> bool {
        !matches!(self, Cells::Resources { detail: false, .. })
    }

    /// What a screen reader (and a test) calls the cell: the resource's
    /// file name, the row and its label.
    fn said(&self, i: usize) -> String {
        match self {
            Cells::Resources { items, .. } => items[i].0.to_string(),
            Cells::Creatures(rows) => format!("{} {}", rows[i].0, rows[i].1),
        }
    }

    /// Its tooltip: what it is and where from.
    fn tip(&self, i: usize) -> String {
        match self {
            Cells::Resources { items, .. } => format!("{}\nfrom {}", items[i].0, items[i].1),
            Cells::Creatures(rows) => format!("{}\nappearance.2da row {}", rows[i].1, rows[i].0),
        }
    }
}

/// Where a gallery stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Place {
    /// The Resources panel, in Gallery mode.
    Panel,
    /// The editor area's Gallery tab.
    Tab,
}

/// A gallery's grid's id (its keys' cursor is kept under it).
pub fn grid_id(at: Place) -> egui::Id {
    egui::Id::new(("mgv-gallery", at))
}

/// Where a gallery's grid stood in the last frame it was drawn in, and
/// how its cells were laid out (for the tests, which click them).
#[derive(Clone, Debug, Default)]
pub struct Drawn {
    pub rect: Option<egui::Rect>,
    pub layout: Option<GridLayout>,
    /// The cells drawn, and those whose pictures were asked for.
    pub drawn: std::ops::Range<usize>,
    pub ahead: std::ops::Range<usize>,
}

/// The cell chosen among `cells`.
fn chosen(app: &Viewer, cells: &Cells) -> Option<usize> {
    match cells {
        Cells::Resources { items, .. } => {
            let k = app.browser.selected?;
            items.iter().position(|(x, _)| *x == k)
        }
        Cells::Creatures(rows) => {
            let r = app.browser.chosen_row?;
            rows.iter().position(|(x, _)| *x == r)
        }
    }
}

fn choose(app: &mut Viewer, cells: &Cells, i: usize) {
    match cells {
        Cells::Resources { items, .. } => app.browser.selected = Some(items[i].0),
        Cells::Creatures(rows) => app.browser.chosen_row = Some(rows[i].0),
    }
}

/// Whether `cells`' `i`th is what the window shows (so it is not opened
/// again), as the list tells.
fn shown(app: &Viewer, cells: &Cells, i: usize) -> bool {
    let doc = app.doc.as_ref();
    match cells {
        Cells::Resources { items, .. } => {
            doc.and_then(|d| d.opened.key.filter(|_| d.creature.is_none())) == Some(items[i].0)
        }
        Cells::Creatures(rows) => {
            doc.and_then(|d| d.creature).map(|c| usize::from(c.appearance)) == Some(rows[i].0)
        }
    }
}

fn open(cells: &Cells, i: usize) -> Action {
    match cells {
        Cells::Resources { items, .. } => Action::OpenResource(items[i].0),
        Cells::Creatures(rows) => Action::OpenCreature(CreatureLook::new(rows[i].0 as u16)),
    }
}

/// The gallery of `cells` in a scroll filling `ui`, as the list: a click
/// shows a picture's resource, the arrows move (the kit's [`kit::step`])
/// and show, a double click or Enter opens; what is shown already is not
/// opened again.
pub(crate) fn grid(app: &mut Viewer, ui: &mut Ui, cells: &Cells, at: Place) {
    let id = grid_id(at);
    let count = cells.len();
    let side = app.settings.gallery_side.clamp(kit::SIDE_MIN, kit::SIDE_MAX);
    let current = chosen(app, cells);
    let label = kit::label_height(false, 1, cells.has_detail());
    let resources = matches!(cells, Cells::Resources { .. });
    let name = match at {
        Place::Panel => "Resources gallery",
        Place::Tab => "Gallery",
    };
    let mut picked = None;
    let pictures = &mut app.pictures;
    let out = egui::ScrollArea::vertical()
        .id_salt(id)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            GalleryGrid::new(id, count, side).label(label).cursor(current).name(name).show(
                ui,
                |ui, i, rect, tile, response, state| {
                    let what = cells.pictured(i);
                    let picture = pictures.picture(what);
                    let (shown, said, detail) = (cells.name(i), cells.said(i), cells.detail(i));
                    GalleryCell::new(&shown, picture)
                        .said(&said)
                        .detail(detail.as_deref())
                        .kind(what.icon())
                        .model(resources)
                        .lines(1)
                        .selected(current == Some(i))
                        .paint(ui, rect, tile, state, response);
                    if response.clicked() || response.double_clicked() {
                        picked = Some(i);
                    }
                    if response.hovered() {
                        let mut tip = cells.tip(i);
                        if let Some(e) = pictures.error(what) {
                            tip += &format!("\nNo picture: {e}");
                        }
                        response.clone().on_hover_text(tip);
                    }
                },
            )
        })
        .inner;
    // (The row under those in sight: asked for before it comes into sight.)
    for i in out.ahead.clone() {
        app.pictures.want(cells.pictured(i));
    }
    app.browser.drawn.insert(
        at,
        Drawn { rect: Some(out.rect), layout: out.layout, drawn: out.drawn, ahead: out.ahead },
    );
    if let Some(i) = out.moved.or(picked).or(out.activated) {
        choose(app, cells, i);
        if !shown(app, cells, i) {
            app.actions.push(open(cells, i));
        }
    }
}

/// The pictures' size slider (the kit's, 64 to 220), `width` wide.
pub(crate) fn size_slider(app: &mut Viewer, ui: &mut Ui, width: f32) {
    ui.scope(|ui| {
        ui.spacing_mut().slider_width = width.max(kit::SIDE_MIN / 2.0);
        let r = kit::size_slider(
            ui,
            &mut app.settings.gallery_side,
            kit::SIDE_MIN..=kit::SIDE_MAX,
            None,
        );
        r.on_hover_text("The pictures' size");
    });
}

/// The tab's header row from this width up holds the slider too (the
/// toolset's palette: a row of its own under a dock of about 370).
pub const ONE_ROW_FROM: f32 = 370.0;

/// The Gallery tab's header: the filter, Find, then the count and the size
/// slider in the room left; under [`ONE_ROW_FROM`] (or with no room for
/// the slider) the count and the slider on a row of their own. Nothing
/// overlaps at any width.
fn header(app: &mut Viewer, ui: &mut Ui, count: &str) {
    const FILTER: f32 = 104.0;
    const SEARCH: std::ops::RangeInclusive<f32> = 80.0..=200.0;
    const SLIDER: std::ops::RangeInclusive<f32> = 64.0..=160.0;
    let gap = ui.spacing().item_spacing.x;
    let width = ui.available_width();
    let count_width = {
        let font = egui::TextStyle::Body.resolve(ui.style());
        ui.fonts_mut(|f| f.layout_no_wrap(count.to_owned(), font, egui::Color32::WHITE).size().x)
    };
    // (The combo's arrow and frame beside its text width.)
    let filter = FILTER + ui.spacing().icon_width + 2.0 * ui.spacing().button_padding.x;
    let fixed = filter + count_width + 3.0 * gap;
    let one_row = width >= ONE_ROW_FROM && width - fixed >= SEARCH.start() + SLIDER.start() + gap;
    let mut placed = Vec::new();
    let slider_row = |app: &mut Viewer, ui: &mut Ui, room: f32, placed: &mut Vec<_>| {
        let at = ui.scope(|ui| size_slider(app, ui, room)).response.rect;
        placed.push(("Size", at));
        placed.push(("Count", ui.weak(count).rect));
    };
    ui.horizontal(|ui| {
        placed.push((
            "Filter",
            ui.scope(|ui| {
                crate::panels::filter_combo(app, ui, "gallery-tab-filter", FILTER);
            })
            .response
            .rect,
        ));
        let left = (width - filter - gap).max(0.0);
        let search = match one_row {
            true => (left - SLIDER.start() - count_width - 2.0 * gap).min(*SEARCH.end()),
            false => left.min(*SEARCH.end() + 120.0),
        };
        placed.push((
            "Search",
            ui.scope(|ui| {
                crate::panels::search_field(app, ui, search.max(0.0));
            })
            .response
            .rect,
        ));
        if one_row {
            let room = width - filter - search - count_width - 4.0 * gap;
            slider_row(app, ui, room.clamp(*SLIDER.start(), *SLIDER.end()), &mut placed);
        }
    });
    if !one_row {
        ui.horizontal(|ui| {
            let room = (width - count_width - 2.0 * gap).min(*SLIDER.end() + 80.0);
            slider_row(app, ui, room.max(0.0), &mut placed);
        });
    }
    app.browser.tab_header = placed;
}

/// The Gallery tab: the filter, Find, the size slider and the count in
/// its header (the board "Gallery Tab at 3440×1440"), the pictures under
/// it.
pub(crate) fn tab(app: &mut Viewer, ui: &mut Ui) {
    let cells = crate::panels::current(app);
    let noun = match cells {
        Cells::Creatures(_) => "creatures",
        Cells::Resources { .. } => "resources",
    };
    let count = format!("{} {noun}", cells.len());
    egui::Frame::new().inner_margin(egui::Margin::symmetric(6, 4)).show(ui, |ui| {
        header(app, ui, &count);
    });
    // (The filter or the query changed in the header: listed again.)
    let cells = crate::panels::current(app);
    egui::Frame::new().inner_margin(egui::Margin::symmetric(6, 0)).show(ui, |ui| {
        ui.set_min_size(ui.available_size());
        grid(app, ui, &cells, Place::Tab);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::ResourcesView;
    use crate::{NoDialogs, Settings, Tab};
    use std::sync::Mutex;

    /// What the test painter did: each picture it was asked for, in
    /// order, the frames' counts, and how many times it forgot.
    #[derive(Default)]
    struct Log {
        painted: Vec<Pictured>,
        forgot: usize,
    }

    /// A painter with no GPU: a gray square with a dot for each picture
    /// (as a model is drawn: `drawn_picture`), the background alone for
    /// the names starting `empty`, none for the names in `fail`, Later
    /// the first `wait` times.
    struct Fake {
        log: Arc<Mutex<Log>>,
        fail: Vec<&'static str>,
        wait: u32,
    }

    impl Painter for Fake {
        fn paint(&mut self, _: &Library, what: Pictured, as_drawn: bool) -> Painted {
            self.log.lock().unwrap().painted.push(what);
            if let Pictured::Resource(k) = what
                && self.fail.iter().any(|f| k.to_string().starts_with(f))
            {
                return Painted::Failed("unreadable".into());
            }
            if self.wait > 0 && !as_drawn {
                self.wait -= 1;
                return Painted::Later;
            }
            let mut image = Rgba::new(4, 4);
            image.data = [90, 90, 90, 255].repeat(16);
            let empty = matches!(what, Pictured::Resource(k) if k.to_string().starts_with("empty"));
            if !empty {
                image.data[..4].copy_from_slice(&[200, 40, 40, 255]);
            }
            painted(drawn_picture(&image).map(Some))
        }
        fn forget(&mut self) {
            self.log.lock().unwrap().forgot += 1;
        }
    }

    /// Installs a [`Fake`]; a picture waiting is drawn again in the next
    /// frame (the kit's wait between tries is the clock's, not the
    /// frames').
    fn fake(v: &mut Viewer, fail: Vec<&'static str>, wait: u32) -> Arc<Mutex<Log>> {
        let log = Arc::new(Mutex::new(Log::default()));
        v.pictures.painter = Some(Box::new(Fake { log: log.clone(), fail, wait }));
        v.pictures.cache.retry.again = Duration::ZERO;
        log
    }

    const MODEL: &str = "newmodel NAME\nsetsupermodel NAME NULL\nclassification character\n\
        beginmodelgeom NAME\nnode dummy NAME\n  parent NULL\nendnode\nendmodelgeom NAME\n\
        donemodel NAME\n";

    /// A viewer with no game and no GPU, in Gallery mode, over a folder of
    /// `n` small models (`m000`, `m001`...) and a TXI.
    fn viewer(n: usize, name: &str) -> (Viewer, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("mgv-gallery-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for i in 0..n {
            let m = format!("m{i:03}");
            std::fs::write(dir.join(format!("{m}.mdl")), MODEL.replace("NAME", &m)).unwrap();
        }
        std::fs::write(dir.join("m000.txi"), "mipmap 0\n").unwrap();
        let settings = Settings {
            game_root: Some(std::path::PathBuf::from("/nonexistent")),
            use_user_dir: false,
            folders: vec![dir.clone()],
            resources_view: ResourcesView::Gallery,
            ..Settings::default()
        };
        (Viewer::new(settings, Box::new(NoDialogs)), dir)
    }

    /// Runs frames of a window `size` big, with `events` in the first.
    fn frame(ctx: &egui::Context, v: &mut Viewer, size: egui::Vec2, events: Vec<egui::Event>) {
        let time = ctx.input(|i| i.time) + 1.0 / 60.0;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            time: Some(time),
            events,
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| v.ui(ui));
        out.textures_delta.clear();
    }

    fn run(ctx: &egui::Context, v: &mut Viewer, size: egui::Vec2, frames: usize) {
        for _ in 0..frames {
            frame(ctx, v, size, Vec::new());
        }
    }

    const BOARD: egui::Vec2 = egui::vec2(1600.0, 900.0);

    fn key(k: egui::Key) -> Vec<egui::Event> {
        let press = |pressed| egui::Event::Key {
            key: k,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        vec![press(true), press(false)]
    }

    /// Clicks (twice: a double click) the middle of cell `i` of the grid at
    /// `at`.
    fn click(ctx: &egui::Context, v: &mut Viewer, at: Place, i: usize, twice: bool) {
        click_in(ctx, v, BOARD, at, i, twice);
    }

    fn click_in(
        ctx: &egui::Context,
        v: &mut Viewer,
        size: egui::Vec2,
        at: Place,
        i: usize,
        twice: bool,
    ) {
        let d = v.browser.drawn[&at].clone();
        let p = d.layout.unwrap().rect(d.rect.unwrap().min, i).center();
        let button = |pressed| egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(ctx, v, size, vec![egui::Event::PointerMoved(p)]);
        for _ in 0..if twice { 2 } else { 1 } {
            frame(ctx, v, size, vec![button(true)]);
            frame(ctx, v, size, vec![button(false)]);
        }
        run(ctx, v, size, 1);
    }

    fn named(v: &Viewer) -> Option<String> {
        v.doc.as_ref().map(crate::Doc::name)
    }

    /// The pictures asked for in a frame are those of the cells in sight
    /// and the row under them, never the whole list: 32,832 resources (a
    /// base install's) in the Left Dock and in the tab at 3440 by 1440,
    /// scrolled to the end and back; and each frame makes at most
    /// `PER_FRAME`.
    #[test]
    fn a_frame_asks_only_for_the_pictures_in_sight() {
        let (mut v, dir) = viewer(1, "bound");
        let log = fake(&mut v, Vec::new(), 0);
        let items: Vec<(ResKey, String)> = (0..32_832)
            .map(|i| (ResKey::parse(&format!("p{i:05}"), ResType::MDL).unwrap(), "game".into()))
            .collect();
        let items = Arc::new(items);
        let ctx = egui::Context::default();
        for (size, tab) in [(BOARD, false), (egui::vec2(3440.0, 1440.0), true)] {
            if tab {
                v.open_gallery_tab();
            }
            let at = if tab { Place::Tab } else { Place::Panel };
            v.browser.inject(items.clone());
            run(&ctx, &mut v, size, 3);
            let check = |v: &Viewer, log: &Arc<Mutex<Log>>, what: &str| {
                let d = v.browser.drawn[&at].clone();
                let layout = d.layout.unwrap();
                assert!(!d.drawn.is_empty(), "{what}: cells drawn");
                // (Those in sight, one row ahead, both grids' in the tab's
                // case: the panel shows its list then.)
                let most = d.ahead.len() + layout.columns;
                assert!(v.pictures.asked() <= most, "{what}: {} > {most}", v.pictures.asked());
                assert!(v.pictures.asked() < 400, "{what}: {}", v.pictures.asked());
                assert!(v.pictures.painted <= PER_FRAME, "{what}");
                assert!(log.lock().unwrap().painted.len() < 2000, "{what}");
            };
            check(&v, &log, "at the top");
            // The keys to the end: the last cells are drawn and asked for.
            // (A click gives the grid the keys.)
            click_in(&ctx, &mut v, size, at, 0, false);
            assert!(ctx.memory(|m| m.has_focus(grid_id(at))), "{size:?}: the keys");
            frame(&ctx, &mut v, size, key(egui::Key::End));
            // (The scroll is animated: a second or two of frames.)
            for _ in 0..120 {
                run(&ctx, &mut v, size, 1);
                check(&v, &log, "scrolling");
            }
            let d = v.browser.drawn[&at].clone();
            assert!(d.drawn.contains(&(items.len() - 1)), "{size:?}: {:?}", d.drawn);
            assert_eq!(v.browser.selected, Some(items[items.len() - 1].0));
            check(&v, &log, "at the end");
            frame(&ctx, &mut v, size, key(egui::Key::Home));
            run(&ctx, &mut v, size, 120);
            assert_eq!(v.browser.drawn[&at].drawn.start, 0);
            check(&v, &log, "back at the top");
        }
        // Nothing far from sight was ever made.
        let painted = log.lock().unwrap().painted.clone();
        let middle = Pictured::Resource(items[16_000].0);
        assert!(!painted.contains(&middle));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Pictures are made a few a frame, after it, until those asked for
    /// are all made: each once.
    #[test]
    fn pictures_are_made_a_few_a_frame_and_kept() {
        let (mut v, dir) = viewer(40, "made");
        let log = fake(&mut v, Vec::new(), 0);
        let ctx = egui::Context::default();
        run(&ctx, &mut v, BOARD, 1);
        assert!(v.pictures.painted <= PER_FRAME);
        run(&ctx, &mut v, BOARD, 30);
        let painted = log.lock().unwrap().painted.clone();
        let asked = v.pictures.asked();
        assert!(asked > PER_FRAME, "{asked}");
        assert_eq!(v.pictures.waiting(), 0);
        let unique: HashSet<_> = painted.iter().collect();
        assert_eq!(unique.len(), painted.len(), "each made once");
        // The TXI has no picture: its kind's icon, never asked for.
        let txi = Pictured::Resource(ResKey::parse("m000", ResType::TXI).unwrap());
        assert!(!txi.has_picture() && !painted.contains(&txi));
        assert_eq!(v.pictures.picture(txi), Picture::Making);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A picture that fails shows "No picture" (and why, in its tip), and
    /// is not tried again; one that waits for the game's programs is tried
    /// again in a later frame.
    #[test]
    fn a_failure_is_no_picture_and_waiting_is_tried_again() {
        let (mut v, dir) = viewer(3, "fail");
        let log = fake(&mut v, vec!["m001"], 2);
        let ctx = egui::Context::default();
        run(&ctx, &mut v, BOARD, 12);
        let m = |n: &str| Pictured::Resource(ResKey::parse(n, ResType::MDL).unwrap());
        assert_eq!(v.pictures.picture(m("m001")), Picture::Missing);
        assert_eq!(v.pictures.error(m("m001")), Some("unreadable"));
        assert!(matches!(v.pictures.picture(m("m000")), Picture::Ready(_)));
        assert!(matches!(v.pictures.picture(m("m002")), Picture::Ready(_)));
        let painted = log.lock().unwrap().painted.clone();
        assert_eq!(painted.iter().filter(|p| **p == m("m001")).count(), 1, "not tried again");
        assert!(painted.len() >= 5, "the two waits drawn again: {painted:?}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A model drawn with nothing in it (only the background: all its
    /// meshes `render 0`) is "No picture", "nothing to draw" in its tip;
    /// one waiting for the game's programs shows the kind's icon (the
    /// kit's `Making`), never a blank cell.
    #[test]
    fn nothing_drawn_is_no_picture_and_waiting_is_the_kinds_icon() {
        let (mut v, dir) = viewer(2, "empty");
        std::fs::write(dir.join("empty_coat.mdl"), MODEL.replace("NAME", "empty_coat")).unwrap();
        v.actions.push(Action::Refresh);
        v.run_actions();
        fake(&mut v, Vec::new(), 0);
        let ctx = egui::Context::default();
        run(&ctx, &mut v, BOARD, 4);
        let m = |n: &str| Pictured::Resource(ResKey::parse(n, ResType::MDL).unwrap());
        assert_eq!(v.pictures.picture(m("empty_coat")), Picture::Missing);
        assert_eq!(v.pictures.error(m("empty_coat")), Some(NOTHING_DRAWN));
        assert!(matches!(v.pictures.picture(m("m000")), Picture::Ready(_)));
        // Waiting: the kind's icon, the picture asked for again.
        let (mut v, dir2) = viewer(2, "making");
        fake(&mut v, Vec::new(), u32::MAX);
        run(&ctx, &mut v, BOARD, 3);
        assert_eq!(v.pictures.picture(m("m000")), Picture::Making);
        assert_eq!(v.pictures.error(m("m000")), None);
        assert!(v.pictures.waiting() > 0);
        for d in [dir, dir2] {
            std::fs::remove_dir_all(&d).unwrap();
        }
    }

    /// A picture still waiting after [`WAIT_AT_MOST`] is taken as drawn;
    /// one no longer asked for stops waiting (its wait starts again when
    /// it comes back into sight).
    #[test]
    fn a_long_wait_takes_the_picture_as_drawn_and_unseen_ones_stop_waiting() {
        let (mut v, dir) = viewer(3, "wait");
        fake(&mut v, Vec::new(), u32::MAX);
        let ctx = egui::Context::default();
        run(&ctx, &mut v, BOARD, 2);
        let m = Pictured::Resource(ResKey::parse("m000", ResType::MDL).unwrap());
        assert_eq!(v.pictures.picture(m), Picture::Making);
        assert_eq!(v.pictures.cache.retry.give_up, WAIT_AT_MOST);
        // Out of sight: the waits go. (Had they stayed, a wait of nothing
        // would take it as drawn at once when it is back in sight.)
        v.settings.resources_view = ResourcesView::List;
        run(&ctx, &mut v, BOARD, 1);
        v.pictures.cache.retry.give_up = Duration::ZERO;
        v.settings.resources_view = ResourcesView::Gallery;
        run(&ctx, &mut v, BOARD, 1);
        assert_eq!(v.pictures.picture(m), Picture::Making, "waiting anew");
        // The wait over: taken as drawn.
        run(&ctx, &mut v, BOARD, 1);
        assert!(matches!(v.pictures.picture(m), Picture::Ready(_)));
        assert_eq!(v.pictures.error(m), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Reading the game's data again (Refresh) lets go of every picture
    /// and the painter's own; opening one from the gallery keeps them.
    #[test]
    fn a_reload_lets_go_of_the_pictures_and_opening_keeps_them() {
        let (mut v, dir) = viewer(6, "reload");
        let log = fake(&mut v, Vec::new(), 0);
        let ctx = egui::Context::default();
        run(&ctx, &mut v, BOARD, 6);
        let kept = v.pictures.kept();
        assert!(kept >= 6, "{kept}");
        click(&ctx, &mut v, Place::Panel, 2, true);
        assert_eq!(named(&v).as_deref(), Some("m002"));
        assert!(v.pictures.kept() >= kept, "opening keeps them");
        let before = log.lock().unwrap().forgot;
        v.actions.push(Action::Refresh);
        v.run_actions();
        assert_eq!(v.pictures.kept(), 0);
        assert_eq!(log.lock().unwrap().forgot, before + 1);
        run(&ctx, &mut v, BOARD, 6);
        assert!(v.pictures.kept() >= 6, "made again");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The List and Gallery buttons switch the panel, and the choice is in
    /// the settings; as in the list, a click shows a picture's resource
    /// (not again if shown), the arrows move and show, a double click and
    /// Enter open.
    #[test]
    fn the_switch_a_click_a_double_click_and_the_keys() {
        let (mut v, dir) = viewer(12, "keys");
        v.settings.resources_view = ResourcesView::List;
        fake(&mut v, Vec::new(), 0);
        let ctx = egui::Context::default();
        run(&ctx, &mut v, BOARD, 3);
        assert!(!v.browser.drawn.contains_key(&Place::Panel), "a list");
        v.settings.resources_view = ResourcesView::Gallery;
        run(&ctx, &mut v, BOARD, 3);
        let d = v.browser.drawn[&Place::Panel].clone();
        let columns = d.layout.unwrap().columns;
        assert!(columns >= 2, "{columns}");
        click(&ctx, &mut v, Place::Panel, 0, false);
        let m = |n: &str| ResKey::parse(n, ResType::MDL).unwrap();
        assert_eq!(v.browser.selected, Some(m("m000")));
        assert_eq!(named(&v).as_deref(), Some("m000"), "a click shows");
        // (The same click again: not opened again, the document kept.)
        let doc = |v: &Viewer| v.doc.as_ref().map(|d| d as *const crate::Doc);
        let before = doc(&v);
        click(&ctx, &mut v, Place::Panel, 0, false);
        assert_eq!(doc(&v), before, "what is shown is not opened again");
        frame(&ctx, &mut v, BOARD, key(egui::Key::ArrowRight));
        run(&ctx, &mut v, BOARD, 1);
        assert_eq!(v.browser.selected, Some(m("m001")), "right: the next picture");
        assert_eq!(named(&v).as_deref(), Some("m001"), "and shows it");
        frame(&ctx, &mut v, BOARD, key(egui::Key::ArrowDown));
        run(&ctx, &mut v, BOARD, 1);
        let below = format!("m{:03}", 1 + columns);
        assert_eq!(v.browser.selected, Some(m(&below)), "down: a row");
        assert_eq!(named(&v).as_deref(), Some(below.as_str()));
        // Enter opens what is chosen and not shown (here, after the list
        // opened another).
        v.actions.push(Action::OpenResource(m("m000")));
        v.run_actions();
        frame(&ctx, &mut v, BOARD, key(egui::Key::Enter));
        run(&ctx, &mut v, BOARD, 1);
        assert_eq!(named(&v).as_deref(), Some(below.as_str()), "Enter opens");
        click(&ctx, &mut v, Place::Panel, 2, true);
        assert_eq!(named(&v).as_deref(), Some("m002"), "a double click opens");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Open as Tab: the Gallery tab in a group of its own left of the 3D
    /// View's (the wide board), the panel back to its list; what opens
    /// from it leaves it in sight; it closes as Texture and ASCII do.
    #[test]
    fn the_gallery_opens_as_a_tab() {
        let (mut v, dir) = viewer(12, "tab");
        fake(&mut v, Vec::new(), 0);
        let ctx = egui::Context::default();
        run(&ctx, &mut v, BOARD, 2);
        v.open_gallery_tab();
        run(&ctx, &mut v, BOARD, 3);
        assert_eq!(v.settings.resources_view, ResourcesView::List);
        let beside_the_view = |v: &Viewer| {
            let (g, view) = (v.dock.find_tab(&Tab::Gallery), v.dock.find_tab(&Tab::View));
            let (g, view) = (g.expect("the tab"), view.unwrap());
            let parent = view.node.parent().unwrap();
            g.node == parent.left() && view.node == parent.right()
        };
        assert!(beside_the_view(&v), "{:?}", crate::shell::groups(&v.dock));
        assert!(v.browser.drawn.contains_key(&Place::Tab));
        click(&ctx, &mut v, Place::Tab, 3, true);
        assert_eq!(named(&v).as_deref(), Some("m003"));
        let front: Vec<Tab> = v.dock.iter_leaves().map(|(_, l)| l.tabs[l.active.0]).collect();
        assert!(front.contains(&Tab::Gallery) && front.contains(&Tab::View), "{front:?}");
        v.close_tab(Tab::Gallery);
        assert!(v.dock.find_tab(&Tab::Gallery).is_none());
        // Opened again, it comes back in the same place.
        v.focus(Tab::Gallery);
        assert!(beside_the_view(&v));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The Gallery tab's header never overlaps its controls nor runs past
    /// its width, from a tab squeezed beside the 3D View (about 330 at
    /// 1600 by 900) to one across a wide window; the slider and count go on
    /// a row of their own under [`ONE_ROW_FROM`].
    #[test]
    fn the_tab_s_header_never_overlaps() {
        let (mut v, dir) = viewer(1, "header");
        fake(&mut v, Vec::new(), 0);
        let items: Vec<(ResKey, String)> = (0..32_832)
            .map(|i| (ResKey::parse(&format!("p{i:05}"), ResType::MDL).unwrap(), "game".into()))
            .collect();
        v.browser.inject(Arc::new(items));
        let ctx = egui::Context::default();
        for width in [180.0, 240.0, 300.0, 330.0, 369.0, 370.0, 420.0, 600.0, 1200.0, 2400.0] {
            for _ in 0..2 {
                let input = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, BOARD)),
                    ..Default::default()
                };
                let mut out = ctx.run_ui(input, |ui| {
                    egui::CentralPanel::default().show(ui, |ui| {
                        let at =
                            egui::Rect::from_min_size(ui.cursor().min, egui::vec2(width, 600.0));
                        ui.scope_builder(egui::UiBuilder::new().max_rect(at), |ui| tab(&mut v, ui));
                    });
                });
                out.textures_delta.clear();
            }
            let placed = v.browser.tab_header.clone();
            let names: Vec<_> = placed.iter().map(|p| p.0).collect();
            assert_eq!(names.len(), 4, "{width}: {names:?}");
            let left = placed[0].1.left();
            for (i, (a, ra)) in placed.iter().enumerate() {
                assert!(ra.width() > 0.0, "{width}: {a} {ra:?}");
                assert!(
                    ra.right() <= left + width - 12.0 + 0.5,
                    "{width}: {a} past the tab {ra:?}"
                );
                for (b, rb) in &placed[i + 1..] {
                    let overlap = ra.intersect(*rb);
                    let touching = overlap.width() <= 0.5 || overlap.height() <= 0.5;
                    assert!(touching, "{width}: {a} {ra:?} over {b} {rb:?}");
                }
            }
            let rows = |name| placed.iter().find(|p| p.0 == name).unwrap().1.center().y;
            let one_row = (rows("Size") - rows("Search")).abs() < 2.0;
            assert_eq!(one_row, width >= ONE_ROW_FROM + 12.0, "{width}");
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Gallery mode draws at every size, in the panel and as a tab, in
    /// both themes, the creatures' too (none without a game).
    #[test]
    fn the_gallery_draws_at_every_size() {
        let (mut v, dir) = viewer(30, "sizes");
        fake(&mut v, Vec::new(), 0);
        for theme in mg_uikit::Theme::ALL {
            v.settings.theme = theme;
            for size in
                [(640.0, 480.0), (800.0, 600.0), (1280.0, 800.0), (1600.0, 900.0), (3440.0, 1440.0)]
            {
                let ctx = egui::Context::default();
                let size = egui::vec2(size.0, size.1);
                v.settings.resources_view = ResourcesView::Gallery;
                v.shell.show_panel(Tab::Browser);
                run(&ctx, &mut v, size, 3);
                v.browser.filter = crate::panels::Filter::Creatures;
                run(&ctx, &mut v, size, 2);
                v.browser.filter = crate::panels::Filter::All;
                v.focus(Tab::Gallery);
                run(&ctx, &mut v, size, 3);
                let d = v.browser.drawn[&Place::Tab].clone();
                assert!(!d.drawn.is_empty(), "{size:?}");
                let room = d.rect.unwrap();
                let last = d.layout.unwrap().rect(room.min, d.layout.unwrap().columns - 1);
                assert!(last.right() <= room.right() + 0.5, "{size:?}: cut");
                v.close_tab(Tab::Gallery);
                v.browser.filter = crate::panels::Filter::Models;
            }
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn textures_are_shrunk_to_the_picture_side() {
        let mut big = Rgba::new(1024, 512);
        for (i, px) in big.data.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            *px = if i % 2 == 0 { [255, 0, 0, 255] } else { [0, 0, 255, 255] };
        }
        let small = shrink(&big, SIZE);
        assert_eq!((small.width, small.height), (205, 103));
        assert!(small.width <= SIZE && small.height <= SIZE);
        // (Averaged: red and blue in halves.)
        let px = small.pixel(10, 10);
        assert!(px[0] > 100 && px[2] > 100 && px[3] == 255, "{px:?}");
        let tiny = Rgba::new(16, 16);
        assert_eq!(shrink(&tiny, SIZE).width, 16);
        assert!(color_image(&Rgba::new(0, 0)).is_err());
        // A texture that does not read: an error, never a panic.
        assert!(texture_pixels(ResType::TGA, b"not a texture").is_err());
        assert!(texture_pixels(ResType::DDS, b"DDS ").is_err());
    }

    /// The kinds' icons and which have pictures.
    #[test]
    fn kinds() {
        let k = |n: &str, t| Pictured::Resource(ResKey::parse(n, t).unwrap());
        assert_eq!(k("a", ResType::MDL).icon(), Icon::Model);
        assert_eq!(k("a", ResType::UTC).icon(), Icon::Creature);
        assert_eq!(k("a", ResType::TGA).icon(), Icon::Texture);
        assert!(k("a", ResType::TGA).has_picture() && k("a", ResType::PLT).has_picture());
        assert!(k("a", ResType::MTR).has_picture() && k("a", ResType::UTP).has_picture());
        assert!(!k("a", ResType::WOK).has_picture() && !k("a", ResType::TXI).has_picture());
        assert!(Pictured::Creature(6).has_picture());
    }
}
