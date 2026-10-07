//! Galleries: many models rendered to thumbnails, with an HTML index and a
//! JSON manifest.
//!
//! What goes in: models whose names match a pattern, the models a 2DA
//! names (`placeables.2da`, `appearance.2da` as creatures,
//! `visualeffects.2da`, the door tables), every model of a hak, module or
//! ERF, or every model file in a folder. Each is rendered as the command
//! line's `mgv render` renders it, on one GPU device, in order.
//!
//! The output is deterministic (same inputs, same bytes: no dates), and
//! re-running skips items whose inputs (the model, its supermodels and
//! textures, the picture's settings) have not changed.

mod html;
pub mod sheet;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use mg_core::ResType;
use mg_mdl::NodeKind;
use mg_render::Gpu;
use mg_resman::ResKey;
use mgv_library::Library;
use mgv_stage::headless::{self, Shot};
use mgv_stage::subject::{self, CreatureLook};
use mgv_stage::{Stage, Viewport};
use serde_json::{Value, json};
use thiserror::Error;

pub use html::index_html;

#[derive(Debug, Error)]
pub enum GalleryError {
    #[error("{0}")]
    Source(String),
    #[error("{path}: {message}")]
    Io { path: PathBuf, message: String },
}

fn io(path: &Path, e: std::io::Error) -> GalleryError {
    GalleryError::Io { path: path.to_path_buf(), message: e.to_string() }
}

/// What a gallery shows.
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    /// Model names matching a pattern (`*`, `?`), e.g. `plc_*`.
    Pattern(String),
    /// The models a 2DA names.
    Table(Table),
    /// Every model in a hak, module or ERF.
    Archive(PathBuf),
    /// Every model file in a folder.
    Folder(PathBuf),
}

/// The 2DAs a gallery can follow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Table {
    /// `placeables.2da` `ModelName`.
    Placeables,
    /// `appearance.2da`: each row as a creature (part-based bodies bare).
    Appearance,
    /// `visualeffects.2da`: each row's first model.
    VisualEffects,
    /// `genericdoors.2da` and `doortypes.2da`.
    Doors,
}

impl Table {
    pub fn parse(s: &str) -> Option<Table> {
        match s.trim_end_matches(".2da").to_ascii_lowercase().as_str() {
            "placeables" => Some(Table::Placeables),
            "appearance" => Some(Table::Appearance),
            "visualeffects" => Some(Table::VisualEffects),
            "doors" | "doortypes" | "genericdoors" => Some(Table::Doors),
            _ => None,
        }
    }
}

impl Source {
    /// From the command line: `2da:NAME`, `hak:FILE`, `folder:DIR`,
    /// `pattern:TEXT`, or, without a prefix, a folder or archive that
    /// exists, a 2DA's name, else a pattern.
    pub fn parse(s: &str) -> Result<Source, GalleryError> {
        if let Some((kind, rest)) = s.split_once(':')
            && kind.len() > 1
        {
            return match kind {
                "2da" => Table::parse(rest).map(Source::Table).ok_or_else(|| {
                    GalleryError::Source(format!(
                        "{rest}: tables are placeables, appearance, visualeffects and doors"
                    ))
                }),
                "hak" | "mod" | "erf" | "archive" => Ok(Source::Archive(rest.into())),
                "folder" | "dir" => Ok(Source::Folder(rest.into())),
                "pattern" => Ok(Source::Pattern(rest.into())),
                k => Err(GalleryError::Source(format!("{k}: use 2da:, hak:, folder: or pattern:"))),
            };
        }
        let p = Path::new(s);
        if p.is_dir() {
            return Ok(Source::Folder(p.into()));
        }
        if p.is_file() {
            return Ok(Source::Archive(p.into()));
        }
        if let Some(t) = Table::parse(s) {
            return Ok(Source::Table(t));
        }
        Ok(Source::Pattern(s.into()))
    }
}

/// What one item renders.
#[derive(Debug, Clone, PartialEq)]
pub enum What {
    /// A model by resource name.
    Model(String),
    /// A model file.
    File(PathBuf),
    /// A creature by appearance row.
    Creature(CreatureLook),
}

