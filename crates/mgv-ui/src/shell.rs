//! The window's shell, the Moonglow suite's (the UI redesign's M6, part B:
//! proposal sections 5 and 9.17), drawn with the kit's parts
//! (`mg_uikit::shell`, `mg_uikit::status`, `mg_uikit::dock`):
//!
//! - the **Top Bar**: the menus File, Model, View and Help, Open and Save
//!   as icon buttons after them, and Decompile, Compile and Compile and
//!   View at its right end (their words go under a window 1300 wide);
//! - the **Left Dock**: Resources over the Outliner (folded to its strip
//!   under a window 1500 wide);
//! - the **Right Dock**: Inspector over Lighting (280 wide and tabbed
//!   under 1400; in the wide layout, from 2560, two columns: Inspector
//!   over Log, Lighting over Effects);
//! - the **Bottom Dock**: Timeline, Effects and Log as tabs (the Timeline
//!   alone in the wide layout), under the editor area;
//! - the **Status Bar**: the Bottom Dock's switches and the Log's last
//!   line while the Log is out of sight, the hints of the panel under the
//!   pointer as key caps, and the document, its origin and the game folder;
//! - the **editor area**: egui_dock's `DockArea` in the kit's look, the 3D
//!   View in one group and Texture and ASCII in a second (three groups in
//!   the wide layout), each tab with its kind icon, its close button and
//!   the modified dot.
//!
//! The docks are the kit's, not egui_dock's: their panels stand where
//! [`place`] says at the window's width, unless the user dragged one to
//! another dock (its header, or its tab, dropped on the dock: the
//! toolset's way of dragging its Right Dock's panels, here between the
//! Left, Right and Bottom Docks). What the user chose is kept between
//! runs ([`Shell::layout`], `settings::Layout`): which dock a panel was
//! dragged to, each dock's size as its edge was dragged, the docks folded
//! or closed, the tabs in front, the editor area's groups; what the
//! window's width decides is not (a narrow window folds a side dock
//! without writing it, as Moonglow Toolset's does). Reset Layout makes it
//! all again.

use egui::{Rect, Ui};
use mg_uikit::shell::{self as kit, DockTab, DockTabs, FoldStrip, PanelHeader, Side, Yields};
use mg_uikit::status::{self, Hint, Hints, StatusItem, StatusJob, StatusLog, StatusSwitch};
use mg_uikit::{Icon, IconButton, metrics};

use crate::settings::{EditorGroup, Layout};
use crate::{Action, Buffer, Level, Tab, Viewer};

/// The Left Dock's name (its group, its strip, its fold button's).
pub const LEFT_DOCK: &str = "Left Dock";
/// The Right Dock's name.
pub const RIGHT_DOCK: &str = "Right Dock";
/// The Bottom Dock's name (its tab bar).
pub const BOTTOM_DOCK: &str = "Bottom Dock";
/// The Top Bar's Open button's name.
pub const OPEN: &str = "Open";

/// The least a side dock is dragged to.
const SIDE_LEAST: f32 = 200.0;

/// The Right Dock's width between the tabbed and the wide layouts: the
/// viewer boards' (`ViewerCreature.html`, `ViewerGallery.html`:
/// `--right-w: 352px`), wider than the kit's 320 so that the Inspector's
/// pair grid stands two pairs a row (`panels::pairs_per_row`).
pub(crate) const RIGHT_DOCK_WIDTH: f32 = 352.0;
/// The Left Dock's width at first in Gallery mode (the board "Resources
/// as a Gallery": three pictures of 128 a row).
const GALLERY_DOCK: f32 = 440.0;
/// The least a stacked panel keeps (its header and a few rows).
const PANEL_LEAST: f32 = 90.0;
/// The upper of two stacked panels' share of their column at first.
const UPPER_SHARE: f32 = 0.55;
/// The least the Bottom Dock is dragged to.
const BOTTOM_LEAST: f32 = 80.0;
/// The room round a dock panel's body.
const BODY_MARGIN: i8 = 6;
/// How far into the editor area a dock that is folded, closed or holds no
/// panel takes a dragged panel (and shows where it would land).
const DROP_BAND: f32 = 96.0;

/// Where a panel stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    /// A tab of the editor area (3D View, Texture, ASCII).
    Editor,
    /// The Left Dock.
    Left,
    /// The Right Dock, in its column (0, or 1 in the wide layout).
    Right(usize),
    /// A tab of the Bottom Dock.
    Bottom,
}

impl Place {
    /// Its name in the kept layout.
    fn key(self) -> &'static str {
        match self {
            Place::Editor => "Editor",
            Place::Left => "Left",
            Place::Right(0) => "Right",
            Place::Right(_) => "Right 2",
            Place::Bottom => "Bottom",
        }
    }

    /// The dock named `key` in a kept layout (not the editor area: a
    /// panel never stands there).
    fn from_key(key: &str) -> Option<Place> {
        match key {
            "Left" => Some(Place::Left),
            "Right" => Some(Place::Right(0)),
            "Right 2" => Some(Place::Right(1)),
            "Bottom" => Some(Place::Bottom),
            _ => None,
        }
    }
}

/// The docks' panels, in the order they stand in each.
pub const PANELS: [Tab; 7] = [
    Tab::Browser,
    Tab::Outliner,
    Tab::Inspector,
    Tab::Log,
    Tab::Lighting,
    Tab::Timeline,
    Tab::Effects,
];

/// The Bottom Dock's order: its own tabs first, then any dragged there.
const BOTTOM_ORDER: [Tab; 7] = [
    Tab::Timeline,
    Tab::Effects,
    Tab::Log,
    Tab::Browser,
    Tab::Outliner,
    Tab::Inspector,
    Tab::Lighting,
];

/// Where `tab` stands by default, in the wide layout or not (proposal
/// section 9.17, and the moves' "Viewer at 3440 by 1440": Inspector and
/// Log in the Right Dock's first column, Lighting and Effects in its
/// second, the Timeline alone in the Bottom Dock).
pub fn place(tab: Tab, wide: bool) -> Place {
    match tab {
        Tab::View | Tab::Editor | Tab::Texture | Tab::Gallery => Place::Editor,
        Tab::Browser | Tab::Outliner => Place::Left,
        Tab::Inspector => Place::Right(0),
        Tab::Lighting => Place::Right(usize::from(wide)),
        Tab::Log if wide => Place::Right(0),
        Tab::Effects if wide => Place::Right(1),
        Tab::Timeline | Tab::Effects | Tab::Log => Place::Bottom,
    }
}

/// The panels at `at` by default, in their order: Timeline, Effects, Log
/// in the Bottom Dock (not [`PANELS`]' order, which stacks the columns).
pub fn panels_at(at: Place, wide: bool) -> Vec<Tab> {
    order(at).into_iter().filter(|t| place(*t, wide) == at).collect()
}

/// The order the panels stand in at `at`.
fn order(at: Place) -> [Tab; 7] {
    if at == Place::Bottom { BOTTOM_ORDER } else { PANELS }
}

/// The docks' sizes as the user dragged their edges (and the seams
/// between stacked panels), in whole points; `None`: not chosen, the
/// size the window's width gives. Each is drawn held to what the window
/// allows and kept as chosen, so that a dock is as large again when the
/// window is (the toolset's `chosen_size`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sizes {
    pub left: Option<u32>,
    /// The Left Dock's in Gallery mode.
    pub left_gallery: Option<u32>,
    pub right: Option<u32>,
    /// The Right Dock's in the wide layout.
    pub right_wide: Option<u32>,
    pub bottom: Option<u32>,
    /// The height of a panel over another, by the upper panel.
    pub seams: Vec<(Tab, u32)>,
}

/// The shell's state: what the window's width yields, the docks folded
/// or open, the panels in front, where each panel was dragged, the
/// docks' sizes, and the panel under the pointer.
#[derive(Debug)]
pub struct Shell {
    /// What the shell yields at this frame's widths.
    pub yields: Yields,
    /// The Left Dock folded by the user, in a window wide enough not to
    /// fold it (kept between runs).
    left_hidden: bool,
    /// The Right Dock's, likewise.
    right_hidden: bool,
    /// The Left Dock unfolded by hand in a window that folds it: for as
    /// long as the window does (not kept).
    left_here: bool,
    /// The Right Dock's, likewise.
    right_here: bool,
    /// The Bottom Dock is open.
    pub bottom_open: bool,
    /// The Bottom Dock's tab in front.
    pub bottom_tab: Tab,
    /// The Right Dock's panel in front where its panels are tabs.
    pub right_tab: Tab,
    /// The panels the user dragged to another dock than their own.
    moved: Vec<(Tab, Place)>,
    /// The docks' sizes as chosen.
    pub sizes: Sizes,
    /// The panel being dragged by its header or tab.
    pub dragging: Option<Tab>,
    /// Where each dock was drawn at this frame (a column of the Right
    /// Dock each), for a dragged panel's drop.
    zones: Vec<(Place, Rect)>,
    /// The editor area's rectangle at this frame.
    editor_rect: Option<Rect>,
    /// The panel the pointer was over at the last frame (its hints).
    pub hovered: Option<Tab>,
    /// The panel the pointer is over at this frame.
    pub(crate) hovering: Option<Tab>,
    /// The editor area's width at the last frame.
    pub(crate) editor_width: Option<f32>,
    /// Whether the editor area was laid out for the wide layout (`None`
    /// before the first frame).
    pub(crate) laid_out: Option<bool>,
}

impl Default for Shell {
    fn default() -> Shell {
        Shell {
            yields: Yields::of(1600.0, 1600.0),
            left_hidden: false,
            right_hidden: false,
            left_here: false,
            right_here: false,
            bottom_open: true,
            bottom_tab: Tab::Timeline,
            right_tab: Tab::Inspector,
            moved: Vec::new(),
            sizes: Sizes::default(),
            dragging: None,
            zones: Vec::new(),
            editor_rect: None,
            hovered: None,
            hovering: None,
            editor_width: None,
            laid_out: None,
        }
    }
}

impl Shell {
    /// Whether the Left Dock shows as its strip: as the window's width
    /// says where it folds it (unless unfolded there by hand), else as the
    /// user left it.
    pub fn left_folded(&self) -> bool {
        match self.yields.left_dock_folded {
            true => !self.left_here,
            false => self.left_hidden,
        }
    }

    /// Whether the Right Dock shows as its strip, likewise.
    pub fn right_folded(&self) -> bool {
        match self.yields.right_dock_folded {
            true => !self.right_here,
            false => self.right_hidden,
        }
    }

    /// Folds a side dock or unfolds it, as the user asks: in a window
    /// whose width folds it, for as long as the window does; else the
    /// user's own fold (kept).
    pub fn set_folded(&mut self, side: Side, folded: bool) {
        match side {
            Side::Left if self.yields.left_dock_folded => self.left_here = !folded,
            Side::Left => self.left_hidden = folded,
            Side::Right if self.yields.right_dock_folded => self.right_here = !folded,
            Side::Right => self.right_hidden = folded,
        }
    }

