//! What the viewer keeps between runs.

use std::path::{Path, PathBuf};

use std::collections::{BTreeMap, BTreeSet};

use mg_uikit::stack::DockRecord;
pub use mg_uikit::stack::EditorGroup;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DecompileWith {
    /// In process (exact; see `mg_mdl::ascii_write`).
    #[default]
    Native,
    Nwnmdlcomp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CompileWith {
    /// The viewer's own compiler, in process (kept under the name it had
    /// in settings files when it stood for a choice between the others).
    #[default]
    #[serde(alias = "Native")]
    Auto,
    Engine,
    Nwnmdlcomp,
}

impl std::fmt::Display for CompileWith {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            CompileWith::Auto => "Moonglow's compiler",
            CompileWith::Engine => "the game's compiler",
            CompileWith::Nwnmdlcomp => "nwnmdlcomp",
        })
    }
}

/// How the Resources panel shows what it lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ResourcesView {
    /// Names, one a row.
    #[default]
    List,
    /// Pictures in a grid of the kit's cells.
    Gallery,
}

/// What the 3D view draws over the scene.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Overlays {
    /// Walkmeshes, faces coloured by surface material (always on for a
    /// walkmesh opened on its own).
    pub walkmesh: bool,
    pub wireframe: bool,
    pub normals: bool,
    /// Nodes as a tree of lines (bones, hooks, emitters, lights).
    pub skeleton: bool,
    /// The shadow casters (`render 0`, `shadow 1`) in blue in place of the
    /// visible meshes. Not kept between runs.
    #[serde(skip)]
    pub casters: bool,
}

/// The settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// The game folder; `None`: found where Steam installs it.
    pub game_root: Option<PathBuf>,
    /// Read the player's override and development folders.
    pub use_user_dir: bool,
    /// Folders added above the game.
    pub folders: Vec<PathBuf>,
    /// Haks, modules and ERFs added above the game.
    pub haks: Vec<PathBuf>,
    /// Files opened lately, newest first.
    pub recent: Vec<PathBuf>,
    pub show_grid: bool,
    pub show_axes: bool,
    pub overlays: Overlays,
    pub decompile_with: DecompileWith,
    pub compile_with: CompileWith,
    /// Where compiled models of game resources go (others: `compiled/`
    /// beside their file).
    pub output_folder: Option<PathBuf>,
    /// Pause after the last keystroke before the view reloads, ms.
    pub reload_delay_ms: u32,
    /// Draw with the game's own shaders (View › Game shaders); `None`: as
    /// [`mgv_stage::GAME_SHADERS_BY_DEFAULT`] has it, until the switch is
    /// used.
    pub game_shaders: Option<bool>,
    /// The model casts the light's shadow on a ground under it (View ›
    /// Shadow); `None`: on, until the switch is used.
    pub model_shadows: Option<bool>,
    /// The look: dark (the default) or light (View › Theme).
    pub theme: mg_uikit::Theme,
    /// The Resources panel's List or Gallery.
    pub resources_view: ResourcesView,
    /// The galleries' pictures' side, points (the kit's size slider: 64
    /// to 220), shared by the Resources panel's and the Gallery tab's.
    pub gallery_side: f32,
    /// The window's layout as the user left it (View › Reset Layout puts
    /// it back): a layout that does not read, or one of another version,
    /// loads as the default without taking the other settings with it.
    #[serde(deserialize_with = "lenient_layout")]
    pub layout: DockRecord,
}

/// The version of the layout record this viewer writes: 2, the kit's
/// [`DockRecord`] (1 was the viewer's own record, read once and moved
/// over); a record of another version loads as the default.
pub const LAYOUT_VERSION: u32 = 2;

/// The window's layout as the user left it, kept between runs as the
/// kit's [`DockRecord`] (the toolset's way: what the user chose is kept,
/// what the window's width decides is not). Panels and tabs are kept by
/// their names (`Tab::key`), the docks by the kit's (`left`, `right`,
/// `right-2`, `bottom`); a name this viewer does not know is passed by.
///
/// - Which dock a panel was dragged to, where it is not its own.
/// - The docks folded or closed by the user in a window wide enough not
///   to fold them (a narrow window folds them without writing it).
/// - The Right and Bottom Docks' panels in front, where not the first.
/// - Each dock's width or height as its edge was dragged to (`left`,
///   `left-gallery`, `right`, `right-wide`, `bottom`; none: the size the
///   window's width gives); drawn held to what the window allows, kept as
///   chosen for a larger window.
/// - The height of a panel over another (its seam as dragged).
/// - The editor area's groups, where they are not the default of the
///   window's width.
pub fn new_layout() -> DockRecord {
    DockRecord { version: LAYOUT_VERSION, ..DockRecord::default() }
}