/// One picture of a gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    /// Unique; names the image file.
    pub id: String,
    pub label: String,
    /// A heading it goes under (a 2DA row's label, say).
    pub group: Option<String>,
    pub what: What,
}

/// `*` (any run) and `?` (one character) matching, case-insensitive.
pub fn matches(pattern: &str, name: &str) -> bool {
    fn go(p: &[u8], n: &[u8]) -> bool {
        match (p.first(), n.first()) {
            (None, None) => true,
            (Some(b'*'), _) => go(&p[1..], n) || (!n.is_empty() && go(p, &n[1..])),
            (Some(b'?'), Some(_)) => go(&p[1..], &n[1..]),
            (Some(a), Some(b)) if a.eq_ignore_ascii_case(b) => go(&p[1..], &n[1..]),
            _ => false,
        }
    }
    go(pattern.as_bytes(), name.as_bytes())
}

fn cell(t: &mg_2da::TwoDa, row: usize, col: &str) -> Option<String> {
    t.get(row, col).filter(|v| !v.is_empty() && *v != "****").map(str::to_string)
}

/// The items of a source.
pub fn select(lib: &mut Library, source: &Source) -> Result<Vec<Item>, GalleryError> {
    let model_item = |name: &str| Item {
        id: name.to_ascii_lowercase(),
        label: name.to_ascii_lowercase(),
        group: None,
        what: What::Model(name.to_ascii_lowercase()),
    };
    match source {
        Source::Pattern(p) => Ok(lib
            .resman()
            .list(ResType::MDL)
            .into_iter()
            .map(|r| r.to_lowercase().to_string())
            .filter(|n| matches(p, n))
            .map(|n| model_item(&n))
            .collect()),
        Source::Folder(dir) => {
            lib.add_folder(dir);
            let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
                .map_err(|e| io(dir, e))?
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("mdl")))
                .collect();
            files.sort();
            Ok(files
                .into_iter()
                .map(|p| {
                    let stem = p
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_lowercase())
                        .unwrap_or_default();
                    Item { id: stem.clone(), label: stem, group: None, what: What::File(p) }
                })
                .collect())
        }
        Source::Archive(path) => {
            lib.add_archive(path).map_err(|e| GalleryError::Source(e.to_string()))?;
            let label = format!("archive:{}", path.display());
            let layer =
                lib.resman().layers().iter().find(|l| l.label == label).ok_or_else(|| {
                    GalleryError::Source(format!("{}: not added", path.display()))
                })?;
            let mut names: Vec<String> = layer
                .container
                .keys()
                .filter(|k| k.restype == ResType::MDL)
                .map(|k| k.resref.to_lowercase().to_string())
                .collect();
            names.sort();
            Ok(names.iter().map(|n| model_item(n)).collect())
        }
        Source::Table(t) => table_items(lib, *t),
    }
}

