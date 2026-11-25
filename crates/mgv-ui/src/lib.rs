//! Moonglow Viewer's window: a 3D view of what is open, the ASCII editor
//! beside it, the resource browser, the node outliner, the inspector, the
//! animation timeline, the texture view and the log, docked.
//!
//! The window only shows state and turns input into [`Action`]s; the work
//! is done by the UI-free crates (`mgv-library`, `mgv-stage`, `mgv-mdl`), as
//! the command line does it.

pub mod code;
pub mod editor;
mod effects;
mod jobs;
mod lighting;
pub mod manual;
pub mod palette;
mod panels;
pub mod settings;
pub mod shell;
mod texture;
pub mod view3d;

use std::path::{Path, PathBuf};

use egui::{Ui, WidgetText};
use egui_dock::{DockState, NodeIndex, TabViewer};
use mg_core::ResType;
use mg_resman::{GameInstall, ResKey};
use mgv_library::watch::Watcher;
use mgv_library::{Kind, Library, Opened};
use mgv_stage::camera::View;
use mgv_stage::subject::Shown;

pub use editor::Buffer;
pub use settings::Settings;

/// Native dialogs, behind a trait so tests can answer them.
pub trait Dialogs {
    fn open_file(&mut self, title: &str, filters: &[(&str, &[&str])]) -> Option<PathBuf>;
    fn save_file(&mut self, title: &str, suggested: &Path) -> Option<PathBuf>;
    fn pick_folder(&mut self, title: &str) -> Option<PathBuf>;
}

/// No dialogs (tests): every question is cancelled.
#[derive(Debug, Default)]
pub struct NoDialogs;

impl Dialogs for NoDialogs {
    fn open_file(&mut self, _: &str, _: &[(&str, &[&str])]) -> Option<PathBuf> {
        None
    }
    fn save_file(&mut self, _: &str, _: &Path) -> Option<PathBuf> {
        None
    }
    fn pick_folder(&mut self, _: &str) -> Option<PathBuf> {
        None
    }
}

/// What the window can be asked to do.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Ask for a file and open it.
    OpenDialog,
    Open(PathBuf),
    OpenResource(ResKey),
    /// A creature by `appearance.2da` row, bare.
    OpenCreature(mgv_stage::subject::CreatureLook),
    /// The PLT colours of the model shown on its own (`None`: as opened).
    SetColors(Option<[u8; 10]>),
    /// Applies a `visualeffects.2da` row to what is shown.
    ApplyEffect(usize),
    /// Ends an applied effect (its cessation plays).
    RemoveEffect(usize),
    /// Takes every effect off.
    ClearEffects,
    /// Decompile what is shown into the editor.
    Decompile,
    /// Compile the editor's model (and show the result).
    Compile {
        view_result: bool,
    },
    Save,
    SaveAs,
    /// Close the editor's buffer (discarding unsaved edits).
    CloseBuffer,
    Frame,
    SetView(View),
    AddFolderDialog,
    AddArchiveDialog,
    GameFolderDialog,
    /// Re-read everything from disk.
    Refresh,
    ResetLayout,
    Quit,
}

/// The window's tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tab {
    View,
    Editor,
    Texture,
    Browser,
    Outliner,
    Inspector,
    Timeline,
    Effects,
    Lighting,
    Log,
}

impl Tab {
    fn title(self) -> &'static str {
        match self {
            Tab::View => "3D View",
            Tab::Editor => "ASCII",
            Tab::Texture => "Texture",
            Tab::Browser => "Resources",
            Tab::Outliner => "Outliner",
            Tab::Inspector => "Inspector",
            Tab::Timeline => "Timeline",
            Tab::Effects => "Effects",
            Tab::Lighting => "Lighting",
            Tab::Log => "Log",
        }
    }

    /// Its icon: a document tab's kind icon, a dock tab's and a folded
    /// dock's strip's.
    pub fn icon(self) -> mg_uikit::Icon {
        use mg_uikit::Icon;
        match self {
            Tab::View => Icon::Model,
            Tab::Editor => Icon::Ascii,
            Tab::Texture => Icon::Texture,
            Tab::Browser => Icon::FolderOpen,
            Tab::Outliner => Icon::Outliner,
            Tab::Inspector => Icon::Inspector,
            Tab::Timeline => Icon::Timeline,
            Tab::Effects => Icon::Sparkles,
            Tab::Lighting => Icon::Sun,
            Tab::Log => Icon::Log,
        }
    }

    /// A document tab the user closes (the 3D View stays).
    pub(crate) fn closeable(self) -> bool {
        matches!(self, Tab::Editor | Tab::Texture)
    }
}

/// A log line.
#[derive(Debug, Clone, PartialEq)]
pub struct LogLine {
    pub level: Level,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Info,
    Warning,
    Error,
}

/// What is open.
#[derive(Debug)]
pub struct Doc {
    pub opened: Opened,
    /// What went on the stage (3D kinds).
    pub shown: Option<Shown>,
    /// A creature by appearance row (then `opened` holds nothing).
    pub creature: Option<mgv_stage::subject::CreatureLook>,
}

impl Doc {
    pub fn name(&self) -> String {
        match &self.creature {
            Some(c) => format!("appearance {}", c.appearance),
            None => self.opened.name(),
        }
    }

    /// The `SIZECATEGORY` effects use (a creature's, else medium).
    pub fn size(&self, lib: &Library) -> u32 {
        self.creature.map_or(3, |c| mgv_stage::vfx::size_of(lib, c.appearance.into()))
    }

    /// The model key the editor's buffer stands for.
    fn model_key(&self) -> Option<ResKey> {
        self.opened.key.filter(|k| k.restype == ResType::MDL)
    }
}

/// The selected node: an actor and one of its model's nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub actor: mgv_stage::ActorId,
    pub node: usize,
}