/// A kept layout as any version wrote it: version 2's fields (the kit's
/// record) and version 1's (the viewer's own, M6 part D to 0.4).
#[derive(Deserialize)]
#[serde(default)]
#[derive(Default)]
struct AnyLayout {
    version: u32,
    docks: BTreeMap<String, String>,
    folded: BTreeSet<String>,
    fronts: BTreeMap<String, String>,
    sizes: BTreeMap<String, u32>,
    seams: BTreeMap<String, u32>,
    groups: Vec<EditorGroup>,
    // Version 1.
    left_folded: bool,
    right_folded: bool,
    bottom_closed: bool,
    bottom_front: Option<String>,
    right_front: Option<String>,
    left_width: Option<u32>,
    left_gallery_width: Option<u32>,
    right_width: Option<u32>,
    right_wide_width: Option<u32>,
    bottom_height: Option<u32>,
    editor: Vec<EditorGroup>,
}

impl AnyLayout {
    /// The record it comes to: version 2 as it is, version 1 moved over
    /// (its docks' names `Left`, `Right`, `Right 2`, `Bottom` made the
    /// kit's), another version the default.
    fn record(self) -> DockRecord {
        let points = |m: BTreeMap<String, u32>| {
            m.into_iter().map(|(k, v)| (k, v.min(u32::from(u16::MAX)) as u16)).collect()
        };
        match self.version {
            2 => DockRecord {
                version: LAYOUT_VERSION,
                docks: self.docks,
                folded: self.folded,
                fronts: self.fronts,
                sizes: points(self.sizes),
                seams: points(self.seams),
                groups: self.groups,
            },
            1 => {
                let dock = |d: String| match d.as_str() {
                    "Left" => "left".to_string(),
                    "Right" => "right".to_string(),
                    "Right 2" => "right-2".to_string(),
                    "Bottom" => "bottom".to_string(),
                    _ => d,
                };
                let mut r = new_layout();
                r.docks = self.docks.into_iter().map(|(p, d)| (p, dock(d))).collect();
                for (name, folded) in [
                    ("left", self.left_folded),
                    ("right", self.right_folded),
                    ("bottom", self.bottom_closed),
                ] {
                    r.set_folded(name, folded);
                }
                for (name, front) in [("bottom", self.bottom_front), ("right", self.right_front)] {
                    if let Some(front) = front {
                        r.set_front(name, &front);
                    }
                }
                let sizes = [
                    ("left", self.left_width),
                    ("left-gallery", self.left_gallery_width),
                    ("right", self.right_width),
                    ("right-wide", self.right_wide_width),
                    ("bottom", self.bottom_height),
                ];
                r.sizes = points(
                    sizes.into_iter().filter_map(|(n, s)| Some((n.to_string(), s?))).collect(),
                );
                r.seams = points(self.seams);
                r.groups = self.editor;
                r
            }
            _ => new_layout(),
        }
    }
}

/// Reads a layout as it was kept (a version 1 record moved over to the
/// kit's, once), or the default where it does not read (garbage, a type
/// changed) or is of another version: the other settings load all the
/// same.
fn lenient_layout<'de, D: serde::Deserializer<'de>>(d: D) -> Result<DockRecord, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Kept {
        Layout(Box<AnyLayout>),
        Other(serde::de::IgnoredAny),
    }
    Ok(match Kept::deserialize(d)? {
        Kept::Layout(l) => l.record(),
        Kept::Other(_) => new_layout(),
    })
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            game_root: None,
            use_user_dir: true,
            folders: Vec::new(),
            haks: Vec::new(),
            recent: Vec::new(),
            show_grid: true,
            show_axes: false,
            overlays: Overlays::default(),
            decompile_with: DecompileWith::Native,
            compile_with: CompileWith::Auto,
            output_folder: None,
            reload_delay_ms: 150,
            game_shaders: None,
            model_shadows: None,
            theme: mg_uikit::Theme::Dark,
            resources_view: ResourcesView::List,
            gallery_side: mg_uikit::gallery::SIDE_DEFAULT,
            layout: new_layout(),
        }
    }
}