fn table_items(lib: &Library, t: Table) -> Result<Vec<Item>, GalleryError> {
    let game = lib.game();
    let open = |name: &str| game.table(name).map_err(|e| GalleryError::Source(e.to_string()));
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    match t {
        Table::Placeables => {
            let tb = open("placeables")?;
            for row in 0..tb.len() {
                let Some(model) = cell(&tb, row, "ModelName") else { continue };
                let label = cell(&tb, row, "Label").unwrap_or_else(|| model.clone());
                out.push(Item {
                    id: format!("{row:05}_{}", model.to_ascii_lowercase()),
                    label: format!("{row}: {label}"),
                    group: None,
                    what: What::Model(model.to_ascii_lowercase()),
                });
            }
        }
        Table::Appearance => {
            let tb = open("appearance")?;
            for row in 0..tb.len().min(u16::MAX as usize) {
                let Some(race) = cell(&tb, row, "RACE") else { continue };
                let label = cell(&tb, row, "LABEL").unwrap_or_else(|| race.clone());
                out.push(Item {
                    id: format!("{row:05}_{}", race.to_ascii_lowercase()),
                    label: format!("{row}: {label}"),
                    group: None,
                    what: What::Creature(CreatureLook::new(row as u16)),
                });
            }
        }
        Table::VisualEffects => {
            let tb = open("visualeffects")?;
            let columns = [
                "Imp_Impact_Node",
                "Imp_HeadCon_Node",
                "Imp_Root_M_Node",
                "Imp_Root_S_Node",
                "Imp_Root_L_Node",
                "Imp_Root_H_Node",
            ];
            for row in 0..tb.len() {
                let Some(model) = columns.iter().find_map(|c| cell(&tb, row, c)) else { continue };
                let label = cell(&tb, row, "Label").unwrap_or_else(|| model.clone());
                out.push(Item {
                    id: format!("{row:05}_{}", model.to_ascii_lowercase()),
                    label: format!("{row}: {label}"),
                    group: cell(&tb, row, "Type_FD"),
                    what: What::Model(model.to_ascii_lowercase()),
                });
            }
        }
        Table::Doors => {
            for (table, col) in [("genericdoors", "ModelName"), ("doortypes", "Model")] {
                let tb = open(table)?;
                for row in 0..tb.len() {
                    let Some(model) = cell(&tb, row, col) else { continue };
                    if !seen.insert(model.to_ascii_lowercase()) {
                        continue;
                    }
                    let label = cell(&tb, row, "Label").unwrap_or_else(|| model.clone());
                    out.push(Item {
                        id: format!("{table}_{row:05}_{}", model.to_ascii_lowercase()),
                        label: format!("{label} ({table} {row})"),
                        group: Some(table.into()),
                        what: What::Model(model.to_ascii_lowercase()),
                    });
                }
            }
        }
    }
    // Rows naming models the game does not have (reserved rows) are not
    // pictures.
    out.retain(|i| match &i.what {
        What::Model(n) => ResKey::parse(n, ResType::MDL).is_some_and(|k| lib.resman().contains(&k)),
        _ => true,
    });
    Ok(out)
}

/// How a gallery is made.
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    pub shot: Shot,
    pub out: PathBuf,
    pub title: String,
    /// Skip items whose inputs have not changed since the last run.
    pub incremental: bool,
}

/// One picture's record.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub item: Item,
    /// The image, relative to the gallery folder.
    pub image: Option<String>,
    /// What went into it.
    pub hash: String,
    /// What the model holds.
    pub info: Value,
    pub error: Option<String>,
    /// Rendered this run (not kept from the last).
    pub rendered: bool,
}

/// 64-bit FNV-1a, for input hashes that stay the same across builds.
#[derive(Debug, Clone, Copy)]
struct Fnv(u64);

impl Fnv {
    fn new() -> Fnv {
        Fnv(0xcbf2_9ce4_8422_2325)
    }
    fn add(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 ^= u64::from(*b);
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
        // A separator, so ["ab", "c"] and ["a", "bc"] differ.
        self.0 = self.0.wrapping_mul(0x0100_0000_01b3) ^ 0xff;
    }
}

/// Raised when the same inputs give other pictures, so galleries render
/// again: 3, pictures framed as close as shows the model from their angle,
/// by its vertices where they are drawn.
const PICTURES: &str = "3";