/// The viewer.
pub struct Viewer {
    pub settings: Settings,
    pub lib: Library,
    pub watcher: Watcher,
    pub gfx: Option<view3d::Gfx>,
    pub doc: Option<Doc>,
    pub camera: mgv_stage::OrbitCamera,
    pub selection: Option<Selection>,
    pub buffer: Option<Buffer>,
    pub texture: Option<texture::TextureView>,
    pub browser: panels::Browser,
    pub effects: effects::Effects,
    /// The game's palettes, for the colour choosers.
    pub palettes: palette::Palettes,
    pub rig: lighting::Rig,
    pub manual: manual::Manual,
    pub log: Vec<LogLine>,
    pub actions: Vec<Action>,
    /// The editor area's document tabs (3D View, Texture, ASCII).
    pub dock: DockState<Tab>,
    /// The docks around it, the bars and what the window's width yields.
    pub shell: shell::Shell,
    pub dialogs: Box<dyn Dialogs>,
    pub jobs: jobs::Jobs,
    pub quit_requested: bool,
    /// The window's clock at the last frame.
    clock: Option<f64>,
    /// Frame the camera after the next step (bounds need a pose).
    frame_pending: bool,
    /// The editor's cursor line asked for (jump after a pick).
    pub(crate) jump_to_line: Option<usize>,
    /// The game's shaders already said to be drawn with Moonglow's own.
    shader_failures_told: Vec<String>,
}

impl std::fmt::Debug for Viewer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Viewer").field("doc", &self.doc).finish_non_exhaustive()
    }
}

/// What the Game Shaders switch does.
pub(crate) const SHADERS_HELP: &str = "Draw with the game's own shaders, compiled from the game and \
     the haks added: their replacements of the game's, and the custom shaders materials name \
     (swaying grass and leaves, flowing water). Off, or where one does not compile: Moonglow's own \
     shader";

/// What the Shadow switch does.
pub(crate) const SHADOW_HELP: &str = "The model casts the light's shadow on a ground under it, the \
     ground itself unseen (as an area's ground takes a creature's shadow)";

/// The editor area's layout (proposal section 9.17): the 3D View in one
/// group, Texture and ASCII (in front) in a second beside it. The docks'
/// panels are not in it: they stand where [`shell::place`] says.
pub fn default_dock() -> DockState<Tab> {
    default_dock_for(false)
}

/// [`default_dock`], or in the wide layout (section 5.3, a window from
/// 2560 wide) the 3D View, Texture and ASCII in three groups side by side.
pub fn default_dock_for(wide: bool) -> DockState<Tab> {
    let mut dock = DockState::new(vec![Tab::View]);
    let surface = dock.main_surface_mut();
    if wide {
        let [_, rest] = surface.split_right(NodeIndex::root(), 0.4, vec![Tab::Texture]);
        surface.split_right(rest, 0.4, vec![Tab::Editor]);
    } else {
        surface.split_right(NodeIndex::root(), 0.55, vec![Tab::Texture, Tab::Editor]);
        if let Some(at) = dock.find_tab(&Tab::Editor) {
            let _ = dock.set_active_tab(at);
        }
    }
    dock
}

impl Viewer {
    pub fn new(settings: Settings, dialogs: Box<dyn Dialogs>) -> Viewer {
        let mut log = Vec::new();
        let lib = open_library(&settings, &mut log);
        let mut v = Viewer {
            settings,
            lib,
            watcher: Watcher::new(),
            gfx: None,
            doc: None,
            camera: mgv_stage::OrbitCamera::default(),
            selection: None,
            buffer: None,
            texture: None,
            browser: panels::Browser::default(),
            effects: effects::Effects::default(),
            palettes: palette::Palettes::default(),
            rig: lighting::Rig::default(),
            manual: manual::Manual::default(),
            log,
            actions: Vec::new(),
            dock: default_dock(),
            shell: shell::Shell::default(),
            dialogs,
            jobs: jobs::Jobs::default(),
            quit_requested: false,
            clock: None,
            frame_pending: false,
            jump_to_line: None,
            shader_failures_told: Vec::new(),
        };
        v.add_settings_layers();
        v
    }

    /// Gives the viewer the window's GPU (eframe's wgpu state).
    pub fn set_render_state(&mut self, rs: egui_wgpu::RenderState) {
        self.gfx = Some(view3d::Gfx::new(rs));
    }

    pub fn info(&mut self, text: impl Into<String>) {
        self.log.push(LogLine { level: Level::Info, text: text.into() });
    }

    /// Says once each which of the game's shaders are drawn with
    /// Moonglow's own instead, and why (told again after they are
    /// forgotten: the library read again).
    pub(crate) fn tell_shader_failures(&mut self) {
        let Some(g) = &self.gfx else { return };
        let failures = g.viewport.shader_failures().to_vec();
        if failures.is_empty() {
            self.shader_failures_told.clear();
        }
        for (what, why) in failures {
            if !self.shader_failures_told.contains(&what) {
                self.warn(format!(
                    "The game's shader {what} is drawn with Moonglow's own shader instead: {why}"
                ));
                self.shader_failures_told.push(what);
            }
        }
    }

    pub fn warn(&mut self, text: impl Into<String>) {
        self.log.push(LogLine { level: Level::Warning, text: text.into() });
    }

    pub fn error(&mut self, text: impl Into<String>) {
        self.log.push(LogLine { level: Level::Error, text: text.into() });
    }

    /// The window title.
    pub fn title(&self) -> String {
        let dirty = self.buffer.as_ref().is_some_and(Buffer::is_dirty);
        match &self.doc {
            Some(d) => format!("{}{} — Moonglow Viewer", d.name(), if dirty { " *" } else { "" }),
            None => "Moonglow Viewer".into(),
        }
    }

    fn add_settings_layers(&mut self) {
        for f in self.settings.folders.clone() {
            self.lib.add_folder(&f);
            self.watcher.watch(&f);
        }
        for h in self.settings.haks.clone() {
            if let Err(e) = self.lib.add_archive(&h) {
                self.error(format!("{e}"));
            }
        }
    }

    /// Opens a file from disk.
    pub fn open(&mut self, path: &Path) {
        if !self.confirm_discard() {
            return;
        }
        match self.lib.open_file(path) {
            Ok(opened) => {
                self.settings.remember(path);
                self.watcher.watch(path);
                self.show(opened);
            }
            Err(e) => self.error(format!("{e}")),
        }
    }