    /// The yields of a window `window` wide whose editor area is `editor`
    /// wide. A dock unfolded by hand where the window folds it is folded
    /// again once the window no longer does, and has the user's own fold.
    pub(crate) fn frame_yields(&mut self, window: f32, editor: f32) {
        self.yields = Yields::of(window, editor);
        if !self.yields.left_dock_folded {
            self.left_here = false;
        }
        if !self.yields.right_dock_folded {
            self.right_here = false;
        }
        self.zones.clear();
        self.editor_rect = None;
    }

    /// The layout as a new window has it: every panel in its own dock,
    /// both side docks as the width says, the Bottom Dock open on the
    /// Timeline, the docks' sizes their defaults.
    pub(crate) fn reset(&mut self) {
        self.left_hidden = false;
        self.right_hidden = false;
        self.left_here = false;
        self.right_here = false;
        self.bottom_open = true;
        self.bottom_tab = Tab::Timeline;
        self.right_tab = Tab::Inspector;
        self.moved.clear();
        self.sizes = Sizes::default();
        self.dragging = None;
    }

    /// A dock part's id.
    fn id(&self, name: impl std::hash::Hash + std::fmt::Debug) -> egui::Id {
        egui::Id::new(("mgv-shell", name))
    }

    /// Where `tab` stands at this frame's width: where the user dragged
    /// it, else its own place (a Right Dock's second column is its first
    /// where the dock stands in one).
    pub fn place(&self, tab: Tab) -> Place {
        let wide = self.yields.wide;
        match self.moved.iter().find(|(t, _)| *t == tab) {
            Some((_, Place::Right(c))) => Place::Right(if wide { (*c).min(1) } else { 0 }),
            Some((_, at)) => *at,
            None => place(tab, wide),
        }
    }

    /// The panels at `at` at this frame's width, in their order.
    pub fn panels_at(&self, at: Place) -> Vec<Tab> {
        order(at).into_iter().filter(|t| self.place(*t) == at).collect()
    }

    /// Puts a dock's panel in the dock `to` (dragged there, or its menu's
    /// Move to): kept where that is not its own place at this width, and
    /// brought into sight there.
    pub fn move_panel(&mut self, tab: Tab, to: Place) {
        if to == Place::Editor || place(tab, false) == Place::Editor {
            return;
        }
        self.moved.retain(|(t, _)| *t != tab);
        if place(tab, self.yields.wide) != to {
            self.moved.push((tab, to));
        }
        self.show_panel(tab);
    }

    /// Brings a dock's panel into sight: its dock unfolded or opened, the
    /// panel in front where it is a tab.
    pub fn show_panel(&mut self, tab: Tab) {
        match self.place(tab) {
            Place::Left => self.set_folded(Side::Left, false),
            Place::Right(_) => {
                self.set_folded(Side::Right, false);
                self.right_tab = tab;
            }
            Place::Bottom => {
                self.bottom_open = true;
                self.bottom_tab = tab;
            }
            Place::Editor => {}
        }
    }

    /// Whether a dock's panel is in sight.
    pub fn panel_shown(&self, tab: Tab) -> bool {
        match self.place(tab) {
            Place::Left => !self.left_folded(),
            Place::Right(_) => {
                !self.right_folded()
                    && (!self.yields.right_dock_tabbed || self.right_front() == Some(tab))
            }
            Place::Bottom => self.bottom_open && self.bottom_front() == Some(tab),
            Place::Editor => true,
        }
    }

    /// The Bottom Dock's tab in front (its first where the one chosen
    /// stands elsewhere now); none where it holds no panel.
    pub fn bottom_front(&self) -> Option<Tab> {
        let tabs = self.panels_at(Place::Bottom);
        if tabs.contains(&self.bottom_tab) { Some(self.bottom_tab) } else { tabs.first().copied() }
    }

    /// The Right Dock's panel in front where its panels are tabs.
    fn right_front(&self) -> Option<Tab> {
        let tabs = self.panels_at(Place::Right(0));
        if tabs.contains(&self.right_tab) { Some(self.right_tab) } else { tabs.first().copied() }
    }

    /// At the frame's end: the panel the pointer was over.
    pub(crate) fn end_frame(&mut self) {
        self.hovered = self.hovering.take();
    }

    /// The layout to keep: what the user chose (`settings::Layout`), with
    /// the editor area's groups where they are not the default of this
    /// width.
    pub fn layout(&self, dock: &egui_dock::DockState<Tab>) -> Layout {
        let name = |t: Option<Tab>| t.map(|t| t.key().to_string());
        let editor = editor_groups(dock);
        let default = editor_groups(&crate::default_dock_for(self.yields.wide));
        let same = |a: &[EditorGroup], b: &[EditorGroup]| {
            a.len() == b.len()
                && a.iter().zip(b).all(|(x, y)| x.tabs == y.tabs && x.front == y.front)
        };
        Layout {
            docks: (self.moved.iter())
                .map(|(t, at)| (t.key().to_string(), at.key().to_string()))
                .collect(),
            left_folded: self.left_hidden,
            right_folded: self.right_hidden,
            bottom_closed: !self.bottom_open,
            bottom_front: name(Some(self.bottom_tab).filter(|t| *t != Tab::Timeline)),
            right_front: name(Some(self.right_tab).filter(|t| *t != Tab::Inspector)),
            left_width: self.sizes.left,
            left_gallery_width: self.sizes.left_gallery,
            right_width: self.sizes.right,
            right_wide_width: self.sizes.right_wide,
            bottom_height: self.sizes.bottom,
            seams: (self.sizes.seams.iter()).map(|(t, h)| (t.key().to_string(), *h)).collect(),
            editor: if same(&editor, &default) { Vec::new() } else { editor },
            ..Layout::default()
        }
    }

    /// Takes a kept layout (names this viewer does not know passed by, a
    /// panel kept in the editor area or a dock it cannot stand in left
    /// where it is). The window's width rules what it does not keep.
    pub fn restore(&mut self, layout: &Layout) {
        let dock_panel = |key: &str| Tab::from_key(key).filter(|t| PANELS.contains(t));
        self.moved = (layout.docks.iter())
            .filter_map(|(t, at)| Some((dock_panel(t)?, Place::from_key(at)?)))
            .collect();
        self.left_hidden = layout.left_folded;
        self.right_hidden = layout.right_folded;
        self.bottom_open = !layout.bottom_closed;
        if let Some(t) = layout.bottom_front.as_deref().and_then(dock_panel) {
            self.bottom_tab = t;
        }
        if let Some(t) = layout.right_front.as_deref().and_then(dock_panel) {
            self.right_tab = t;
        }
        // (A size of nothing, or larger than any screen, is not one.)
        let size = |s: Option<u32>| s.filter(|s| (16..=16384).contains(s));
        self.sizes = Sizes {
            left: size(layout.left_width),
            left_gallery: size(layout.left_gallery_width),
            right: size(layout.right_width),
            right_wide: size(layout.right_wide_width),
            bottom: size(layout.bottom_height),
            seams: (layout.seams.iter())
                .filter_map(|(t, h)| Some((dock_panel(t)?, size(Some(*h))?)))
                .collect(),
        };
    }

    /// Where the dock `at` was drawn at this frame (a column of the Right
    /// Dock's two), if it was.
    pub fn zone(&self, at: Place) -> Option<Rect> {
        self.zones.iter().find(|(z, _)| *z == at).map(|z| z.1)
    }

    /// A point on the header of the first panel in the dock `at` (its tab
    /// row in the Bottom Dock), where a drag or a right-click takes it.
    /// For the window tests: the docks' accesskit groups carry no bounds.
    pub fn first_header(&self, at: Place) -> Option<egui::Pos2> {
        let zone = self.zone(at)?;
        Some(zone.left_top() + egui::vec2(40.0, metrics::DOCK_TABS / 2.0))
    }

    /// The seam chosen under `tab` (its height over the next panel).
    fn seam(&self, tab: Tab) -> Option<u32> {
        self.sizes.seams.iter().find(|(t, _)| *t == tab).map(|s| s.1)
    }
}

/// The editor area's groups (each one's tabs), its tabs in front aside.
pub(crate) fn groups(dock: &egui_dock::DockState<Tab>) -> Vec<Vec<Tab>> {
    dock.iter_leaves().map(|(_, l)| l.tabs.clone()).collect()
}

/// The editor area's groups as kept: on its main surface, from the left,
/// each with its tab in front and its share of the area's width as last
/// drawn (equal before the first frame).
fn editor_groups(dock: &egui_dock::DockState<Tab>) -> Vec<EditorGroup> {
    let leaves: Vec<_> = dock.main_surface().iter().filter_map(|n| n.get_leaf()).collect();
    let whole: f32 = leaves.iter().map(|l| l.rect.width().max(0.0)).sum();
    let count = leaves.len().max(1) as f32;
    let mut groups: Vec<(f32, EditorGroup)> = (leaves.iter())
        .filter(|l| !l.tabs.is_empty())
        .map(|l| {
            let share = if whole > 1.0 { l.rect.width() / whole } else { 1.0 / count };
            let tabs = l.tabs.iter().map(|t| t.key().to_string()).collect();
            let share = (share * 1000.0).round() / 1000.0;
            (l.rect.left(), EditorGroup { tabs, front: l.active.0, share })
        })
        .collect();
    // (Left to right where they were drawn; the tree's order before.)
    if whole > 1.0 {
        groups.sort_by(|a, b| a.0.total_cmp(&b.0));
    }
    groups.into_iter().map(|g| g.1).collect()
}

/// The editor area made from kept groups, side by side at their shares:
/// none where they do not make one (no group, a tab twice, a dock panel
/// among them); the 3D View brought back in a group of its own where it
/// is missing.
pub fn editor_dock(groups: &[EditorGroup]) -> Option<egui_dock::DockState<Tab>> {
    let mut seen = Vec::new();
    let mut made: Vec<(Vec<Tab>, usize, f32)> = Vec::new();
    for g in groups {
        let mut tabs = Vec::new();
        for key in &g.tabs {
            let tab = Tab::from_key(key)?;
            if place(tab, false) != Place::Editor || seen.contains(&tab) {
                return None;
            }
            seen.push(tab);
            tabs.push(tab);
        }
        if !tabs.is_empty() {
            let front = g.front.min(tabs.len() - 1);
            made.push((tabs, front, g.share.clamp(0.05, 1.0)));
        }
    }
    if made.is_empty() {
        return None;
    }
    if !seen.contains(&Tab::View) {
        made.insert(0, (vec![Tab::View], 0, 1.0 / (made.len() + 1) as f32));
    }
    let (first, rest) = made.split_first()?;
    let mut dock = egui_dock::DockState::new(first.0.clone());
    let mut node = egui_dock::NodeIndex::root();
    // (Each split gives the group its share of what is left.)
    let mut left = made.iter().map(|g| g.2).sum::<f32>();
    let mut at_left = first.2;
    for (tabs, _, share) in rest {
        let fraction = (at_left / left).clamp(0.05, 0.95);
        let [_, new] = dock.main_surface_mut().split_right(node, fraction, tabs.clone());
        node = new;
        left -= at_left;
        at_left = *share;
    }
    for (tabs, front, _) in &made {
        if let Some(at) = dock.find_tab(&tabs[*front]) {
            let _ = dock.set_active_tab(at);
        }
    }
    Some(dock)
}