/// How many recent files are kept.
const RECENT: usize = 12;

impl Settings {
    /// Whether the game's own shaders draw.
    pub fn game_shaders(&self) -> bool {
        self.game_shaders.unwrap_or(mgv_stage::GAME_SHADERS_BY_DEFAULT)
    }

    /// Whether the model casts its shadow on a ground under it (on unless
    /// turned off, as in Moonglow Toolset's model viewer).
    pub fn model_shadows(&self) -> bool {
        self.model_shadows.unwrap_or(true)
    }

    /// Puts a file first among the recent ones.
    pub fn remember(&mut self, path: &Path) {
        self.recent.retain(|p| p != path);
        self.recent.insert(0, path.to_path_buf());
        self.recent.truncate(RECENT);
    }
}

/// The viewer's own data folder: `%APPDATA%\Moonglow Viewer`,
/// `~/Library/Application Support/Moonglow Viewer`, or
/// `$XDG_DATA_HOME/moonglow-viewer` (`~/.local/share/moonglow-viewer`).
pub fn data_dir() -> Option<PathBuf> {
    if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("Moonglow Viewer"))
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME")
            .map(|h| PathBuf::from(h).join("Library/Application Support/Moonglow Viewer"))
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
            .map(|d| d.join("moonglow-viewer"))
    }
}