    /// Opens a resource of the game (or an added folder or archive).
    pub fn open_resource(&mut self, key: ResKey) {
        if !self.confirm_discard() {
            return;
        }
        // The opened file's folder stays when the resource is one of its
        // files (the browser lists them).
        if !self.lib.from_opened_folder(&key) {
            self.lib.set_opened_folder(None);
        }
        match self.lib.open_resource(key) {
            Ok(opened) => self.show(opened),
            Err(e) => self.error(format!("{e}")),
        }
    }

    /// Shows a creature by `appearance.2da` row.
    pub fn open_creature(&mut self, look: mgv_stage::subject::CreatureLook) {
        if !self.confirm_discard() {
            return;
        }
        self.close_buffer();
        self.selection = None;
        self.texture = None;
        self.effects.applied.clear();
        let Some(g) = &mut self.gfx else {
            self.warn("No GPU: the 3D view is off.");
            return;
        };
        match mgv_stage::subject::show_creature(&mut g.stage, &self.lib, &look) {
            Ok(shown) => {
                let opened = Opened {
                    path: None,
                    kind: Kind::Blueprint,
                    key: None,
                    data: std::sync::Arc::from(Vec::new()),
                };
                for m in &shown.missing {
                    self.log.push(LogLine {
                        level: Level::Warning,
                        text: format!("model {m} not found"),
                    });
                }
                self.doc = Some(Doc { opened, shown: Some(shown), creature: Some(look) });
                self.info(format!("Opened appearance {}", look.appearance));
                self.frame_pending = true;
                self.focus(Tab::View);
            }
            Err(e) => self.error(format!("{e}")),
        }
    }

    /// Applies a visual effect to what is shown.
    fn apply_effect(&mut self, row: usize) {
        let (Some(doc), Some(g)) = (&self.doc, &mut self.gfx) else { return };
        let Some(base) = doc.shown.as_ref().map(|s| s.base) else {
            self.warn("Show a model or creature first.");
            return;
        };
        let Some(effect) = mgv_stage::vfx::row(&self.lib, row) else { return };
        let size = self.effects.size.unwrap_or_else(|| doc.size(&self.lib));
        match mgv_stage::vfx::apply(&mut g.stage, &self.lib, base, &effect, size) {
            Ok(applied) => {
                for m in &applied.missing {
                    self.log.push(LogLine {
                        level: Level::Warning,
                        text: format!("{}: model {m} not found", effect.label),
                    });
                }
                if applied.actors.is_empty() {
                    self.warn(format!("{} has no model for this size", effect.label));
                }
                self.effects.applied.push(applied);
            }
            Err(e) => self.error(format!("{e}")),
        }
    }

    /// Unsaved edits are kept unless the user lets them go (the editor's
    /// tab shows them; nothing is lost silently).
    fn confirm_discard(&mut self) -> bool {
        if self.buffer.as_ref().is_some_and(Buffer::is_dirty) {
            self.warn("Save or close the edited model first (Model › Close Edits discards them).");
            return false;
        }
        true
    }

    /// Shows something opened: on the stage, in the texture view, or (an
    /// ASCII model) in the editor too.
    fn show(&mut self, opened: Opened) {
        self.close_buffer();
        self.selection = None;
        self.texture = None;
        self.effects.applied.clear();
        let name = opened.name();
        let source = opened.path.as_ref().map_or_else(
            || self.lib.origin(&opened.key.expect("a resource")).unwrap_or("?").to_string(),
            |p| p.display().to_string(),
        );
        self.info(format!("Opened {name} ({source})"));
        match opened.kind {
            k if k.is_3d() => {
                let shown = match &mut self.gfx {
                    Some(g) => match mgv_stage::subject::show(&mut g.stage, &self.lib, &opened) {
                        Ok(s) => Some(s),
                        Err(e) => {
                            self.error(format!("{e}"));
                            None
                        }
                    },
                    None => {
                        self.warn("No GPU: the 3D view is off.");
                        None
                    }
                };
                if let Some(s) = &shown {
                    for m in &s.missing {
                        self.warn(format!("{name}: model {m} not found"));
                    }
                }
                let ascii_model = k == Kind::Model && !mg_mdl::is_binary(&opened.data);
                self.doc = Some(Doc { opened, shown, creature: None });
                if ascii_model {
                    self.edit_source();
                }
                self.frame_pending = true;
                self.focus(Tab::View);
            }
            Kind::Texture | Kind::Plt => {
                self.texture = Some(texture::TextureView::new(&self.lib, &opened));
                self.doc = Some(Doc { opened, shown: None, creature: None });
                self.focus(Tab::Texture);
            }
            Kind::Tileset => {
                self.doc = Some(Doc { opened, shown: None, creature: None });
                self.info(format!(
                    "{name}: its tiles are listed in the Inspector; click one to show it"
                ));
                self.focus(Tab::Inspector);
            }
            Kind::Txi | Kind::TwoDa | Kind::Other | Kind::Archive => {
                self.doc = Some(Doc { opened, shown: None, creature: None });
                self.warn(format!("{name}: nothing to show in 3D (yet)"));
            }
            _ => self.doc = Some(Doc { opened, shown: None, creature: None }),
        }
    }

    /// Opens the shown ASCII model's text in the editor.
    fn edit_source(&mut self) {
        let Some(doc) = &self.doc else { return };
        let text = String::from_utf8_lossy(&doc.opened.data).into_owned();
        let buffer = Buffer::new(text, doc.opened.path.clone(), doc.model_key(), false);
        self.buffer = Some(buffer);
        self.focus(Tab::Editor);
    }

    /// Makes a tab visible: a dock's panel brought into sight in its
    /// dock; a document tab put in front of its group, or opened again
    /// where it was closed (Texture and ASCII beside each other, in a
    /// group of their own beside the 3D View).
    pub fn focus(&mut self, tab: Tab) {
        if shell::place(tab, self.shell.yields.wide) != shell::Place::Editor {
            self.shell.show_panel(tab);
            return;
        }
        if let Some(at) = self.dock.find_tab(&tab) {
            let _ = self.dock.set_active_tab(at);
            return;
        }
        let partner = match tab {
            Tab::Editor => self.dock.find_tab(&Tab::Texture),
            Tab::Texture => self.dock.find_tab(&Tab::Editor),
            _ => None,
        };
        match (partner, self.dock.find_tab(&Tab::View)) {
            (Some(at), _) => {
                let path = egui_dock::NodePath { surface: at.surface, node: at.node };
                self.dock.set_focused_node_and_surface(path);
                self.dock.push_to_focused_leaf(tab);
            }
            (None, Some(view)) if view.surface.is_main() => {
                self.dock.main_surface_mut().split_right(view.node, 0.55, vec![tab]);
            }
            _ => self.dock.push_to_first_leaf(tab),
        }
    }