/// The 1 px line egui draws along a panel's edge (counted into a bar's
/// height, as the toolset counts it).
fn panel_line(ui: &Ui) -> f32 {
    ui.visuals().widgets.noninteractive.bg_stroke.width.round()
}

// ------------------------------------------------------------------ top bar

/// The Top Bar: the menus, Open and Save, and the build actions.
pub(crate) fn top_bar(app: &mut Viewer, ui: &mut Ui) {
    let t = mg_uikit::tokens(ui.ctx());
    let size = metrics::TOP_BAR + panel_line(ui);
    egui::Panel::top("menu")
        .exact_size(size)
        .resizable(false)
        .frame(kit::top_bar_frame(&t))
        .show(ui, |ui| top_bar_row(app, ui));
}

/// An icon button of the Top Bar, `enabled` or not, its tip also where
/// it is disabled.
fn bar_button(ui: &mut Ui, icon: Icon, name: &str, tip: &str, enabled: bool, alone: bool) -> bool {
    let button = IconButton::new(icon, name).icon_only(alone);
    ui.add_enabled(enabled, button).on_hover_text(tip).on_disabled_hover_text(tip).clicked()
}

fn top_bar_row(app: &mut Viewer, ui: &mut Ui) {
    let words = app.shell.yields.build_words;
    let can_save = app.can_save();
    let (quick, (), build) = kit::top_bar(
        ui,
        0.0,
        |ui| {
            app.menu_bar(ui);
            kit::vsep(ui);
            let mut asked = None;
            if bar_button(ui, Icon::Open, OPEN, "Open… (Ctrl+O)", true, true) {
                asked = Some(Action::OpenDialog);
            }
            if bar_button(ui, Icon::Save, "Save", "Save the ASCII model (Ctrl+S)", can_save, true) {
                asked = Some(Action::Save);
            }
            asked
        },
        |_, _| {},
        |ui| {
            let mut asked = None;
            let tip = "Decompile the compiled model into the ASCII editor (Ctrl+D)";
            if bar_button(ui, Icon::Decompile, "Decompile", tip, true, !words) {
                asked = Some(Action::Decompile);
            }
            let tip = "Compile the ASCII editor's model (Ctrl+B)";
            if bar_button(ui, Icon::Compile, "Compile", tip, true, !words) {
                asked = Some(Action::Compile { view_result: false });
            }
            let tip = "Compile, and show the compiled model as the game loads it (Ctrl+Shift+B)";
            if bar_button(ui, Icon::CompileView, "Compile and View", tip, true, !words) {
                asked = Some(Action::Compile { view_result: true });
            }
            asked
        },
    );
    app.actions.extend(quick);
    app.actions.extend(build);
}

// ------------------------------------------------------------------ status bar

/// The hints of the panel under the pointer (proposal section 8.2): the
/// 3D view's mouse and keys (`view3d`), the Effects' double-click, the
/// Texture's zoom, a gallery's click, double-click and keys (the Gallery
/// tab's, and the Resources panel's in Gallery mode: `gallery`).
pub fn hints(over: Option<Tab>, gallery: bool) -> Vec<Hint> {
    match over {
        Some(Tab::Gallery) => gallery_hints(),
        Some(Tab::Browser) if gallery => gallery_hints(),
        Some(Tab::Effects) => vec![Hint::keys(["Double-click"], "Apply effect")],
        Some(Tab::Texture) => vec![Hint::keys(["Ctrl", "Wheel"], "Zoom texture")],
        _ => vec![
            Hint::keys(["Drag"], "Orbit"),
            Hint::either(["Right Drag", "Middle Drag"], "Pan"),
            Hint::keys(["Wheel"], "Zoom"),
            Hint::keys(["Click"], "Select"),
            Hint::either(["Double-click", "F"], "Frame"),
            Hint::either(["W", "A", "S", "D"], "Walk"),
            Hint::either(["Q", "E"], "Down, Up"),
            Hint::keys(["Shift"], "Walk faster"),
        ],
    }
}

/// A gallery's hints (the board "Resources as a Gallery"'s arrows).
fn gallery_hints() -> Vec<Hint> {
    vec![
        Hint::keys(["Click"], "Show"),
        Hint::either(["Up", "Down"], "Row of pictures"),
        Hint::either(["Left", "Right"], "One picture"),
    ]
}

/// The Status Bar's right-hand parts.
enum Part {
    Job(StatusJob),
    Item(StatusItem),
    Separator,
}

impl Part {
    fn width(&self, ui: &Ui) -> f32 {
        match self {
            Part::Job(j) => j.width(ui),
            Part::Item(i) => i.width(ui),
            Part::Separator => status::SEPARATOR_WIDTH,
        }
    }
}

/// The Status Bar's right-hand group: a running job, the document (with
/// the modified dot while the ASCII has unsaved edits), its origin, the
/// game folder.
fn right_parts(app: &Viewer) -> Vec<Part> {
    // (Narrower in a narrow window, so that the hints keep room.)
    let narrow = app.shell.yields.right_dock_narrow;
    let (doc_most, origin_most) = if narrow { (160.0, 200.0) } else { (240.0, 320.0) };
    let mut parts = Vec::new();
    if app.jobs.busy() {
        parts.push(Part::Job(StatusJob::unknown("Compiling").max_width(160.0)));
        parts.push(Part::Separator);
    }
    match &app.doc {
        Some(d) => {
            let dirty = app.buffer.as_ref().is_some_and(Buffer::is_dirty);
            let item = StatusItem::new(d.name()).modified(dirty).max_width(doc_most);
            parts.push(Part::Item(item.tooltip("The document shown")));
            let origin = match (&d.opened.path, d.opened.key) {
                (Some(p), _) => Some(p.display().to_string()),
                (None, Some(k)) => app.lib.origin(&k).map(str::to_string),
                (None, None) => None,
            };
            if let Some(o) = origin {
                parts.push(Part::Separator);
                let item = StatusItem::new(o).weak().max_width(origin_most);
                parts.push(Part::Item(item.tooltip("Where it was read from")));
            }
        }
        None => {
            let item = StatusItem::new("Open a model (Ctrl+O) or pick one under Resources");
            parts.push(Part::Item(item.weak().max_width(origin_most)));
        }
    }
    if app.shell.yields.game_folder {
        parts.push(Part::Separator);
        let item = match app.lib.install() {
            Some(i) => {
                let folder = i.root.display().to_string();
                StatusItem::new(format!("Game: {folder}")).weak().tooltip("The game folder")
            }
            None => StatusItem::new("No game folder: File › Game Folder…").warn(),
        };
        parts.push(Part::Item(item.max_width(320.0)));
    }
    parts
}

/// The Log's last line's widest beside the 3D View's hints, which are
/// many (the toolset's `LAST_LINE_CAP_AREA`, beside an area's).
pub const LOG_LINE_CAP_VIEW: f32 = 96.0;
/// Its widest beside Texture and ASCII (the toolset's `LAST_LINE_CAP`).
pub const LOG_LINE_CAP: f32 = 160.0;
/// The least it is cut to before it goes (the toolset's
/// `LAST_LINE_LEAST`).
pub const LOG_LINE_LEAST: f32 = 48.0;

/// The room the Log's last line (`line` wide whole) is given in the
/// Status Bar's `middle` (the bar less its padding and gaps, the
/// switches and the right-hand group), `cap` at most. As Moonglow
/// Toolset's Status Bar yields (`mg-ui`'s `shell::status_bar`, parity
/// the rule): the line takes its room up to its cap before the hints get
/// any, so the hints drop whole from the end first (they take what is
/// left, `Hints::show`); then the line is cut down to [`LOG_LINE_LEAST`],
/// then it goes.
pub fn log_line_room(middle: f32, line: f32, cap: f32) -> Option<f32> {
    let want = line.min(cap);
    if middle >= want {
        Some(want.ceil())
    } else if middle >= LOG_LINE_LEAST {
        Some(middle.floor())
    } else {
        None
    }
}

/// The Status Bar.
pub(crate) fn status_bar(app: &mut Viewer, ui: &mut Ui) {
    let t = mg_uikit::tokens(ui.ctx());
    let size = metrics::STATUS_BAR + panel_line(ui);
    egui::Panel::bottom("status")
        .exact_size(size)
        .resizable(false)
        .frame(status::frame(&t))
        .show(ui, |ui| status_row(app, ui));
}

fn status_row(app: &mut Viewer, ui: &mut Ui) {
    let t = mg_uikit::tokens(ui.ctx());
    let shell = &app.shell;
    let switches = shell.panels_at(Place::Bottom);
    let front = shell.bottom_open.then(|| shell.bottom_front()).flatten();
    let icons = shell.yields.status_switch_icons;
    let last = (!shell.panel_shown(Tab::Log)).then(|| app.log.last()).flatten().map(|l| {
        let color = match l.level {
            Level::Info => t.text_2,
            Level::Warning => t.warn,
            Level::Error => t.error,
        };
        (l.text.clone(), color)
    });
    let gallery = app.settings.resources_view == crate::settings::ResourcesView::Gallery;
    let hints = hints(shell.hovered, gallery);
    let parts = right_parts(app);
    let right_width = parts.iter().map(|p| p.width(ui)).sum::<f32>()
        + status::GAP * parts.len().saturating_sub(1) as f32;
    let switch_width: f32 = (switches.iter())
        .map(|t| StatusSwitch::new(t.icon(), t.title(), false).icon_only(icons).width(ui))
        .map(|w| w + status::GAP)
        .sum();
    // (The bar's padding, the gaps round the middle, the separator before
    // the line.)
    let fixed = 2.0 * status::PADDING + 3.0 * status::GAP + status::SEPARATOR_WIDTH;
    let middle = ui.available_width() - fixed - switch_width - right_width;
    // (Its cap: the 3D View's, whose hints are many, where it is the
    // editor in front of the focused group.)
    let active = (app.dock.focused_leaf())
        .and_then(|p| app.dock.leaf(p).ok())
        .and_then(|l| l.tabs.get(l.active.0).copied())
        .unwrap_or(Tab::View);
    let cap = if active == Tab::View { LOG_LINE_CAP_VIEW } else { LOG_LINE_CAP };
    let last = last.and_then(|(text, color)| {
        let whole = StatusLog::new(text.clone()).width(ui);
        log_line_room(middle, whole, cap).map(|room| ((text, color), room))
    });
    let (clicked, _, ()) = status::bar_sized(
        ui,
        right_width,
        |ui| {
            let mut clicked = None;
            for tab in &switches {
                let on = front == Some(*tab);
                let tip = match on {
                    true => format!("Close the Bottom Dock ({})", tab.title()),
                    false => format!("Show the {} in the Bottom Dock", tab.title()),
                };
                let switch = StatusSwitch::new(tab.icon(), tab.title(), on).icon_only(icons);
                if ui.add(switch.tooltip(tip)).clicked() {
                    clicked = Some(*tab);
                }
            }
            if let Some(((text, color), room)) = last {
                status::separator(ui);
                let tip = "The Log's last line: click to show the Log";
                let line = StatusLog::new(text).color(color).max_width(room).tooltip(tip);
                if ui.add(line).clicked() {
                    clicked = Some(Tab::Log);
                }
            }
            clicked
        },
        |ui, room| {
            Hints::new(&hints).show(ui, room);
        },
        |ui| {
            for part in parts {
                match part {
                    Part::Job(j) => ui.add(j),
                    Part::Item(i) => ui.add(i),
                    Part::Separator => status::separator(ui),
                };
            }
        },
    );
    if let Some(tab) = clicked {
        let shell = &mut app.shell;
        if shell.bottom_open && shell.bottom_front() == Some(tab) {
            shell.bottom_open = false;
        } else {
            shell.show_panel(tab);
        }
    }
}