/// Everything that decides an item's picture: the shot, the model and its
/// supermodels, and the textures its meshes name.
fn input_hash(lib: &Library, item: &Item, shot: &Shot) -> String {
    let mut h = Fnv::new();
    h.add(env!("CARGO_PKG_VERSION").as_bytes());
    h.add(PICTURES.as_bytes());
    h.add(format!("{shot:?}").as_bytes());
    h.add(format!("{:?}", item.what).as_bytes());
    let mut models: Vec<String> = match &item.what {
        What::Model(n) => vec![n.clone()],
        What::File(p) => {
            if let Ok(d) = std::fs::read(p) {
                h.add(&d);
            }
            Vec::new()
        }
        What::Creature(_) => Vec::new(),
    };
    let mut seen = std::collections::HashSet::new();
    while let Some(name) = models.pop() {
        if !seen.insert(name.clone()) || seen.len() > 32 {
            continue;
        }
        let Some(key) = ResKey::parse(&name, ResType::MDL) else { continue };
        let Some(data) = lib.get(&key) else { continue };
        h.add(&data);
        if let Ok(m) = mg_mdl::Model::read(&data) {
            if let Some(s) = &m.supermodel {
                models.push(s.clone());
            }
            for n in &m.nodes {
                if let NodeKind::Mesh(mesh) = &n.kind {
                    for t in mesh.textures.iter().flatten() {
                        for ty in
                            [ResType::MTR, ResType::DDS, ResType::TGA, ResType::PLT, ResType::TXI]
                        {
                            if let Some(d) = ResKey::parse(t, ty).and_then(|k| lib.get(&k)) {
                                h.add(&d);
                            }
                        }
                    }
                }
            }
        }
    }
    format!("{:016x}", h.0)
}

/// What a model holds, for the manifest.
fn info(stage: &Stage) -> Value {
    let Some(a) = stage.actors().first() else { return Value::Null };
    let m = &a.model.model;
    let (mut faces, mut vertices) = (0, 0);
    for n in &m.nodes {
        if let Some(mesh) = n.mesh() {
            faces += mesh.faces.len();
            vertices += mesh.vertices.len();
        }
    }
    let bounds =
        stage.bounds().map(|(min, max)| json!({ "min": min.to_array(), "max": max.to_array() }));
    json!({
        "model": m.name,
        "classification": format!("{:?}", m.classification).to_lowercase(),
        "supermodel": m.supermodel,
        "nodes": m.nodes.len(),
        "faces": faces,
        "vertices": vertices,
        "parts": stage.actors().len() - 1,
        "animations": a.animations.names().collect::<Vec<_>>(),
        "playing": a.current().map(|(n, _)| n),
        "bounds": bounds,
    })
}

/// The last run's entries by id, from its manifest.
fn previous(out: &Path) -> BTreeMap<String, (String, Value)> {
    let Ok(text) = std::fs::read_to_string(out.join("manifest.json")) else {
        return BTreeMap::new();
    };
    let Ok(v) = serde_json::from_str::<Value>(&text) else { return BTreeMap::new() };
    v["items"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|e| e["error"].is_null())
        .filter_map(|e| {
            Some((
                e["id"].as_str()?.to_string(),
                (e["hash"].as_str()?.to_string(), e["info"].clone()),
            ))
        })
        .collect()
}

/// Renders the items into `opts.out` (`images/ID.png`), then writes the
/// manifest and index. `progress` hears each item as it starts.
pub fn run(
    lib: &mut Library,
    gpu: &Gpu,
    items: &[Item],
    opts: &Options,
    progress: &mut dyn FnMut(usize, usize, &Item),
) -> Result<Vec<Entry>, GalleryError> {
    let images = opts.out.join("images");
    std::fs::create_dir_all(&images).map_err(|e| io(&images, e))?;
    let before = if opts.incremental { previous(&opts.out) } else { BTreeMap::new() };
    let mut stage = Stage::new(gpu.clone());
    let mut viewport = Viewport::new(gpu);
    let mut entries = Vec::with_capacity(items.len());
    for (i, item) in items.iter().enumerate() {
        progress(i, items.len(), item);
        let hash = input_hash(lib, item, &opts.shot);
        let file = format!("images/{}.png", item.id);
        if let Some((h, info)) = before.get(&item.id)
            && *h == hash
            && opts.out.join(&file).is_file()
        {
            entries.push(Entry {
                item: item.clone(),
                image: Some(file),
                hash,
                info: info.clone(),
                error: None,
                rendered: false,
            });
            continue;
        }
        let shown = show(&mut stage, lib, item);
        let entry = match shown {
            Ok(_) => {
                let img = headless::still(&mut stage, &mut viewport, lib, &opts.shot);
                let path = opts.out.join(&file);
                mgv_stage::render::save_png(&img, &path).map_err(|e| io(&path, e))?;
                Entry {
                    item: item.clone(),
                    image: Some(file),
                    hash,
                    info: info(&stage),
                    error: None,
                    rendered: true,
                }
            }
            Err(e) => Entry {
                item: item.clone(),
                image: None,
                hash,
                info: Value::Null,
                error: Some(e),
                rendered: true,
            },
        };
        entries.push(entry);
    }
    write_index(&opts.out, &opts.title, &opts.shot, &entries)?;
    Ok(entries)
}