    /// Closes a document tab (Texture or ASCII; its document stays open).
    pub fn close_tab(&mut self, tab: Tab) {
        if tab.closeable()
            && let Some(at) = self.dock.find_tab(&tab)
        {
            self.dock.remove_tab(at);
        }
    }

    /// Whether Save (the menu's and the Top Bar's) can be chosen.
    pub fn can_save(&self) -> bool {
        self.buffer.is_some()
    }

    /// Whether a document tab shows the modified dot: the ASCII while it
    /// has unsaved edits.
    pub fn tab_modified(&self, tab: Tab) -> bool {
        tab == Tab::Editor && self.buffer.as_ref().is_some_and(Buffer::is_dirty)
    }

    /// Drops the editor's buffer and the library's copy of it.
    fn close_buffer(&mut self) {
        if let Some(b) = self.buffer.take()
            && let Some(k) = b.key
            && self.lib.buffer(&k).is_some()
        {
            self.lib.set_buffer(k, None);
            if let Some(g) = &mut self.gfx {
                g.stage.reload(&self.lib);
            }
        }
    }

    /// Runs queued actions.
    pub fn run_actions(&mut self) {
        while !self.actions.is_empty() {
            let action = self.actions.remove(0);
            self.run(action);
        }
    }

    fn run(&mut self, action: Action) {
        match action {
            Action::OpenDialog => {
                let filters: &[(&str, &[&str])] = &[
                    (
                        "Models and textures",
                        &[
                            "mdl", "wok", "pwk", "dwk", "tga", "dds", "plt", "mtr", "utc", "utp",
                            "utd", "uti",
                        ],
                    ),
                    ("All files", &["*"]),
                ];
                if let Some(p) = self.dialogs.open_file("Open", filters) {
                    self.open(&p);
                }
            }
            Action::Open(p) => self.open(&p),
            Action::OpenResource(k) => self.open_resource(k),
            Action::OpenCreature(look) => self.open_creature(look),
            Action::SetColors(colors) => {
                if let (Some(g), Some(base)) = (
                    &mut self.gfx,
                    self.doc.as_ref().and_then(|d| d.shown.as_ref()).map(|s| s.base),
                ) && let Some(a) = g.stage.actor_mut(base)
                {
                    a.look.colors = colors;
                }
            }
            Action::ApplyEffect(row) => self.apply_effect(row),
            Action::RemoveEffect(i) => {
                if let (Some(a), Some(g)) = (self.effects.applied.get(i), &mut self.gfx) {
                    mgv_stage::vfx::remove(&mut g.stage, a);
                    self.effects.applied.remove(i);
                }
            }
            Action::ClearEffects => {
                if let Some(g) = &mut self.gfx {
                    for a in self.effects.applied.drain(..) {
                        for id in a.actors {
                            if let Some(actor) = g.stage.actor_mut(id) {
                                actor.visible = false;
                            }
                        }
                    }
                }
            }
            Action::Decompile => self.decompile(),
            Action::Compile { view_result } => self.compile(view_result),
            Action::Save => self.save(false),
            Action::SaveAs => self.save(true),
            Action::CloseBuffer => {
                if let Some(doc) = &self.doc
                    && doc.opened.kind == Kind::Model
                    && !mg_mdl::is_binary(&doc.opened.data)
                {
                    // An ASCII model's edits are dropped; its text stays.
                    self.close_buffer();
                    self.edit_source();
                } else {
                    self.close_buffer();
                }
            }
            Action::Frame => self.frame_pending = true,
            Action::SetView(v) => {
                self.camera.set_view(v);
                self.frame_pending = true;
            }
            Action::AddFolderDialog => {
                if let Some(f) = self.dialogs.pick_folder("Add a folder of models and textures") {
                    self.lib.add_folder(&f);
                    self.watcher.watch(&f);
                    if !self.settings.folders.contains(&f) {
                        self.settings.folders.push(f);
                    }
                    self.browser.invalidate();
                }
            }
            Action::AddArchiveDialog => {
                let filters: &[(&str, &[&str])] =
                    &[("Haks, modules and ERFs", &["hak", "mod", "erf", "nwm"])];
                if let Some(f) = self.dialogs.open_file("Add a hak, module or ERF", filters) {
                    match self.lib.add_archive(&f) {
                        Ok(()) => {
                            if !self.settings.haks.contains(&f) {
                                self.settings.haks.push(f);
                            }
                            self.browser.invalidate();
                        }
                        Err(e) => self.error(format!("{e}")),
                    }
                }
            }
            Action::GameFolderDialog => {
                if let Some(f) = self.dialogs.pick_folder("The Neverwinter Nights folder") {
                    if !GameInstall::is_install(&f) {
                        self.error(format!(
                            "{}: not a game folder (no data/nwn_base.key)",
                            f.display()
                        ));
                        return;
                    }
                    self.settings.game_root = Some(f);
                    self.lib = open_library(&self.settings, &mut self.log);
                    self.add_settings_layers();
                    self.browser.invalidate();
                }
            }
            Action::Refresh => self.refresh(),
            Action::ResetLayout => {
                self.dock = default_dock_for(self.shell.yields.wide);
                self.shell.reset();
            }
            Action::Quit => {
                if self.confirm_discard() {
                    self.quit_requested = true;
                }
            }
        }
    }

    /// Re-reads folders and models from disk.
    fn refresh(&mut self) {
        self.lib.rescan();
        self.browser.invalidate();
        self.palettes.clear();
        if let Some(g) = &mut self.gfx {
            g.viewport.clear_textures();
            let n = g.stage.reload(&self.lib);
            self.info(format!("Reloaded ({n} models changed)"));
        }
    }