// ------------------------------------------------------------------ docks

/// Has egui draw its panel `id` `size` points along `axis` at this frame
/// (the size chosen, held to what the window allows): egui draws a panel
/// at the size it kept for it and keeps the size it drew, so a window
/// made smaller would otherwise leave the dock as small as it was drawn
/// there (the toolset's `size_panel`).
fn size_panel(ctx: &egui::Context, id: egui::Id, axis: usize, size: f32) {
    use egui::containers::panel::PanelState;
    let mut state = PanelState::load(ctx, id).unwrap_or(PanelState {
        outer_rect: Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::splat(size)),
    });
    if (state.size()[axis] - size).abs() > 0.01 {
        let r = &mut state.outer_rect;
        if axis == 0 {
            r.max.x = r.min.x + size;
        } else {
            r.max.y = r.min.y + size;
        }
        ctx.data_mut(|d| d.insert_persisted(id, state));
    }
}

/// Whether the edge of egui's panel `id` is being dragged or was let go
/// of at this frame (read before the panel is drawn, as egui reads it).
fn edge_dragged(ctx: &egui::Context, id: egui::Id) -> bool {
    let handle = id.with("__resize");
    ctx.read_response(handle).is_some_and(|r| r.dragged() || r.drag_stopped())
}

/// A dock's size this frame: `kept` (else `default`) held to `range`,
/// given to egui's panel `id`; whether its edge is dragged (the size it
/// is drawn at is then the one chosen).
fn sized(
    ctx: &egui::Context,
    id: egui::Id,
    axis: usize,
    kept: Option<u32>,
    default: f32,
    range: &std::ops::RangeInclusive<f32>,
) -> bool {
    let size = kept.map_or(default, |k| k as f32).clamp(*range.start(), *range.end());
    size_panel(ctx, id, axis, size);
    edge_dragged(ctx, id)
}

/// A size as kept: whole points.
fn kept(size: f32) -> Option<u32> {
    Some(size.round().max(0.0) as u32)
}

/// A dock panel's body: the panel's own drawing, with room round it;
/// notes the panel under the pointer.
fn body(app: &mut Viewer, ui: &mut Ui, tab: Tab) {
    if ui.rect_contains_pointer(ui.max_rect()) {
        app.shell.hovering = Some(tab);
    }
    egui::Frame::new().inner_margin(egui::Margin::same(BODY_MARGIN)).show(ui, |ui| {
        ui.set_min_size(ui.available_size());
        crate::draw_tab(app, ui, tab);
    });
}

/// The docks a panel can be moved to from where it stands (its menu's
/// Move to items): the Right Dock's two columns in the wide layout.
fn other_docks(shell: &Shell, tab: Tab) -> Vec<(Place, String)> {
    let here = shell.place(tab);
    let mut to = vec![(Place::Left, format!("Move to the {LEFT_DOCK}"))];
    match shell.yields.wide {
        true => {
            to.push((Place::Right(0), format!("Move to the {RIGHT_DOCK}, First Column")));
            to.push((Place::Right(1), format!("Move to the {RIGHT_DOCK}, Second Column")));
        }
        false => to.push((Place::Right(0), format!("Move to the {RIGHT_DOCK}"))),
    }
    to.push((Place::Bottom, format!("Move to the {BOTTOM_DOCK}")));
    to.into_iter().filter(|(at, _)| *at != here).collect()
}

/// A dock panel's header or tab: dragged, it is the panel being dragged
/// (dropped on a dock, it moves there); right-clicked, its menu moves it
/// to another dock or puts the layout back.
fn drag_source(app: &mut Viewer, response: &egui::Response, tab: Tab) {
    if response.drag_started() {
        app.shell.dragging = Some(tab);
    }
    let docks = other_docks(&app.shell, tab);
    let chosen = mg_uikit::context_menu(response, |ui| {
        let mut chosen = None;
        for (at, words) in &docks {
            if ui.button(words).clicked() {
                chosen = Some(Some(*at));
                ui.close();
            }
        }
        mg_uikit::menu_separator(ui);
        if ui.button("Reset Layout").clicked() {
            chosen = Some(None);
            ui.close();
        }
        chosen
    });
    match chosen.and_then(|c| c.inner) {
        Some(Some(at)) => app.shell.move_panel(tab, at),
        Some(None) => app.actions.push(Action::ResetLayout),
        None => {}
    }
}

/// A panel under its header (named "Resources panel"), the dock's fold
/// button among its tools where `fold` is given; whether it was pressed.
/// The header drags the panel to another dock.
fn headed(app: &mut Viewer, ui: &mut Ui, tab: Tab, fold: Option<(Side, &str)>) -> bool {
    let name = format!("{} panel", tab.title());
    let mut header = PanelHeader::new(tab.title()).name(name).draggable(true);
    if let Some(detail) = detail(app, tab) {
        header = header.detail(detail);
    }
    let shown = header.show(ui, |ui| {
        let folded = fold.is_some_and(|(side, dock)| kit::fold_button(ui, side, dock).clicked());
        // (Resources: its gallery as a tab, the board's icon.)
        let tabbed = tab == Tab::Browser && {
            let button = IconButton::new(Icon::OpenInTab, crate::gallery::OPEN_AS_TAB);
            let tip = "Open the gallery as a tab in the editor area";
            ui.add(button.icon_only(true)).on_hover_text(tip).clicked()
        };
        (folded, tabbed)
    });
    let (folded, tabbed) = shown.inner;
    let tip = "Drag to another dock to move it there; right-click for more";
    drag_source(app, &shown.response.on_hover_text(tip), tab);
    body(app, ui, tab);
    if tabbed {
        app.open_gallery_tab();
    }
    folded
}

/// What a panel's header says after its title: the Inspector's what is
/// open (the board's "Creature", "Model").
fn detail(app: &Viewer, tab: Tab) -> Option<String> {
    let doc = app.doc.as_ref().filter(|_| tab == Tab::Inspector)?;
    Some(match doc.creature {
        Some(_) => "Creature".into(),
        None => format!("{:?}", doc.opened.kind),
    })
}

/// The panels of a dock's column, each under its header, each over the
/// next at a seam the user drags (kept as the upper one's height); whether
/// the fold button (in the first one's header, where `fold` is given) was
/// pressed.
fn stack(app: &mut Viewer, ui: &mut Ui, tabs: &[Tab], fold: Option<(Side, &str)>) -> bool {
    match tabs {
        [] => false,
        [one] => headed(app, ui, *one, fold),
        [first, rest @ ..] => {
            let whole = ui.max_rect().height();
            // (Room for the panels under it at their least.)
            let under = PANEL_LEAST * rest.len() as f32;
            let most = (whole - under).max(PANEL_LEAST);
            let at_first = (whole / (rest.len() + 1) as f32 * 2.0 * UPPER_SHARE).round();
            let range = PANEL_LEAST..=most;
            let id = app.shell.id(("seam", *first));
            let ctx = ui.ctx().clone();
            let dragged = sized(&ctx, id, 1, app.shell.seam(*first), at_first, &range);
            let mut folded = false;
            let shown = egui::Panel::top(id)
                .resizable(true)
                .default_size(at_first.clamp(PANEL_LEAST, most))
                .size_range(range)
                .frame(egui::Frame::new())
                .show(ui, |ui| folded = headed(app, ui, *first, fold));
            if dragged {
                let seams = &mut app.shell.sizes.seams;
                seams.retain(|(t, _)| t != first);
                seams.extend(kept(shown.response.rect.height()).map(|h| (*first, h)));
            }
            folded | stack(app, ui, rest, None)
        }
    }
}

/// A folded dock: its strip of its panels' icons under the chevron; the
/// panel clicked (`Some(None)`: the chevron).
fn strip(
    app: &mut Viewer,
    ui: &mut Ui,
    dock: &str,
    side: Side,
    tabs: &[Tab],
) -> Option<Option<Tab>> {
    let t = mg_uikit::tokens(ui.ctx());
    let id = app.shell.id((dock, "strip"));
    let panel = match side {
        Side::Left => egui::Panel::left(id),
        Side::Right => egui::Panel::right(id),
    };
    let shown = panel
        .exact_size(kit::STRIP + panel_line(ui))
        .resizable(false)
        .frame(egui::Frame::new().fill(t.bar))
        .show(ui, |ui| {
            let mut strip = FoldStrip::new(dock, side);
            for tab in tabs {
                strip = strip.panel(tab.icon(), tab.title(), format!("Show the {}", tab.title()));
            }
            strip.show(ui)
        });
    let at = if side == Side::Left { Place::Left } else { Place::Right(0) };
    app.shell.zones.push((at, shown.response.rect));
    shown.inner.map(|click| match click {
        kit::StripClick::Unfold => None,
        kit::StripClick::Panel(i) => tabs.get(i).copied(),
    })
}

/// The widest a side dock is dragged to: two fifths of the window.
fn side_most(ui: &Ui, least: f32) -> f32 {
    (ui.ctx().content_rect().width() * 0.4).max(least)
}

/// The Left Dock: Resources over the Outliner (and any panel dragged
/// there), or its strip; nothing where it holds no panel.
pub(crate) fn left_dock(app: &mut Viewer, ui: &mut Ui) {
    let tabs = app.shell.panels_at(Place::Left);
    if tabs.is_empty() {
        return;
    }
    if app.shell.left_folded() {
        if let Some(chosen) = strip(app, ui, LEFT_DOCK, Side::Left, &tabs) {
            app.shell.set_folded(Side::Left, false);
            if let Some(tab) = chosen {
                app.shell.show_panel(tab);
            }
        }
        return;
    }
    let t = mg_uikit::tokens(ui.ctx());
    let ctx = ui.ctx().clone();
    // (Wider in Gallery mode, three pictures a row: the board's 440; each
    // mode keeps its own width.)
    let gallery = app.settings.resources_view == crate::settings::ResourcesView::Gallery
        && tabs.contains(&Tab::Browser);
    let (chosen, default) = match gallery {
        true => (app.shell.sizes.left_gallery, app.shell.yields.left_dock_width.max(GALLERY_DOCK)),
        false => (app.shell.sizes.left, app.shell.yields.left_dock_width),
    };
    let range = SIDE_LEAST..=side_most(ui, SIDE_LEAST);
    let id = app.shell.id("left-dock");
    let dragged = sized(&ctx, id, 0, chosen, default, &range);
    let mut folded = false;
    let shown = egui::Panel::left(id)
        .resizable(true)
        .default_size(default.min(*range.end()))
        .size_range(range)
        .frame(egui::Frame::new().fill(t.panel))
        .show(ui, |ui| {
            mg_uikit::name_group(ui, LEFT_DOCK);
            folded = stack(app, ui, &tabs, Some((Side::Left, LEFT_DOCK)));
        });
    let rect = shown.response.rect;
    if dragged {
        match gallery {
            true => app.shell.sizes.left_gallery = kept(rect.width()),
            false => app.shell.sizes.left = kept(rect.width()),
        }
    }
    app.shell.zones.push((Place::Left, rect));
    if folded {
        app.shell.set_folded(Side::Left, true);
    }
}