/// Puts an item on the stage, alone.
pub(crate) fn show(stage: &mut Stage, lib: &mut Library, item: &Item) -> Result<(), String> {
    match &item.what {
        What::Model(name) => ResKey::parse(name, ResType::MDL)
            .ok_or_else(|| format!("{name}: not a resource name"))
            .and_then(|k| lib.open_resource(k).map_err(|e| e.to_string()))
            .and_then(|o| subject::show(stage, lib, &o).map_err(|e| e.to_string())),
        What::File(p) => lib
            .open_file(p)
            .map_err(|e| e.to_string())
            .and_then(|o| subject::show(stage, lib, &o).map_err(|e| e.to_string())),
        What::Creature(look) => subject::show_creature(stage, lib, look).map_err(|e| e.to_string()),
    }
    .map(|_| ())
}

/// The manifest (`manifest.json`) and the page (`index.html`).
pub fn write_index(
    out: &Path,
    title: &str,
    shot: &Shot,
    entries: &[Entry],
) -> Result<(), GalleryError> {
    let items: Vec<Value> = entries
        .iter()
        .map(|e| {
            json!({
                "id": e.item.id,
                "label": e.item.label,
                "group": e.item.group,
                "image": e.image,
                "hash": e.hash,
                "info": e.info,
                "error": e.error,
            })
        })
        .collect();
    let manifest = json!({
        "title": title,
        "generator": format!("Moonglow Viewer {}", env!("CARGO_PKG_VERSION")),
        "shot": {
            "size": [shot.size.0, shot.size.1],
            "time": shot.time,
            "fps": shot.fps,
            "view": format!("{:?}", shot.view),
            "yaw": shot.yaw,
            "pitch": shot.pitch,
            "zoom": shot.zoom,
            "plt_colors": shot.plt_colors,
        },
        "items": items,
    });
    let text = serde_json::to_string_pretty(&manifest).expect("JSON values serialise") + "\n";
    let path = out.join("manifest.json");
    std::fs::write(&path, text).map_err(|e| io(&path, e))?;
    let path = out.join("index.html");
    std::fs::write(&path, index_html(title, entries)).map_err(|e| io(&path, e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns() {
        assert!(matches("plc_*", "plc_a01"));
        assert!(matches("PLC_A0?", "plc_a01"));
        assert!(!matches("plc_a0?", "plc_a011"));
        assert!(matches("*golem*", "c_golemerald"));
        assert!(matches("*", ""));
        assert!(!matches("c_*", "plc_a01"));
    }

    #[test]
    fn sources() {
        assert_eq!(Source::parse("2da:placeables").unwrap(), Source::Table(Table::Placeables));
        assert_eq!(Source::parse("appearance.2da").unwrap(), Source::Table(Table::Appearance));
        assert_eq!(Source::parse("plc_*").unwrap(), Source::Pattern("plc_*".into()));
        assert!(Source::parse("2da:nothing").is_err());
        assert!(matches!(Source::parse("folder:/x").unwrap(), Source::Folder(_)));
    }

    #[test]
    fn hashes_are_stable() {
        let mut h = Fnv::new();
        h.add(b"plc_a01");
        assert_eq!(h.0, {
            let mut g = Fnv::new();
            g.add(b"plc_a01");
            g.0
        });
        let mut a = Fnv::new();
        a.add(b"ab");
        a.add(b"c");
        let mut b = Fnv::new();
        b.add(b"a");
        b.add(b"bc");
        assert_ne!(a.0, b.0);
    }
}