    /// The whole window, once a frame.
    pub fn ui(&mut self, ui: &mut Ui) {
        // (The suite's look: fonts, style and tokens; cheap once applied.)
        mg_uikit::apply(ui.ctx(), self.settings.theme);
        // What the window's width yields (the editor area's as it was at
        // the last frame); the editor area laid out for it at the first.
        let window = ui.max_rect().width();
        self.shell.frame_yields(window, self.shell.editor_width.unwrap_or(window));
        if self.shell.laid_out.is_none() {
            let wide = self.shell.yields.wide;
            if wide {
                self.dock = default_dock_for(true);
            }
            self.shell.laid_out = Some(wide);
        }
        self.shortcuts(ui);
        self.watch_files(ui.input(|i| i.time));
        self.jobs_done();
        self.tick(ui);

        // The bars, the side docks the whole height between them, the
        // Bottom Dock under the editor area (proposal section 5).
        shell::top_bar(self, ui);
        shell::status_bar(self, ui);
        shell::left_dock(self, ui);
        shell::right_dock(self, ui);
        shell::bottom_dock(self, ui);
        shell::editor_area(self, ui);
        self.shell.end_frame();
        manual::windows(self, ui.ctx());
        self.run_actions();
        self.tell_shader_failures();
        if self.jobs.busy() || self.watcher.busy() {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    /// Advances the stage by the frame time; frames the camera when asked.
    fn tick(&mut self, ui: &Ui) {
        let now = ui.input(|i| i.time);
        let dt = self.clock.map_or(0.0, |c| (now - c) as f32).clamp(0.0, 0.25);
        self.clock = Some(now);
        if let Some(buffer) = &mut self.buffer
            && buffer.reload_due(now)
        {
            self.apply_buffer();
        }
        lighting::update(self);
        let Some(g) = &mut self.gfx else { return };
        g.stage.shadow = self.settings.model_shadows();
        g.stage.step(&self.lib, dt);
        // A model facing elsewhere: the camera keeps its angle to the front.
        if g.stage.front != self.camera.front {
            self.camera.set_front(g.stage.front);
        }
        if self.frame_pending {
            self.frame_pending = false;
            if let Some((min, max)) = g.stage.bounds_with_particles() {
                self.camera.frame(min, max);
            }
        }
        if g.stage.animating() {
            ui.ctx().request_repaint();
        }
    }

    /// Selects a node (`None`: nothing). From the view or the outliner,
    /// the editor follows to the node's block.
    pub fn select(&mut self, sel: Option<Selection>, from_editor: bool) {
        self.selection = sel;
        if from_editor {
            return;
        }
        let (Some(sel), Some(g), Some(buffer)) = (sel, &self.gfx, &self.buffer) else { return };
        // Only the base model's nodes are in the editor's text.
        if self.doc.as_ref().and_then(|d| d.shown.as_ref()).is_none_or(|s| s.base != sel.actor) {
            return;
        }
        let Some(a) = g.stage.actor(sel.actor) else { return };
        let name = &a.model.model.nodes[sel.node].name;
        if let Some(line) = buffer.node_line(sel.node, name) {
            self.jump_to_line = Some(line);
        } else if let Some(n) = buffer.outline.find_node(name, None) {
            self.jump_to_line = Some(n.lines.start);
        }
    }

    /// Selects the base model's node `index` (from the editor: the text's
    /// node there), when the model on the stage has it by that name; else
    /// the node of that name.
    pub fn select_node_at(&mut self, index: usize, name: &str) {
        let Some(base) = self.doc.as_ref().and_then(|d| d.shown.as_ref()).map(|s| s.base) else {
            return;
        };
        let same = self
            .gfx
            .as_ref()
            .and_then(|g| g.stage.actor(base))
            .and_then(|a| a.model.model.nodes.get(index))
            .is_some_and(|n| n.name.eq_ignore_ascii_case(name));
        if same {
            self.select(Some(Selection { actor: base, node: index }), true);
        } else {
            self.select_node_named(name);
        }
    }

    /// Selects the base model's node of this name (from the editor).
    pub fn select_node_named(&mut self, name: &str) {
        let Some(base) = self.doc.as_ref().and_then(|d| d.shown.as_ref()).map(|s| s.base) else {
            return;
        };
        let node = self
            .gfx
            .as_ref()
            .and_then(|g| g.stage.actor(base))
            .and_then(|a| a.model.model.node(name));
        if let Some(node) = node {
            self.select(Some(Selection { actor: base, node }), true);
        }
    }

    /// Puts the editor's text on the stage (hot reload) and checks it.
    pub fn apply_buffer(&mut self) {
        let Some(buffer) = &mut self.buffer else { return };
        buffer.refresh_analysis();
        let text = buffer.text();
        match buffer.key {
            Some(key) => {
                self.lib.set_buffer(key, Some(text.into_bytes().into()));
                if let Some(g) = &mut self.gfx {
                    g.stage.reload(&self.lib);
                }
            }
            None => {
                let base = self.doc.as_ref().and_then(|d| d.shown.as_ref()).map(|s| s.base);
                if let (Some(g), Some(base)) = (&mut self.gfx, base) {
                    match mg_mdl::Model::read(text.as_bytes()) {
                        Ok(m) => g.stage.replace_model(&self.lib, base, std::sync::Arc::new(m)),
                        Err(e) => {
                            let msg = format!("Not reloaded: {e}");
                            self.warn(msg);
                        }
                    }
                }
            }
        }
        // A node that no longer exists cannot stay selected.
        if let (Some(sel), Some(g)) = (self.selection, &self.gfx)
            && g.stage.actor(sel.actor).is_none_or(|a| sel.node >= a.model.model.nodes.len())
        {
            self.selection = None;
        }
    }

    /// Files changed by other programs.
    fn watch_files(&mut self, _now: f64) {
        let changed = self.watcher.poll(std::time::Instant::now());
        if changed.is_empty() {
            return;
        }
        let mut library_changed = false;
        for path in changed {
            let is_buffer = self
                .buffer
                .as_ref()
                .and_then(|b| b.path.as_deref())
                .is_some_and(|p| same_file(p, &path));
            if is_buffer {
                let Ok(bytes) = std::fs::read(&path) else { continue };
                let text = String::from_utf8_lossy(&bytes).into_owned();
                let buffer = self.buffer.as_mut().expect("checked");
                if buffer.matches_disk(&text) {
                    continue;
                }
                if buffer.is_dirty() {
                    buffer.disk_changed = true;
                    self.warn(format!(
                        "{} changed on disk; it has unsaved edits here",
                        path.display()
                    ));
                } else {
                    buffer.replace_from_disk(text);
                    self.info(format!("Reloaded {} (changed on disk)", path.display()));
                    self.apply_buffer();
                }
            }
            if self.lib.reads_from(&path) {
                library_changed = true;
            }
        }
        if library_changed {
            self.lib.rescan();
            self.browser.invalidate();
            if let Some(g) = &mut self.gfx {
                g.viewport.clear_textures();
                g.stage.reload(&self.lib);
            }
            if let Some(t) = &mut self.texture
                && let Some(doc) = &self.doc
                && let Some(p) = &doc.opened.path
                && let Ok(data) = std::fs::read(p)
            {
                let mut o = doc.opened.clone();
                o.data = data.into();
                *t = texture::TextureView::new(&self.lib, &o);
            }
        }
    }

    /// Decompiles what is shown into the editor (an unsaved buffer that the
    /// view shows at once).
    fn decompile(&mut self) {
        let Some(doc) = &self.doc else {
            self.warn("Open a model first.");
            return;
        };
        if doc.opened.kind != Kind::Model || !mg_mdl::is_binary(&doc.opened.data) {
            self.warn("Only compiled models decompile.");
            return;
        }
        let name = doc.name();
        let key = doc.model_key();
        let suggested = doc.opened.path.as_ref().map(|p| p.with_extension("ascii.mdl"));
        let text = match self.settings.decompile_with {
            settings::DecompileWith::Native => {
                mg_mdl::decompile(&doc.opened.data).map_err(|e| e.to_string())
            }
            settings::DecompileWith::Nwnmdlcomp => {
                match mgv_mdl::tools::Nwnmdlcomp::find(self.lib.install().map(|i| i.root.clone())) {
                    Some(t) => t.decompile(&doc.opened.data, &name).map_err(|e| e.to_string()),
                    None => Err("nwnmdlcomp not found".into()),
                }
            }
        };
        match text {
            Ok(text) => {
                self.close_buffer();
                let mut b = Buffer::new(text, None, key, true);
                b.suggested_path = suggested;
                self.buffer = Some(b);
                self.apply_buffer();
                self.info(format!("Decompiled {name}: edit it here; Save writes it as ASCII"));
                self.focus(Tab::Editor);
            }
            Err(e) => self.error(format!("Decompile {name}: {e}")),
        }
    }

    /// Saves the editor's buffer.
    fn save(&mut self, ask: bool) {
        let Some(buffer) = &mut self.buffer else { return };
        let path = match (&buffer.path, ask) {
            (Some(p), false) => Some(p.clone()),
            _ => {
                let suggested = buffer
                    .suggested_path
                    .clone()
                    .or_else(|| buffer.path.clone())
                    .unwrap_or_else(|| PathBuf::from("model.mdl"));
                self.dialogs.save_file("Save the ASCII model", &suggested)
            }
        };
        let Some(path) = path else { return };
        let buffer = self.buffer.as_mut().expect("checked");
        match buffer.save(&path) {
            Ok(()) => {
                self.watcher.watch(&path);
                self.info(format!("Saved {}", path.display()));
            }
            Err(e) => self.error(format!("Saving {}: {e}", path.display())),
        }
    }

    /// Compiles the editor's model in the background.
    fn compile(&mut self, view_result: bool) {
        let Some(buffer) = &self.buffer else {
            self.warn(
                "Compile works on the ASCII in the editor: open an ASCII model or decompile one.",
            );
            return;
        };
        let name = buffer
            .key
            .map(|k| k.resref.to_lowercase().to_string())
            .or_else(|| {
                buffer
                    .path
                    .as_ref()
                    .and_then(|p| p.file_stem())
                    .map(|s| s.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| "model".into());
        let out_dir = buffer
            .path
            .as_ref()
            .and_then(|p| p.parent())
            .map(|d| d.join("compiled"))
            .or_else(|| self.settings.output_folder.clone());
        let Some(out_dir) =
            out_dir.or_else(|| self.dialogs.pick_folder("Where to put the compiled model"))
        else {
            return;
        };
        let request = jobs::CompileRequest {
            name,
            text: buffer.text(),
            search: buffer
                .path
                .as_ref()
                .and_then(|p| p.parent())
                .map(Path::to_path_buf)
                .into_iter()
                .collect(),
            out_dir,
            with: self.settings.compile_with,
            game_root: self.lib.install().map(|i| i.root.clone()),
            player_dir: self.lib.install().and_then(|i| i.user_dir.clone()),
            scratch: settings::scratch_dir(),
            view_result,
        };
        self.info(format!("Compiling {}…", request.name));
        self.jobs.compile(request);
    }

    fn jobs_done(&mut self) {
        for done in self.jobs.finished() {
            match done {
                jobs::Done::Compiled { name, path, with, view_result } => {
                    self.info(format!("Compiled {name} with {with}: {}", path.display()));
                    if view_result {
                        // Show the compiled model as the game will load it.
                        self.close_buffer();
                        self.open(&path);
                    }
                }
                jobs::Done::Failed { name, message } => {
                    self.error(format!("Compiling {name}: {message}"))
                }
            }
        }
    }

    fn shortcuts(&mut self, ui: &Ui) {
        use egui::{Key, KeyboardShortcut, Modifiers};
        let press = |m: Modifiers, k: Key| {
            ui.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(m, k)))
        };
        if press(Modifiers::COMMAND, Key::O) {
            self.actions.push(Action::OpenDialog);
        }
        if press(Modifiers::COMMAND | Modifiers::SHIFT, Key::S) {
            self.actions.push(Action::SaveAs);
        }
        if press(Modifiers::COMMAND, Key::S) {
            self.actions.push(Action::Save);
        }
        if press(Modifiers::COMMAND, Key::D) {
            self.actions.push(Action::Decompile);
        }
        if press(Modifiers::COMMAND | Modifiers::SHIFT, Key::B) {
            self.actions.push(Action::Compile { view_result: true });
        }
        if press(Modifiers::COMMAND, Key::B) {
            self.actions.push(Action::Compile { view_result: false });
        }
        if press(Modifiers::NONE, Key::F5) {
            self.actions.push(Action::Refresh);
        }
        if press(Modifiers::NONE, Key::F1) {
            self.manual.open = true;
        }
        if press(Modifiers::COMMAND, Key::Q) {
            self.actions.push(Action::Quit);
        }
    }

    /// The menus, in the Top Bar (the kit's menu bar and menu styles).
    /// egui's menu bar takes the width it is given: given the menus' own,
    /// as they took it in the last frame, the Top Bar's buttons follow
    /// them; when that changes, the frame is drawn again.
    pub(crate) fn menu_bar(&mut self, ui: &mut Ui) {
        let width_id = ui.id().with("menu-bar-width");
        let start = ui.cursor().min.x;
        let width = ui.data(|d| d.get_temp::<f32>(width_id)).unwrap_or(ui.available_width());
        let size = egui::vec2(width.min(ui.available_width()), ui.available_height());
        let layout = egui::Layout::left_to_right(egui::Align::Center);
        let mut end = start;
        ui.allocate_ui_with_layout(size, layout, |ui| self.menus(ui, &mut end));
        let measured = (end - start).ceil();
        if (measured - width).abs() > 0.5 {
            ui.data_mut(|d| d.insert_temp(width_id, measured));
            ui.ctx().request_discard("the menu bar's width changed");
        }
    }

    /// File, Model, View and Help; `end`: where the last menu's title ends.
    fn menus(&mut self, ui: &mut Ui, end: &mut f32) {
        let config = egui::containers::menu::MenuConfig::new().style(mg_uikit::menu_style);
        egui::MenuBar::new().style(mg_uikit::menu_bar_style).config(config).ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.add(egui::Button::new("Open…").shortcut_text("Ctrl+O")).clicked() {
                    self.actions.push(Action::OpenDialog);
                }
                ui.menu_button("Open Recent", |ui| {
                    if self.settings.recent.is_empty() {
                        ui.weak("Nothing yet");
                    }
                    for p in self.settings.recent.clone() {
                        if ui.button(p.display().to_string()).clicked() {
                            self.actions.push(Action::Open(p));
                        }
                    }
                });
                ui.separator();
                let has_buffer = self.can_save();
                if ui
                    .add_enabled(has_buffer, egui::Button::new("Save").shortcut_text("Ctrl+S"))
                    .clicked()
                {
                    self.actions.push(Action::Save);
                }
                if ui
                    .add_enabled(
                        has_buffer,
                        egui::Button::new("Save As…").shortcut_text("Ctrl+Shift+S"),
                    )
                    .clicked()
                {
                    self.actions.push(Action::SaveAs);
                }
                ui.separator();
                if ui
                    .button("Add Folder…")
                    .on_hover_text("Loose models and textures, above the game's")
                    .clicked()
                {
                    self.actions.push(Action::AddFolderDialog);
                }
                if ui.button("Add Hak, Module or ERF…").clicked() {
                    self.actions.push(Action::AddArchiveDialog);
                }
                if ui.button("Game Folder…").clicked() {
                    self.actions.push(Action::GameFolderDialog);
                }
                if ui.add(egui::Button::new("Refresh").shortcut_text("F5")).clicked() {
                    self.actions.push(Action::Refresh);
                }
                ui.separator();
                if ui.add(egui::Button::new("Quit").shortcut_text("Ctrl+Q")).clicked() {
                    self.actions.push(Action::Quit);
                }
            });
            ui.menu_button("Model", |ui| {
                if ui.add(egui::Button::new("Decompile").shortcut_text("Ctrl+D")).clicked() {
                    self.actions.push(Action::Decompile);
                }
                if ui.add(egui::Button::new("Compile").shortcut_text("Ctrl+B")).clicked() {
                    self.actions.push(Action::Compile { view_result: false });
                }
                if ui
                    .add(egui::Button::new("Compile and View").shortcut_text("Ctrl+Shift+B"))
                    .clicked()
                {
                    self.actions.push(Action::Compile { view_result: true });
                }
                if ui
                    .add_enabled(self.buffer.is_some(), egui::Button::new("Close Edits"))
                    .on_hover_text("Drop unsaved edits")
                    .clicked()
                {
                    self.actions.push(Action::CloseBuffer);
                }
                ui.separator();
                ui.label("Decompile with");
                ui.radio_value(
                    &mut self.settings.decompile_with,
                    settings::DecompileWith::Native,
                    "Moonglow (native)",
                );
                ui.radio_value(
                    &mut self.settings.decompile_with,
                    settings::DecompileWith::Nwnmdlcomp,
                    "nwnmdlcomp",
                );
                ui.label("Compile with");
                ui.radio_value(
                    &mut self.settings.compile_with,
                    settings::CompileWith::Auto,
                    "Moonglow",
                )
                .on_hover_text(
                    "The viewer's own compiler: no game, no window; every kind of model, \
                     keeping the part numbers of its compiled version",
                );
                ui.radio_value(
                    &mut self.settings.compile_with,
                    settings::CompileWith::Engine,
                    "The game's compiler",
                )
                .on_hover_text("Runs the game; it cannot compile skin meshes");
                ui.radio_value(
                    &mut self.settings.compile_with,
                    settings::CompileWith::Nwnmdlcomp,
                    "nwnmdlcomp",
                )
                .on_hover_text("Drops EE's materials and normals; 17 bones a skin");
            });
            ui.menu_button("View", |ui| {
                for tab in [
                    Tab::View,
                    Tab::Editor,
                    Tab::Texture,
                    Tab::Browser,
                    Tab::Outliner,
                    Tab::Inspector,
                    Tab::Timeline,
                    Tab::Effects,
                    Tab::Lighting,
                    Tab::Log,
                ] {
                    if ui.button(tab.title()).clicked() {
                        self.focus(tab);
                    }
                }
                ui.separator();
                ui.checkbox(&mut self.settings.show_grid, "Grid");
                ui.checkbox(&mut self.settings.show_axes, "Axes");
                if let Some(g) = &mut self.gfx {
                    ui.checkbox(&mut g.stage.model_lights, "Model lights");
                }
                let mut shaders = self.settings.game_shaders();
                if ui.checkbox(&mut shaders, "Game shaders").on_hover_text(SHADERS_HELP).changed() {
                    self.settings.game_shaders = Some(shaders);
                }
                let mut shadow = self.settings.model_shadows();
                if ui.checkbox(&mut shadow, "Shadow").on_hover_text(SHADOW_HELP).changed() {
                    self.settings.model_shadows = Some(shadow);
                }
                ui.separator();
                ui.label("Theme");
                for theme in mg_uikit::Theme::ALL {
                    ui.radio_value(&mut self.settings.theme, theme, theme.name());
                }
                ui.separator();
                if ui.button("Reset Layout").clicked() {
                    self.actions.push(Action::ResetLayout);
                }
            });
            let help = ui.menu_button("Help", |ui| {
                if ui.add(egui::Button::new("User Manual").shortcut_text("F1")).clicked() {
                    self.manual.open = true;
                }
                if ui.button("About Moonglow Viewer").clicked() {
                    self.manual.about = true;
                }
            });
            *end = help.response.rect.right();
        });
    }
}