/// The Right Dock: Inspector over Lighting (and any panel dragged there);
/// their tabs in a narrow window; two columns in the wide layout; or its
/// strip; nothing where it holds no panel.
pub(crate) fn right_dock(app: &mut Viewer, ui: &mut Ui) {
    let y = app.shell.yields;
    let columns: Vec<Vec<Tab>> = match y.wide {
        true => [0, 1].map(|c| app.shell.panels_at(Place::Right(c))).into(),
        false => vec![app.shell.panels_at(Place::Right(0))],
    };
    let columns: Vec<Vec<Tab>> = columns.into_iter().filter(|c| !c.is_empty()).collect();
    if columns.is_empty() {
        return;
    }
    if app.shell.right_folded() {
        let all: Vec<Tab> = columns.concat();
        if let Some(chosen) = strip(app, ui, RIGHT_DOCK, Side::Right, &all) {
            app.shell.set_folded(Side::Right, false);
            if let Some(tab) = chosen {
                app.shell.right_tab = tab;
            }
        }
        return;
    }
    let t = mg_uikit::tokens(ui.ctx());
    let ctx = ui.ctx().clone();
    let narrow = y.right_dock_narrow;
    let two = columns.len() == 2;
    // (Each form of the dock keeps its own width: a window narrowed to
    // the tabbed dock and widened again finds it as it was.)
    let id = app.shell.id(("right-dock", narrow, y.wide));
    let least = if two { 2.0 * kit::RIGHT_DOCK_NARROW + 1.0 } else { SIDE_LEAST };
    let range = least..=side_most(ui, least);
    let (chosen, default) = match y.wide {
        true if two => (app.shell.sizes.right_wide, y.right_dock_width),
        true => (app.shell.sizes.right_wide, kit::RIGHT_DOCK_WIDE_SINGLE),
        false => (app.shell.sizes.right, RIGHT_DOCK_WIDTH),
    };
    let mut panel = egui::Panel::right(id).frame(egui::Frame::new().fill(t.panel));
    let mut dragged = false;
    panel = match narrow {
        true => panel.exact_size(kit::RIGHT_DOCK_NARROW).resizable(false),
        false => {
            dragged = sized(&ctx, id, 0, chosen, default, &range);
            panel.resizable(true).default_size(default.min(*range.end())).size_range(range)
        }
    };
    let mut folded = false;
    let mut first_column = None;
    let shown = panel.show(ui, |ui| {
        let fold = Some((Side::Right, RIGHT_DOCK));
        if y.right_dock_tabbed {
            let tabs = &columns[0];
            let front = app.shell.right_front().unwrap_or(tabs[0]);
            let mut i = tabs.iter().position(|t| *t == front).unwrap_or(0);
            let button = IconButton::new(Icon::ChevronRight, "").icon_only(true).width(ui);
            let mut bar = DockTabs::new(RIGHT_DOCK).tools_width(button).draggable(true);
            for tab in tabs {
                bar = bar.with(DockTab::new(tab.title()).icon(tab.icon()));
            }
            let out =
                bar.show(ui, &mut i, |ui| kit::fold_button(ui, Side::Right, RIGHT_DOCK).clicked());
            folded = out.inner;
            for (tab, r) in tabs.iter().zip(&out.tabs) {
                drag_source(app, r, *tab);
            }
            app.shell.right_tab = tabs[i];
            body(app, ui, tabs[i]);
            return;
        }
        mg_uikit::name_group(ui, RIGHT_DOCK);
        if let [first, second] = columns.as_slice() {
            // Two columns, a line between them (`Yields::right_first_column`).
            let width = Yields::right_first_column(ui.max_rect().width());
            let column = egui::Panel::left(app.shell.id("right-column"))
                .exact_size(width)
                .resizable(false)
                .frame(egui::Frame::new())
                .show(ui, |ui| folded = stack(app, ui, first, fold));
            first_column = Some(column.response.rect);
            stack(app, ui, second, None);
        } else {
            folded = stack(app, ui, &columns[0], fold);
        }
    });
    let rect = shown.response.rect;
    if dragged {
        match y.wide {
            true => app.shell.sizes.right_wide = kept(rect.width()),
            false => app.shell.sizes.right = kept(rect.width()),
        }
    }
    match first_column {
        Some(c) => {
            app.shell.zones.push((Place::Right(0), c));
            app.shell.zones.push((Place::Right(1), rect.with_min_x(c.right())));
        }
        // (In the wide layout a dock standing in one column takes a panel
        // into the other at that column's side: its right half for the
        // second, its left half for the first.)
        None if y.wide => {
            let (col, other) = match app.shell.place(columns[0][0]) {
                Place::Right(1) => (1, 0),
                _ => (0, 1),
            };
            let half = rect.center().x;
            let band = match other {
                0 => rect.with_max_x(half),
                _ => rect.with_min_x(half),
            };
            app.shell.zones.push((Place::Right(other), band));
            app.shell.zones.push((Place::Right(col), rect));
        }
        None => app.shell.zones.push((Place::Right(0), rect)),
    }
    if folded {
        app.shell.set_folded(Side::Right, true);
    }
}

/// The Bottom Dock, under the editor area, unless closed: its panels as
/// tabs, which drag to another dock.
pub(crate) fn bottom_dock(app: &mut Viewer, ui: &mut Ui) {
    let tabs = app.shell.panels_at(Place::Bottom);
    if !app.shell.bottom_open || tabs.is_empty() {
        return;
    }
    let t = mg_uikit::tokens(ui.ctx());
    let ctx = ui.ctx().clone();
    let range = BOTTOM_LEAST..=(ctx.content_rect().height() * 0.5).max(BOTTOM_LEAST);
    let id = app.shell.id("bottom-dock");
    let dragged = sized(&ctx, id, 1, app.shell.sizes.bottom, metrics::BOTTOM_DOCK, &range);
    let shown = egui::Panel::bottom(id)
        .resizable(true)
        .default_size(metrics::BOTTOM_DOCK.min(*range.end()))
        .size_range(range)
        .frame(egui::Frame::new().fill(t.panel))
        .show(ui, |ui| {
            let front = app.shell.bottom_front().unwrap_or(tabs[0]);
            let mut i = tabs.iter().position(|t| *t == front).unwrap_or(0);
            let mut bar = DockTabs::new(BOTTOM_DOCK).draggable(true);
            for tab in &tabs {
                bar = bar.with(DockTab::new(tab.title()).icon(tab.icon()));
            }
            let out = bar.show(ui, &mut i, |_| {});
            for (tab, r) in tabs.iter().zip(&out.tabs) {
                drag_source(app, r, *tab);
            }
            app.shell.bottom_tab = tabs[i];
            body(app, ui, tabs[i]);
        });
    if dragged {
        app.shell.sizes.bottom = kept(shown.response.rect.height());
    }
    app.shell.zones.push((Place::Bottom, shown.response.rect));
}

/// Where a panel dragged to `pos` would land, and the rectangle that
/// shows it: a dock drawn under the pointer (a column of the Right Dock's
/// two), else the band of the editor area along a dock that is folded,
/// closed or holds no panel.
pub(crate) fn drop_target(shell: &Shell, pos: egui::Pos2) -> Option<(Place, Rect)> {
    if let Some((at, r)) = shell.zones.iter().find(|(_, r)| r.contains(pos)) {
        // (A folded dock's strip shows its band too.)
        let band = band(shell, *at).filter(|b| r.width() < DROP_BAND && b.contains(pos));
        return Some((*at, band.unwrap_or(*r)));
    }
    [Place::Left, Place::Right(0), Place::Bottom]
        .into_iter()
        .filter_map(|at| Some((at, band(shell, at)?)))
        .find(|(_, r)| r.contains(pos))
}

/// The band of the editor area along the dock `at` (the Right Dock's
/// first column standing for the dock), where that dock is not drawn
/// wider than the band.
fn band(shell: &Shell, at: Place) -> Option<Rect> {
    let e = shell.editor_rect?;
    let drawn = shell.zones.iter().filter(|(z, _)| match at {
        Place::Right(_) => matches!(z, Place::Right(_)),
        _ => *z == at,
    });
    if drawn.clone().any(|(_, r)| r.width() >= DROP_BAND && r.height() >= DROP_BAND) {
        return None;
    }
    let w = DROP_BAND.min(e.width() / 3.0);
    let h = DROP_BAND.min(e.height() / 3.0);
    Some(match at {
        Place::Left => e.with_max_x(e.left() + w),
        Place::Right(_) => e.with_min_x(e.right() - w),
        Place::Bottom => e.with_min_y(e.bottom() - h),
        Place::Editor => return None,
    })
}

/// A panel dragged by its header or tab: where it would land outlined in
/// the accent (the kit's `stack::paint_drop_preview`, the toolset's
/// preview), moved there once the button is let go; Escape gives the drag
/// up. Where it lands is the viewer's own (a whole dock, or its band):
/// the kit's `StackDrop` finds a zone within a section, which the
/// viewer's docks, panels in a fixed order, do not have.
pub(crate) fn drop_panel(app: &mut Viewer, ui: &Ui) {
    let Some(tab) = app.shell.dragging else { return };
    let ctx = ui.ctx();
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.shell.dragging = None;
        return;
    }
    ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
    let pos = ctx.pointer_interact_pos().or_else(|| ctx.pointer_latest_pos());
    let target = pos.and_then(|p| drop_target(&app.shell, p));
    if let Some((_, rect)) = target {
        mg_uikit::stack::paint_drop_preview(ctx, rect);
    }
    if !ctx.input(|i| i.pointer.any_down()) {
        app.shell.dragging = None;
        if let Some((at, _)) = target
            && at != app.shell.place(tab)
        {
            app.shell.move_panel(tab, at);
        }
    }
}

// ------------------------------------------------------------------ editor area

/// A document tab egui_dock drew this frame: where its button is (on its
/// layer, the part of it in sight) and its title's width.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TabButton {
    pub tab: Tab,
    pub rect: Rect,
    pub layer: egui::LayerId,
    pub shown: Rect,
    pub title_width: f32,
}