/// The scratch user folder the game's compiler runs in (never the
/// player's).
pub fn scratch_dir() -> PathBuf {
    data_dir().unwrap_or_else(std::env::temp_dir).join("engine-compiler")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The theme choice is kept in the settings, dark until chosen, and a
    /// settings file from before it (no `theme`) still loads.
    #[test]
    fn the_theme_choice_is_kept() {
        use mg_uikit::Theme;
        assert_eq!(Settings::default().theme, Theme::Dark);
        let s = Settings { theme: Theme::Light, ..Settings::default() };
        let text = serde_json::to_string(&s).unwrap();
        assert_eq!(serde_json::from_str::<Settings>(&text).unwrap().theme, Theme::Light);
        let old = serde_json::from_str::<Settings>(r#"{"show_grid":false}"#).unwrap();
        assert_eq!(old.theme, Theme::Dark);
        assert!(!old.show_grid);
    }

    /// List or Gallery and the pictures' side are kept, a list of 128 until
    /// chosen, and a settings file from before them still loads.
    #[test]
    fn the_resources_view_is_kept() {
        let s = Settings {
            resources_view: ResourcesView::Gallery,
            gallery_side: 200.0,
            ..Settings::default()
        };
        let text = serde_json::to_string(&s).unwrap();
        let back = serde_json::from_str::<Settings>(&text).unwrap();
        assert_eq!((back.resources_view, back.gallery_side), (ResourcesView::Gallery, 200.0));
        let old = serde_json::from_str::<Settings>(r#"{"show_grid":false}"#).unwrap();
        assert_eq!(old.resources_view, ResourcesView::List);
        assert_eq!(old.gallery_side, mg_uikit::gallery::SIDE_DEFAULT);
    }

    /// A layout as the user left it, with something in every field.
    pub(crate) fn a_layout() -> DockRecord {
        let mut r = new_layout();
        r.set_dock("log", Some("right"));
        r.set_folded("left", true);
        r.set_folded("bottom", true);
        r.set_front("bottom", "effects");
        r.set_front("right", "lighting");
        r.set_size("left", 333.0);
        r.set_size("right-wide", 900.0);
        r.set_size("bottom", 222.0);
        r.keep_seam("resources", 400.0);
        r.groups = vec![
            EditorGroup { tabs: vec!["view".into()], front: 0, share: 0.5 },
            EditorGroup { tabs: vec!["texture".into(), "ascii".into()], front: 0, share: 0.5 },
        ];
        r
    }

    /// The layout is kept as eframe keeps the settings (RON) and as JSON;
    /// a file from before it, one whose layout is garbage or of another
    /// version, loads the layout's default and every other setting.
    #[test]
    fn the_layout_is_kept_and_garbage_loads_the_default() {
        let s = Settings { layout: a_layout(), show_grid: false, ..Settings::default() };
        let ron_text = ron::to_string(&s).unwrap();
        assert_eq!(ron::from_str::<Settings>(&ron_text).unwrap(), s);
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(serde_json::from_str::<Settings>(&json).unwrap(), s);
        // From before the layout was kept.
        let old = serde_json::from_str::<Settings>(r#"{"show_grid":false}"#).unwrap();
        assert_eq!(old.layout, new_layout());
        let old: Settings = ron::from_str("(show_grid: false)").unwrap();
        assert_eq!(old.layout, new_layout());
        assert!(!old.show_grid);
        // Garbage, a type changed, another version: the default, and the
        // other settings as they were.
        let recent = r#""recent":["/m/a.mdl"],"show_grid":false"#;
        for layout in [
            r#""layout":17"#,
            r#""layout":"garbage""#,
            r#""layout":{"sizes":{"left":"wide"},"docks":[1,2]}"#,
            r#""layout":{"version":99,"sizes":{"left":400}}"#,
            r#""layout":[null,{"a":true}]"#,
        ] {
            let text = format!("{{{recent},{layout}}}");
            let s: Settings = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{text}: {e}"));
            assert_eq!(s.layout, new_layout(), "{text}");
            assert_eq!(s.recent, [PathBuf::from("/m/a.mdl")], "{text}");
            assert!(!s.show_grid);
        }
        for layout in [
            "layout: 17",
            "layout: \"garbage\"",
            "layout: (sizes: {\"left\": \"wide\"}, docks: [1, 2])",
            "layout: (version: 99, sizes: {\"left\": 400})",
            "layout: Some((version: 2))",
            "layout: [(), {\"a\": true}]",
        ] {
            let text = format!("(recent: [\"/m/a.mdl\"], show_grid: false, {layout})");
            let s: Settings = ron::from_str(&text).unwrap_or_else(|e| panic!("{text}: {e}"));
            assert_eq!(s.recent, [PathBuf::from("/m/a.mdl")], "{text}");
            assert!(!s.show_grid, "{text}");
            assert_eq!(s.layout, new_layout(), "{text}");
        }
        // A layout written by RON with every field set reads back through
        // the lenient reader whole (RON's maps survive the untagged
        // buffering).
        let only = ron::to_string(&a_layout()).unwrap();
        let text = format!("(layout: {only})");
        assert_eq!(ron::from_str::<Settings>(&text).unwrap().layout, a_layout());
    }

    /// A layout as M6 part D to 0.4 wrote it (version 1, the viewer's own
    /// record, RON captured from that viewer with every field set) is read
    /// once and moved over to the kit's `DockRecord` (version 2): its docks
    /// named as the kit names them, and written as that from then on.
    #[test]
    fn a_part_d_layout_moves_to_the_kit_s_record() {
        const PART_D: &str = r#"(version:1,docks:{"log":"Right"},left_folded:true,right_folded:true,bottom_closed:true,bottom_front:Some("effects"),right_front:Some("lighting"),left_width:Some(333),left_gallery_width:Some(440),right_width:Some(352),right_wide_width:Some(900),bottom_height:Some(222),seams:{"resources":400},editor:[(tabs:["view"],front:0,share:0.5),(tabs:["texture","ascii"],front:0,share:0.5)])"#;
        let s: Settings = ron::from_str(&format!("(show_grid: false, layout: {PART_D})")).unwrap();
        let mut want = a_layout();
        want.set_folded("right", true);
        want.set_size("left-gallery", 440.0);
        want.set_size("right", 352.0);
        assert_eq!(s.layout, want);
        assert!(!s.show_grid);
        let written = ron::to_string(&s.layout).unwrap();
        assert!(written.starts_with("(version:2,"), "{written}");
        assert_eq!(
            ron::from_str::<Settings>(&format!("(layout: {written})")).unwrap().layout,
            want
        );
    }

    #[test]
    fn recent_files() {
        let mut s = Settings::default();
        for i in 0..20 {
            s.remember(Path::new(&format!("/m/{i}.mdl")));
        }
        s.remember(Path::new("/m/5.mdl"));
        assert_eq!(s.recent.len(), RECENT);
        assert_eq!(s.recent[0], Path::new("/m/5.mdl"));
        assert_eq!(s.recent.iter().filter(|p| p.ends_with("5.mdl")).count(), 1);
    }
}