/// The library for the settings' game folder (or the one detected).
fn open_library(settings: &Settings, log: &mut Vec<LogLine>) -> Library {
    let install = match &settings.game_root {
        Some(root) => Some(GameInstall::new(root, None, "en")),
        None => GameInstall::detect(),
    };
    let install = install.map(|mut i| {
        if !settings.use_user_dir {
            i.user_dir = None;
        }
        i
    });
    match Library::open(install) {
        Ok(l) => l,
        Err(e) => {
            log.push(LogLine { level: Level::Error, text: format!("The game's resources: {e}") });
            Library::open(None).expect("an empty library always opens")
        }
    }
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

/// A dock panel's or a document tab's own drawing.
pub(crate) fn draw_tab(app: &mut Viewer, ui: &mut Ui, tab: Tab) {
    match tab {
        Tab::View => view3d::ui(app, ui),
        Tab::Editor => editor::ui(app, ui),
        Tab::Texture => texture::ui(app, ui),
        Tab::Browser => panels::browser(app, ui),
        Tab::Outliner => panels::outliner(app, ui),
        Tab::Inspector => panels::inspector(app, ui),
        Tab::Timeline => panels::timeline(app, ui),
        Tab::Effects => effects::ui(app, ui),
        Tab::Lighting => lighting::ui(app, ui),
        Tab::Log => panels::log(app, ui),
    }
}

/// The editor area's tabs for egui_dock: their titles laid out with room
/// for their parts (`mg_uikit::dock::doc_tab_title`), their buttons kept
/// for the parts painted over them (`shell::editor_area`).
pub(crate) struct Tabs<'a> {
    pub(crate) app: &'a mut Viewer,
    pub(crate) ctx: egui::Context,
    pub(crate) buttons: Vec<shell::TabButton>,
    /// Each group's tab in front, as the frame began (the dock is out of
    /// `app` while it is drawn).
    pub(crate) fronts: Vec<Tab>,
}