/// The editor area: the document tabs of egui_dock's `DockArea` in the
/// kit's look, flush with the docks.
pub(crate) fn editor_area(app: &mut Viewer, ui: &mut Ui) {
    let flush = egui::Frame::central_panel(ui.style()).inner_margin(0);
    egui::CentralPanel::default().frame(flush).show(ui, |ui| {
        app.shell.editor_width = Some(ui.available_width());
        app.shell.editor_rect = Some(ui.max_rect());
        let mut dock = std::mem::replace(&mut app.dock, egui_dock::DockState::new(Vec::new()));
        let fronts =
            (dock.iter_leaves()).filter_map(|(_, l)| l.tabs.get(l.active.0).copied()).collect();
        let mut tabs = crate::Tabs { app, ctx: ui.ctx().clone(), buttons: Vec::new(), fronts };
        // (The headers flush with the tab bar, as the boards draw them.)
        let mut style = mg_uikit::dock::doc_tabs_style(ui);
        style.tab.tab_body.inner_margin = egui::Margin::ZERO;
        egui_dock::DockArea::new(&mut dock)
            .style(style)
            .show_close_buttons(false)
            .show_leaf_collapse_buttons(false)
            .show_leaf_close_all_buttons(false)
            .show_inside(ui, &mut tabs);
        let buttons = std::mem::take(&mut tabs.buttons);
        app.dock = dock;
        doc_tab_parts(app, ui, &buttons);
    });
}

/// Each document tab's kind icon, modified dot (said "Modified") and
/// close button (the kit's `doc_tab_close`, on the tab in front of its
/// group and the one under the pointer, for Texture and ASCII), the lines
/// between the tabs and the accent line along the top of the focused
/// group's tab in front.
fn doc_tab_parts(app: &mut Viewer, ui: &mut Ui, buttons: &[TabButton]) {
    let ctx = ui.ctx().clone();
    let pointer = ctx.pointer_hover_pos();
    let mut marks = Vec::with_capacity(buttons.len());
    let mut close = None;
    for b in buttons {
        let modified = app.tab_modified(b.tab);
        let active = app.dock.iter_leaves().any(|(_, l)| l.tabs.get(l.active.0) == Some(&b.tab));
        let hovered = pointer.is_some_and(|p| b.shown.contains(p));
        let parts = kit::doc_tab_parts(b.rect, b.title_width, modified);
        if let Some(dot) = parts.dot.filter(|d| b.shown.contains_rect(*d)) {
            let r = ui.interact(dot, egui::Id::new(("doc-tab-dot", b.tab)), egui::Sense::hover());
            r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, "Modified"));
        }
        let mut state = None;
        if b.tab.closeable() && (active || hovered) && b.shown.contains_rect(parts.close) {
            let id = egui::Id::new(("doc-tab-close", b.tab));
            let name = format!("Close {}", b.tab.title());
            let (r, drawn) = kit::doc_tab_close(ui, id, (b.layer, b.shown), parts.close, &name);
            state = Some(drawn);
            if r.clicked() || r.middle_clicked() {
                close = Some(b.tab);
            }
        }
        marks.push(mg_uikit::dock::DocTabMark {
            tab: b.tab,
            rect: b.rect,
            layer: b.layer,
            shown: b.shown,
            icon: b.tab.icon(),
            title_width: b.title_width,
            modified,
            active,
            close: state,
        });
    }
    // (The focused group's tab in front has the accent line; before any
    // click, the 3D View's group.)
    let focused = app.dock.focused_leaf().or_else(|| {
        app.dock
            .find_tab(&Tab::View)
            .map(|at| egui_dock::NodePath { surface: at.surface, node: at.node })
    });
    mg_uikit::dock::doc_tab_marks(&ctx, &app.dock, focused, &marks);
    if let Some(tab) = close {
        app.close_tab(tab);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NoDialogs, Settings};

    /// A viewer with no game and no GPU (the shell, the headers and the
    /// bars draw all the same).
    fn viewer() -> Viewer {
        let settings = Settings {
            game_root: Some(std::path::PathBuf::from("/nonexistent")),
            use_user_dir: false,
            ..Settings::default()
        };
        Viewer::new(settings, Box::new(NoDialogs))
    }

    /// Runs `frames` frames of a window `size` big.
    fn run(ctx: &egui::Context, v: &mut Viewer, size: egui::Vec2, frames: usize) {
        for _ in 0..frames {
            frame(ctx, v, size, Vec::new());
        }
    }

    /// One frame of a window `size` big, given `events`.
    fn frame(ctx: &egui::Context, v: &mut Viewer, size: egui::Vec2, events: Vec<egui::Event>) {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, size)),
            events,
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| v.ui(ui));
        out.textures_delta.clear();
        v.run_actions();
    }

    /// The settings as eframe keeps them between runs (RON) and reads them
    /// back: a new run's viewer.
    fn next_run(v: &Viewer) -> Viewer {
        let text = ron::to_string(&v.settings).unwrap();
        Viewer::new(ron::from_str(&text).unwrap(), Box::new(NoDialogs))
    }

    /// The left button pressed at `from`, the pointer moved to `to` in
    /// steps, the button let go there.
    fn drag(
        ctx: &egui::Context,
        v: &mut Viewer,
        size: egui::Vec2,
        from: egui::Pos2,
        to: egui::Pos2,
    ) {
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(ctx, v, size, vec![egui::Event::PointerMoved(from)]);
        frame(ctx, v, size, vec![button(from, true)]);
        for i in 1..=8 {
            let p = from + (to - from) * (i as f32 / 8.0);
            frame(ctx, v, size, vec![egui::Event::PointerMoved(p)]);
        }
        frame(ctx, v, size, vec![button(to, false)]);
        run(ctx, v, size, 2);
    }

    const MODEL: &str = "newmodel tri\nsetsupermodel tri NULL\nclassification character\n\
        beginmodelgeom tri\nnode dummy tri\n  parent NULL\nendnode\n\
        node trimesh plane\n  parent tri\n  verts 3\n    0 0 0\n    1 0 0\n    0 1 0\n\
        faces 1\n    0 1 2 1 0 0 0 0\nendnode\nendmodelgeom tri\ndonemodel tri\n";

    /// Every panel stands somewhere, once, in both layouts: the board's
    /// docks (section 9.17) and the wide layout's columns.
    #[test]
    fn every_panel_has_one_place() {
        for wide in [false, true] {
            for tab in PANELS {
                let at = place(tab, wide);
                assert_ne!(at, Place::Editor);
                assert_eq!(panels_at(at, wide).iter().filter(|t| **t == tab).count(), 1);
            }
        }
        assert_eq!(panels_at(Place::Left, false), [Tab::Browser, Tab::Outliner]);
        assert_eq!(panels_at(Place::Right(0), false), [Tab::Inspector, Tab::Lighting]);
        assert_eq!(panels_at(Place::Bottom, false), [Tab::Timeline, Tab::Effects, Tab::Log]);
        assert_eq!(panels_at(Place::Right(0), true), [Tab::Inspector, Tab::Log]);
        assert_eq!(panels_at(Place::Right(1), true), [Tab::Lighting, Tab::Effects]);
        assert_eq!(panels_at(Place::Bottom, true), [Tab::Timeline]);
    }

    /// The hints: the 3D view's camera, and the moves' "New" ones over
    /// Effects and Texture.
    #[test]
    fn the_hints_follow_the_panel_under_the_pointer() {
        let words =
            |over| hints(over, false).iter().map(|h| h.word().to_string()).collect::<Vec<_>>();
        assert_eq!(words(Some(Tab::Effects)), ["Apply effect"]);
        assert_eq!(words(Some(Tab::Texture)), ["Zoom texture"]);
        let view = words(Some(Tab::View));
        assert_eq!(view[..3], ["Orbit", "Pan", "Zoom"]);
        assert_eq!(words(None), view);
        // Over a gallery: its click, double-click and arrows; the Resources
        // panel's only in Gallery mode.
        let gallery = ["Show", "Row of pictures", "One picture"];
        let over = |tab, mode| -> Vec<String> {
            hints(Some(tab), mode).iter().map(|h| h.word().to_string()).collect()
        };
        assert_eq!(over(Tab::Gallery, false), gallery);
        assert_eq!(over(Tab::Browser, true), gallery);
        assert_eq!(over(Tab::Browser, false), view);
    }

    /// The toolset's order: the Log's last line takes its room up to its
    /// cap before the hints get any (they drop whole from the end first),
    /// then it is cut down to 48, then it goes.
    #[test]
    fn the_hints_drop_before_the_log_line_is_cut() {
        // A short line whole; a long one at its cap.
        assert_eq!(log_line_room(900.0, 70.0, LOG_LINE_CAP_VIEW), Some(70.0));
        assert_eq!(log_line_room(900.0, 400.0, LOG_LINE_CAP_VIEW), Some(96.0));
        assert_eq!(log_line_room(900.0, 400.0, LOG_LINE_CAP), Some(160.0));
        // The 3D View's hints (some 700 wide) beside it in a middle of 400:
        // the line keeps its cap, the hints drop to what the rest holds.
        let hints = hints(Some(Tab::View), false);
        let widths = [70.0, 110.0, 80.0, 70.0, 120.0, 100.0, 90.0, 90.0];
        assert_eq!(hints.len(), widths.len());
        let line = log_line_room(400.0, 400.0, LOG_LINE_CAP_VIEW).unwrap();
        let left = 400.0 - line - status::GAP;
        let fit = status::hints_that_fit(&widths, status::HINT_GAP, left);
        assert!(fit > 0 && fit < hints.len(), "{fit}");
        // No room for a hint: the line is cut, to 48 at least, then goes.
        assert_eq!(log_line_room(60.0, 400.0, LOG_LINE_CAP), Some(60.0));
        assert_eq!(log_line_room(47.0, 400.0, LOG_LINE_CAP), None);
    }

    /// Asking for a dock's panel brings it into sight: its dock unfolded,
    /// its tab in front; the switch of the tab in front closes the dock.
    #[test]
    fn panels_come_into_sight() {
        let mut s = Shell::default();
        s.frame_yields(1200.0, 600.0);
        assert!(s.left_folded() && !s.panel_shown(Tab::Browser));
        s.show_panel(Tab::Outliner);
        assert!(!s.left_folded());
        // (Narrow: the Right Dock's panels are tabs.)
        assert!(s.panel_shown(Tab::Inspector) && !s.panel_shown(Tab::Lighting));
        s.show_panel(Tab::Lighting);
        assert!(s.panel_shown(Tab::Lighting));
        assert!(s.panel_shown(Tab::Timeline));
        s.show_panel(Tab::Log);
        assert!(s.panel_shown(Tab::Log) && !s.panel_shown(Tab::Timeline));
        // A window crossing the width gives the user's fold up.
        s.frame_yields(1600.0, 900.0);
        assert!(!s.left_folded());
        s.frame_yields(1200.0, 600.0);
        assert!(s.left_folded());
        s.reset();
        assert_eq!(s.bottom_tab, Tab::Timeline);
    }

    /// The whole window draws without a GPU at the board sizes, the least
    /// window and the wide one, in both themes, with an ASCII model open;
    /// every control of the 3D View's and the ASCII editor's headers is
    /// on the row or in More, none cut.
    #[test]
    fn the_shell_draws_at_every_size() {
        use mg_uikit::header::{self, Place as At};
        let dir = std::env::temp_dir().join(format!("mgv-ui-shell-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("tri.mdl");
        std::fs::write(&file, MODEL).unwrap();
        for theme in mg_uikit::Theme::ALL {
            for size in
                [(640.0, 480.0), (800.0, 600.0), (1200.0, 800.0), (1500.0, 900.0), (3440.0, 1440.0)]
            {
                let ctx = egui::Context::default();
                header::record_headers(&ctx);
                let mut v = viewer();
                v.settings.theme = theme;
                v.actions.push(Action::Open(file.clone()));
                let size = egui::vec2(size.0, size.1);
                run(&ctx, &mut v, size, 4);
                assert!(v.buffer.is_some(), "{size:?}: the ASCII opened");
                for name in [crate::view3d::HEADER, crate::editor::HEADER] {
                    let places = header::places(&ctx, name);
                    assert!(!places.is_empty(), "{size:?}: {name} drawn");
                    for (item, at) in places {
                        assert_ne!(at, At::Cut, "{size:?}: {name}'s {item} cut");
                    }
                }
                let places = header::places(&ctx, crate::view3d::HEADER);
                let overlays = places.iter().find(|(n, _)| n == "Overlays").map(|(_, at)| *at);
                match size.x >= kit::WIDE_FROM {
                    true => assert_eq!(overlays, Some(At::Unfolded), "{size:?}"),
                    false => assert_ne!(overlays, Some(At::Unfolded), "{size:?}"),
                }
                assert_eq!(v.shell.laid_out, Some(size.x >= kit::WIDE_FROM));
            }
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// What `tests/app.rs` takes for granted at the board's size (it runs
    /// on a GPU, so not here): its model lints clean and its broken one
    /// does not, no game folder is found at `/nonexistent`, and the
    /// controls it clicks by name are on their headers' rows, not in More.
    #[test]
    fn what_the_window_tests_click_is_on_the_rows() {
        use mg_uikit::header::{self, Place as At};
        const APP_MODEL: &str = "newmodel tri\nsetsupermodel tri NULL\nclassification character\n\
            beginmodelgeom tri\nnode dummy tri\n  parent NULL\nendnode\n\
            node trimesh plane\n  parent tri\n  verts 3\n    0 0 0\n    1 0 0\n    0 1 0\n\
            faces 1\n    0 1 2 1 0 0 0 0\nendnode\n\
            node dummy hook\n  parent tri\n  position 0 0 2\nendnode\n\
            endmodelgeom tri\ndonemodel tri\n";
        let counts = |t: &str| crate::editor::counts(&mg_mdl::lint::check(t));
        assert_eq!(counts(APP_MODEL), (0, 0));
        let broken =
            APP_MODEL.replace("endmodelgeom tri", "node\nendnode\nendnode\nendmodelgeom tri");
        let (e, w) = counts(&broken);
        assert!(e > 0 && w > 0);
        let mut v = viewer();
        assert!(v.lib.install().is_none());
        let dir = std::env::temp_dir().join(format!("mgv-ui-rows-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("tri.mdl"), APP_MODEL).unwrap();
        v.actions.push(Action::Open(dir.join("tri.mdl")));
        let ctx = egui::Context::default();
        header::record_headers(&ctx);
        run(&ctx, &mut v, egui::vec2(1600.0, 900.0), 4);
        let on_row = |name: &str, item: &str| {
            let at = header::places(&ctx, name).into_iter().find(|(n, _)| n == item).map(|p| p.1);
            assert!(matches!(at, Some(At::Row | At::Icon)), "{name}'s {item}: {at:?}");
        };
        for item in ["Frame", "Grid", "Axes", "Overlays", "Shaders", "Shadow"] {
            on_row(crate::view3d::HEADER, item);
        }
        for item in ["Save", "Compile", "Compile and View"] {
            on_row(crate::editor::HEADER, item);
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A window opened narrower than 2560 and widened past it takes the
    /// three editor groups, unless the user moved a tab.
    #[test]
    fn a_window_widened_to_the_wide_layout_takes_three_groups() {
        let ctx = egui::Context::default();
        let mut v = viewer();
        run(&ctx, &mut v, egui::vec2(1440.0, 900.0), 2);
        assert_eq!(groups(&v.dock), groups(&crate::default_dock_for(false)));
        run(&ctx, &mut v, egui::vec2(3440.0, 1440.0), 2);
        assert_eq!(groups(&v.dock), groups(&crate::default_dock_for(true)));
        run(&ctx, &mut v, egui::vec2(1600.0, 900.0), 2);
        assert_eq!(groups(&v.dock), groups(&crate::default_dock_for(false)));
        // A tab closed: the layout is the user's now, and stays.
        v.close_tab(Tab::Texture);
        run(&ctx, &mut v, egui::vec2(3440.0, 1440.0), 2);
        assert!(v.dock.find_tab(&Tab::Texture).is_none());
        assert_eq!(groups(&v.dock).len(), 2);
    }

    /// The wide layout's editor area: 3D View, Texture and ASCII in three
    /// groups; the board's: the 3D View, Texture and ASCII in a second.
    #[test]
    fn the_editor_groups() {
        let groups = |wide| {
            let dock = crate::default_dock_for(wide);
            dock.iter_leaves().map(|(_, l)| l.tabs.clone()).collect::<Vec<_>>()
        };
        assert_eq!(groups(false), [vec![Tab::View], vec![Tab::Texture, Tab::Editor]]);
        assert_eq!(groups(true), [vec![Tab::View], vec![Tab::Texture], vec![Tab::Editor]]);
    }

    /// A closed Texture or ASCII tab comes back beside the other in the
    /// second group, not among the 3D View's; a dock panel is brought into
    /// its dock, never into the editor area.
    #[test]
    fn tabs_come_back_where_they_were() {
        let mut v = viewer();
        v.close_tab(Tab::Texture);
        assert!(v.dock.find_tab(&Tab::Texture).is_none());
        v.focus(Tab::Texture);
        let at = v.dock.find_tab(&Tab::Texture).unwrap();
        let with = v.dock.find_tab(&Tab::Editor).unwrap();
        assert_eq!(at.node, with.node);
        v.close_tab(Tab::Texture);
        v.close_tab(Tab::Editor);
        v.focus(Tab::Editor);
        let at = v.dock.find_tab(&Tab::Editor).unwrap();
        assert_ne!(at.node, v.dock.find_tab(&Tab::View).unwrap().node);
        v.focus(Tab::Inspector);
        v.focus(Tab::Log);
        assert_eq!(v.dock.iter_all_tabs().count(), 2, "no panel among the documents");
        assert!(v.shell.panel_shown(Tab::Log));
    }
    /// The layout goes into the settings as the user leaves it and comes
    /// back from them at the next run (through RON, as eframe keeps it):
    /// a panel dragged to another dock, the sizes chosen, a dock folded and
    /// the Bottom Dock's tab, the editor area's groups.
    #[test]
    fn the_layout_round_trips_through_the_settings() {
        let ctx = egui::Context::default();
        let board = egui::vec2(1600.0, 900.0);
        let mut v = viewer();
        run(&ctx, &mut v, board, 2);
        assert_eq!(v.settings.layout, Layout::default(), "nothing chosen yet");
        v.shell.move_panel(Tab::Log, Place::Left);
        v.shell.set_folded(Side::Right, true);
        v.shell.show_panel(Tab::Effects);
        v.shell.sizes.left = Some(300);
        v.shell.sizes.bottom = Some(240);
        v.close_tab(Tab::Texture);
        run(&ctx, &mut v, board, 2);
        let kept = v.settings.layout.clone();
        assert_eq!(kept.docks.get("log").map(String::as_str), Some("Left"));
        assert!(kept.right_folded && !kept.left_folded);
        assert_eq!(kept.bottom_front.as_deref(), Some("effects"));
        assert_eq!((kept.left_width, kept.bottom_height), (Some(300), Some(240)));
        assert_eq!(kept.editor.len(), 2, "the groups, without the Texture");
        // The next run.
        let ctx = egui::Context::default();
        let mut w = next_run(&v);
        run(&ctx, &mut w, board, 3);
        assert_eq!(w.settings.layout, kept);
        assert_eq!(w.shell.place(Tab::Log), Place::Left);
        assert_eq!(w.shell.panels_at(Place::Left), [Tab::Browser, Tab::Outliner, Tab::Log]);
        assert!(w.shell.right_folded() && !w.shell.left_folded());
        assert_eq!(w.shell.bottom_front(), Some(Tab::Effects));
        assert!(w.dock.find_tab(&Tab::Texture).is_none());
        assert_eq!(groups(&w.dock), groups(&v.dock));
        // Drawn as chosen.
        let left = w.shell.zone(Place::Left).unwrap();
        assert!((left.width() - 300.0).abs() < 1.5, "{left:?}");
        let bottom = w.shell.zone(Place::Bottom).unwrap();
        assert!((bottom.height() - 240.0).abs() < 1.5, "{bottom:?}");
        // Reset Layout puts it all back, and the settings with it.
        w.actions.push(Action::ResetLayout);
        run(&ctx, &mut w, board, 2);
        assert_eq!(w.settings.layout, Layout::default());
        assert_eq!(w.shell.place(Tab::Log), Place::Bottom);
    }

    /// A layout that does not read, or of another version, gives the
    /// default window; one naming panels or tabs this viewer does not
    /// have, or a panel in the editor area, keeps what it can.
    #[test]
    fn an_old_or_garbage_layout_opens_the_default() {
        let board = egui::vec2(1600.0, 900.0);
        for text in [
            "(layout: 17, show_grid: false)",
            "(layout: (version: 0, left_width: Some(500)), show_grid: false)",
            "(layout: (docks: {\"log\": 3}), show_grid: false)",
            "(show_grid: false)",
        ] {
            let settings: Settings = ron::from_str(text).unwrap();
            assert!(!settings.show_grid);
            let mut v = Viewer::new(settings, Box::new(NoDialogs));
            run(&egui::Context::default(), &mut v, board, 2);
            assert_eq!(v.settings.layout, Layout::default(), "{text}");
            assert_eq!(groups(&v.dock), groups(&crate::default_dock_for(false)), "{text}");
        }
        let mut layout = Layout::default();
        layout.docks.insert("log".into(), "Left".into());
        layout.docks.insert("timeline".into(), "Ceiling".into());
        layout.docks.insert("problems".into(), "Right".into());
        layout.docks.insert("view".into(), "Left".into());
        layout.bottom_front = Some("problems".into());
        layout.left_width = Some(0);
        layout.editor = vec![EditorGroup {
            tabs: vec!["view".into(), "inspector".into()],
            ..EditorGroup::default()
        }];
        let settings = Settings { layout, ..viewer().settings };
        let mut v = Viewer::new(settings, Box::new(NoDialogs));
        run(&egui::Context::default(), &mut v, board, 2);
        assert_eq!(v.shell.place(Tab::Log), Place::Left);
        assert_eq!(v.shell.place(Tab::Timeline), Place::Bottom);
        assert_eq!(v.shell.place(Tab::View), Place::Editor);
        assert_eq!(v.shell.bottom_front(), Some(Tab::Timeline));
        assert_eq!(v.shell.sizes.left, None, "no width of nothing");
        assert_eq!(groups(&v.dock), groups(&crate::default_dock_for(false)));
        // Groups without the 3D View get it back, in a group of its own.
        let ascii = vec![EditorGroup { tabs: vec!["ascii".into()], ..EditorGroup::default() }];
        let dock = editor_dock(&ascii).unwrap();
        assert_eq!(groups(&dock), [vec![Tab::View], vec![Tab::Editor]]);
        let twice = [ascii.clone(), ascii].concat();
        assert!(editor_dock(&twice).is_none());
    }

    /// The Resources panel dragged by its header from the Left Dock onto
    /// the Right Dock stands there, under the Right Dock's own panels; it
    /// stays there at the next run.
    #[test]
    fn a_panel_dragged_from_left_to_right_stays_after_a_save_and_load() {
        let ctx = egui::Context::default();
        let board = egui::vec2(1600.0, 900.0);
        let mut v = viewer();
        run(&ctx, &mut v, board, 3);
        let left = v.shell.zone(Place::Left).unwrap();
        let right = v.shell.zone(Place::Right(0)).unwrap();
        // (The Resources panel's header, at the top of the Left Dock.)
        let header = left.left_top() + egui::vec2(40.0, metrics::DOCK_TABS / 2.0);
        drag(&ctx, &mut v, board, header, right.center());
        assert_eq!(v.shell.place(Tab::Browser), Place::Right(0));
        assert_eq!(v.shell.panels_at(Place::Left), [Tab::Outliner]);
        assert_eq!(
            v.shell.panels_at(Place::Right(0)),
            [Tab::Browser, Tab::Inspector, Tab::Lighting]
        );
        assert!(v.shell.dragging.is_none());
        assert_eq!(v.settings.layout.docks.get("resources").map(String::as_str), Some("Right"));
        // A drag let go over the editor area, away from any dock, moves
        // nothing.
        let right = v.shell.zone(Place::Right(0)).unwrap();
        let header = right.left_top() + egui::vec2(40.0, metrics::DOCK_TABS / 2.0);
        let middle = v.shell.editor_rect.unwrap().center();
        drag(&ctx, &mut v, board, header, middle);
        assert_eq!(v.shell.place(Tab::Browser), Place::Right(0));
        // The next run.
        let ctx = egui::Context::default();
        let mut w = next_run(&v);
        run(&ctx, &mut w, board, 3);
        assert_eq!(w.shell.place(Tab::Browser), Place::Right(0));
        assert_eq!(w.shell.panels_at(Place::Left), [Tab::Outliner]);
        // Dragged back onto its own dock: nothing kept for it.
        let right = w.shell.zone(Place::Right(0)).unwrap();
        let left = w.shell.zone(Place::Left).unwrap();
        let header = right.left_top() + egui::vec2(40.0, metrics::DOCK_TABS / 2.0);
        drag(&ctx, &mut w, board, header, left.center());
        assert_eq!(w.shell.place(Tab::Browser), Place::Left);
        assert!(w.settings.layout.docks.is_empty());
    }

    /// A Bottom Dock tab dragged onto a folded dock's band unfolds it
    /// there; docks emptied of panels go, and a panel is dropped on the
    /// band of the editor area along them; the Status Bar keeps its
    /// switches for what the Bottom Dock holds.
    #[test]
    fn panels_move_to_folded_and_empty_docks() {
        let ctx = egui::Context::default();
        let board = egui::vec2(1600.0, 900.0);
        let mut v = viewer();
        v.shell.set_folded(Side::Left, true);
        run(&ctx, &mut v, board, 3);
        let bottom = v.shell.zone(Place::Bottom).unwrap();
        // (The Timeline's tab, the first of the Bottom Dock's tab bar.)
        let tab = bottom.left_top() + egui::vec2(10.0, metrics::DOCK_TABS / 2.0);
        let band = v.shell.editor_rect.unwrap().left_center() + egui::vec2(20.0, 0.0);
        assert_eq!(drop_target(&v.shell, band).map(|t| t.0), Some(Place::Left));
        drag(&ctx, &mut v, board, tab, band);
        assert_eq!(v.shell.place(Tab::Timeline), Place::Left);
        assert!(!v.shell.left_folded(), "unfolded where it was dropped");
        // Every panel out of the Bottom Dock and the Right Dock: neither is
        // drawn, nothing breaks, and the bands take panels back.
        for tab in [Tab::Effects, Tab::Log, Tab::Inspector, Tab::Lighting] {
            v.shell.move_panel(tab, Place::Left);
        }
        run(&ctx, &mut v, board, 2);
        assert!(v.shell.zone(Place::Bottom).is_none() && v.shell.zone(Place::Right(0)).is_none());
        assert_eq!(v.shell.bottom_front(), None);
        let e = v.shell.editor_rect.unwrap();
        let low = egui::pos2(e.center().x, e.bottom() - 10.0);
        assert_eq!(drop_target(&v.shell, low).map(|t| t.0), Some(Place::Bottom));
        let side = egui::pos2(e.right() - 10.0, e.center().y);
        assert_eq!(drop_target(&v.shell, side).map(|t| t.0), Some(Place::Right(0)));
        // Narrow and wide windows with them so.
        for size in [egui::vec2(800.0, 600.0), egui::vec2(3440.0, 1440.0)] {
            run(&ctx, &mut v, size, 2);
        }
        v.shell.move_panel(Tab::Log, Place::Bottom);
        run(&ctx, &mut v, board, 2);
        assert_eq!(v.shell.panels_at(Place::Bottom), [Tab::Log]);
        assert_eq!(v.shell.bottom_front(), Some(Tab::Log));
    }

    /// The window test's steps (`tests/app.rs`'s
    /// `panels_are_dragged_between_docks`) without a GPU: the Inspector
    /// dragged by the point `first_header` gives onto the Bottom Dock's
    /// zone, then Lighting's header right-clicked at its `first_header`
    /// and its menu's Move to the Left Dock clicked.
    #[test]
    fn the_window_tests_drag_and_menu_points_take_the_panels() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let board = egui::vec2(1600.0, 900.0);
        let mut v = viewer();
        run(&ctx, &mut v, board, 3);
        let from = v.shell.first_header(Place::Right(0)).unwrap();
        let to = v.shell.zone(Place::Bottom).unwrap().center();
        drag(&ctx, &mut v, board, from, to);
        assert_eq!(v.shell.place(Tab::Inspector), Place::Bottom);
        assert!(v.shell.panel_shown(Tab::Inspector));
        let at = v.shell.first_header(Place::Right(0)).unwrap();
        let secondary = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(&ctx, &mut v, board, vec![egui::Event::PointerMoved(at)]);
        frame(&ctx, &mut v, board, vec![secondary(true)]);
        frame(&ctx, &mut v, board, vec![secondary(false)]);
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, board)),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| v.ui(ui));
        out.textures_delta.clear();
        v.run_actions();
        let item = crate::panels::tests::named_rect(&out, "Move to the Left Dock")
            .expect("the header's menu");
        let click = |pressed| egui::Event::PointerButton {
            pos: item.center(),
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(&ctx, &mut v, board, vec![egui::Event::PointerMoved(item.center())]);
        frame(&ctx, &mut v, board, vec![click(true)]);
        frame(&ctx, &mut v, board, vec![click(false)]);
        run(&ctx, &mut v, board, 2);
        assert_eq!(v.shell.place(Tab::Lighting), Place::Left);
        assert!(v.shell.panels_at(Place::Right(0)).is_empty());
    }

    /// The Inspector's room in the default layout (the Right Dock at its
    /// 352) holds the Creature grid's two pairs a row at the first run's
    /// 1440 by 900 and the boards' 1600 by 900; the tabbed dock (280,
    /// under 1400) one.
    #[test]
    fn the_inspector_holds_two_pairs_a_row_in_the_default_layout() {
        for (width, per_row) in [(1440.0, 2), (1600.0, 2), (1300.0, 1)] {
            let ctx = egui::Context::default();
            let mut v = viewer();
            run(&ctx, &mut v, egui::vec2(width, 900.0), 3);
            let room: f32 = ctx
                .data(|d| d.get_temp(egui::Id::new(crate::panels::INSPECTOR_ROOM)))
                .expect("the Inspector drawn");
            assert_eq!(crate::panels::pairs_per_row(room), per_row, "{width}: {room}");
        }
    }

    /// A layout kept from a wider window still yields to this one's width:
    /// the Left Dock folds under 1500 and the Right Dock is 280 and tabbed
    /// under 1400 whatever was kept, a dock wider than two fifths of the
    /// window is drawn at that, and the wide layout's columns and groups are
    /// the wide window's alone. The user's own fold is kept where the width
    /// does not fold; unfolding by hand in a narrow window is not kept.
    #[test]
    fn the_width_rules_fold_a_dock_whose_layout_came_from_a_wider_window() {
        let ctx = egui::Context::default();
        let mut v = viewer();
        run(&ctx, &mut v, egui::vec2(3440.0, 1440.0), 3);
        assert_eq!(groups(&v.dock).len(), 3);
        v.shell.sizes.left = Some(900);
        v.shell.sizes.right_wide = Some(1000);
        run(&ctx, &mut v, egui::vec2(3440.0, 1440.0), 2);
        assert!(v.settings.layout.editor.is_empty(), "the wide default is not kept");
        assert!(v.shell.zone(Place::Right(1)).is_some(), "two columns");
        // At 1200 by 800: the Left Dock folded, the Right Dock tabbed at
        // 280, the editor area the narrow default.
        let ctx = egui::Context::default();
        let mut w = next_run(&v);
        let small = egui::vec2(1200.0, 800.0);
        run(&ctx, &mut w, small, 3);
        assert!(w.shell.left_folded());
        assert!(w.shell.zone(Place::Left).unwrap().width() < 40.0, "the strip");
        let right = w.shell.zone(Place::Right(0)).unwrap();
        assert!((right.width() - kit::RIGHT_DOCK_NARROW).abs() < 2.0, "{right:?}");
        assert!(w.shell.zone(Place::Right(1)).is_none());
        assert_eq!(groups(&w.dock), groups(&crate::default_dock_for(false)));
        // Unfolded by hand here: not kept.
        w.shell.set_folded(Side::Left, false);
        run(&ctx, &mut w, small, 2);
        assert!(!w.shell.left_folded() && !w.settings.layout.left_folded);
        // At 1600: open, held to two fifths of the window (640, not 900).
        run(&ctx, &mut w, egui::vec2(1600.0, 900.0), 3);
        let left = w.shell.zone(Place::Left).unwrap();
        assert!(!w.shell.left_folded() && (left.width() - 640.0).abs() < 2.0, "{left:?}");
        assert_eq!(w.shell.sizes.left, Some(900), "kept as chosen");
        // The user's own fold, in a window that does not fold: kept, and
        // the narrow window folds it all the same.
        w.shell.set_folded(Side::Left, true);
        run(&ctx, &mut w, egui::vec2(1600.0, 900.0), 2);
        assert!(w.settings.layout.left_folded);
        let mut x = next_run(&w);
        run(&egui::Context::default(), &mut x, egui::vec2(1600.0, 900.0), 2);
        assert!(x.shell.left_folded());
        run(&egui::Context::default(), &mut x, egui::vec2(3440.0, 1440.0), 2);
        assert!(x.shell.left_folded(), "the user's fold at any width that does not fold");
    }
}