impl Tabs<'_> {
    fn title_of(&self, tab: Tab) -> mg_uikit::dock::DocTabTitle {
        let font = egui::FontId::proportional(mg_uikit::metrics::BODY_SIZE);
        let modified = self.app.tab_modified(tab);
        mg_uikit::dock::doc_tab_title(&self.ctx, font, tab.title(), f32::INFINITY, modified)
    }
}

impl TabViewer for Tabs<'_> {
    type Tab = Tab;

    fn id(&mut self, tab: &mut Tab) -> egui::Id {
        egui::Id::new(("tab", *tab))
    }

    fn title(&mut self, tab: &mut Tab) -> WidgetText {
        WidgetText::Galley(self.title_of(*tab).galley)
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut Tab) {
        if ui.rect_contains_pointer(ui.max_rect()) {
            self.app.shell.hovering = Some(*tab);
        }
        draw_tab(self.app, ui, *tab);
    }

    /// The tab's button: kept for its parts, and named by its title for a
    /// screen reader (and tests), said selected where it is in front.
    fn on_tab_button(&mut self, tab: &mut Tab, response: &egui::Response) {
        let shown = response.rect.intersect(response.interact_rect);
        let title_width = self.title_of(*tab).width;
        self.buttons.push(shell::TabButton {
            tab: *tab,
            rect: response.rect,
            layer: response.layer_id,
            shown,
            title_width,
        });
        let front = self.fronts.contains(tab);
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::Button, true, front, tab.title())
        });
    }

    fn is_closeable(&self, tab: &Tab) -> bool {
        tab.closeable()
    }

    /// No floating windows: the viewer's editors stay in the window.
    fn allowed_in_windows(&self, _: &mut Tab) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mg_uikit::{Theme, Tokens};

    /// What the viewer applies each frame gives the context the kit's own
    /// tokens, for each theme the setting can hold.
    #[test]
    fn the_applied_theme_is_the_kits_tokens() {
        let ctx = egui::Context::default();
        for (theme, tokens) in [(Theme::Dark, Tokens::DARK), (Theme::Light, Tokens::LIGHT)] {
            mg_uikit::apply(&ctx, theme);
            assert_eq!(mg_uikit::tokens(&ctx), tokens);
            assert_eq!(mg_uikit::applied(&ctx), Some(theme));
        }
    }

    /// The ruled names (the proposal's section 9.17). The layout is not
    /// stored (only `Settings` is: eframe keeps nothing else of the
    /// window), so no layout from before the renames can name a tab; the
    /// titles are drawn each run. The Outliner stands in the Left Dock,
    /// the Timeline in the Bottom Dock.
    #[test]
    fn the_panels_carry_their_ruled_names() {
        assert_eq!(Tab::Outliner.title(), "Outliner");
        assert_eq!(Tab::Timeline.title(), "Timeline");
        let titles =
            |at| -> Vec<&str> { shell::panels_at(at, false).into_iter().map(Tab::title).collect() };
        assert_eq!(titles(shell::Place::Left), ["Resources", "Outliner"]);
        assert_eq!(titles(shell::Place::Bottom), ["Timeline", "Effects", "Log"]);
        let all = [titles(shell::Place::Left), titles(shell::Place::Bottom)].concat();
        assert!(!all.contains(&"Nodes") && !all.contains(&"Animation"));
    }
}
