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
//! [`place`] says, at the window's width. Nothing of the layout is kept
//! between runs (only `Settings` is); Reset Layout makes it again.

use egui::{Rect, Ui};
use mg_uikit::shell::{self as kit, DockTab, DockTabs, FoldStrip, PanelHeader, Side, Yields};
use mg_uikit::status::{self, Hint, Hints, StatusItem, StatusJob, StatusLog, StatusSwitch};
use mg_uikit::{Icon, IconButton, metrics};

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

/// The docks' panels, in the order they stand in each.
const PANELS: [Tab; 7] = [
    Tab::Browser,
    Tab::Outliner,
    Tab::Inspector,
    Tab::Log,
    Tab::Lighting,
    Tab::Timeline,
    Tab::Effects,
];

/// Where `tab` stands, in the wide layout or not (proposal section 9.17,
/// and the moves' "Viewer at 3440 by 1440": Inspector and Log in the
/// Right Dock's first column, Lighting and Effects in its second, the
/// Timeline alone in the Bottom Dock).
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

/// The panels at `at`, in their order: Timeline, Effects, Log in the
/// Bottom Dock (not [`PANELS`]' order, which stacks the columns).
pub fn panels_at(at: Place, wide: bool) -> Vec<Tab> {
    match at {
        Place::Bottom => [Tab::Timeline, Tab::Effects, Tab::Log]
            .into_iter()
            .filter(|t| place(*t, wide) == at)
            .collect(),
        _ => PANELS.into_iter().filter(|t| place(*t, wide) == at).collect(),
    }
}

/// The shell's state: what the window's width yields, the docks folded
/// or open, the panels in front, and the panel under the pointer.
#[derive(Debug)]
pub struct Shell {
    /// What the shell yields at this frame's widths.
    pub yields: Yields,
    /// The Left Dock folded (`Some(true)`) or open by the user's hand;
    /// `None`: as the window's width says.
    left_folded: Option<bool>,
    /// The Right Dock's, likewise.
    right_folded: Option<bool>,
    /// The folds the window's width said when the user's were taken (a
    /// window crossing a width gives the user's choice up).
    folds_seen: (bool, bool),
    /// The Bottom Dock is open.
    pub bottom_open: bool,
    /// The Bottom Dock's tab in front.
    pub bottom_tab: Tab,
    /// The Right Dock's panel in front where its panels are tabs.
    pub right_tab: Tab,
    /// The panel the pointer was over at the last frame (its hints).
    pub hovered: Option<Tab>,
    /// The panel the pointer is over at this frame.
    pub(crate) hovering: Option<Tab>,
    /// The editor area's width at the last frame.
    pub(crate) editor_width: Option<f32>,
    /// Whether the editor area was laid out for the wide layout (`None`
    /// before the first frame).
    pub(crate) laid_out: Option<bool>,
    /// Bumped by Reset Layout: the docks' sizes, kept under ids salted
    /// with it, start again.
    generation: u32,
}

impl Default for Shell {
    fn default() -> Shell {
        Shell {
            yields: Yields::of(1600.0, 1600.0),
            left_folded: None,
            right_folded: None,
            folds_seen: (false, false),
            bottom_open: true,
            bottom_tab: Tab::Timeline,
            right_tab: Tab::Inspector,
            hovered: None,
            hovering: None,
            editor_width: None,
            laid_out: None,
            generation: 0,
        }
    }
}

impl Shell {
    /// Whether the Left Dock shows as its strip.
    pub fn left_folded(&self) -> bool {
        self.left_folded.unwrap_or(self.yields.left_dock_folded)
    }

    /// Whether the Right Dock shows as its strip.
    pub fn right_folded(&self) -> bool {
        self.right_folded.unwrap_or(self.yields.right_dock_folded)
    }

    /// The yields of a window `window` wide whose editor area is `editor`
    /// wide. Where the window crosses a width that folds a dock or
    /// unfolds it, the user's own fold of that dock gives way.
    pub(crate) fn frame_yields(&mut self, window: f32, editor: f32) {
        self.yields = Yields::of(window, editor);
        let folds = (self.yields.left_dock_folded, self.yields.right_dock_folded);
        if folds.0 != self.folds_seen.0 {
            self.left_folded = None;
        }
        if folds.1 != self.folds_seen.1 {
            self.right_folded = None;
        }
        self.folds_seen = folds;
    }

    /// The layout as a new window has it: both docks as the width says,
    /// the Bottom Dock open on the Timeline, the docks' sizes their
    /// defaults.
    pub(crate) fn reset(&mut self) {
        self.left_folded = None;
        self.right_folded = None;
        self.bottom_open = true;
        self.bottom_tab = Tab::Timeline;
        self.right_tab = Tab::Inspector;
        self.generation += 1;
    }

    /// A dock part's id, salted with the layout's generation.
    fn id(&self, name: impl std::hash::Hash + std::fmt::Debug) -> egui::Id {
        egui::Id::new(("mgv-shell", name, self.generation))
    }

    /// Brings a dock's panel into sight: its dock unfolded or opened, the
    /// panel in front where it is a tab.
    pub fn show_panel(&mut self, tab: Tab) {
        match place(tab, self.yields.wide) {
            Place::Left => self.left_folded = Some(false),
            Place::Right(_) => {
                self.right_folded = Some(false);
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
        match place(tab, self.yields.wide) {
            Place::Left => !self.left_folded(),
            Place::Right(_) => {
                !self.right_folded()
                    && (!self.yields.right_dock_tabbed || self.right_front() == tab)
            }
            Place::Bottom => self.bottom_open && self.bottom_front() == tab,
            Place::Editor => true,
        }
    }

    /// The Bottom Dock's tab in front (its first where the one chosen
    /// stands elsewhere now).
    fn bottom_front(&self) -> Tab {
        let tabs = panels_at(Place::Bottom, self.yields.wide);
        if tabs.contains(&self.bottom_tab) { self.bottom_tab } else { tabs[0] }
    }

    /// The Right Dock's panel in front where its panels are tabs.
    fn right_front(&self) -> Tab {
        let tabs = panels_at(Place::Right(0), self.yields.wide);
        if tabs.contains(&self.right_tab) { self.right_tab } else { tabs[0] }
    }

    /// At the frame's end: the panel the pointer was over.
    pub(crate) fn end_frame(&mut self) {
        self.hovered = self.hovering.take();
    }
}

/// The editor area's groups (each one's tabs), its tabs in front aside.
pub(crate) fn groups(dock: &egui_dock::DockState<Tab>) -> Vec<Vec<Tab>> {
    dock.iter_leaves().map(|(_, l)| l.tabs.clone()).collect()
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
    let switches = panels_at(Place::Bottom, shell.yields.wide);
    let front = shell.bottom_open.then(|| shell.bottom_front());
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
        if shell.bottom_open && shell.bottom_front() == tab {
            shell.bottom_open = false;
        } else {
            shell.show_panel(tab);
        }
    }
}

// ------------------------------------------------------------------ docks

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

/// A panel under its header (named "Resources panel"), the dock's fold
/// button among its tools where `fold` is given; whether it was pressed.
fn headed(app: &mut Viewer, ui: &mut Ui, tab: Tab, fold: Option<(Side, &str)>) -> bool {
    let name = format!("{} panel", tab.title());
    let header = PanelHeader::new(tab.title()).name(name);
    let (folded, tabbed) = header
        .show(ui, |ui| {
            let folded =
                fold.is_some_and(|(side, dock)| kit::fold_button(ui, side, dock).clicked());
            // (Resources: its gallery as a tab, the board's icon.)
            let tabbed = tab == Tab::Browser && {
                let button = IconButton::new(Icon::OpenInTab, crate::gallery::OPEN_AS_TAB);
                let tip = "Open the gallery as a tab in the editor area";
                ui.add(button.icon_only(true)).on_hover_text(tip).clicked()
            };
            (folded, tabbed)
        })
        .inner;
    body(app, ui, tab);
    if tabbed {
        app.open_gallery_tab();
    }
    folded
}

/// The panels of a dock's column, each under its header, the first over
/// the second at a seam the user drags; whether the fold button (in the
/// first one's header, where `fold` is given) was pressed.
fn stack(
    app: &mut Viewer,
    ui: &mut Ui,
    key: &str,
    tabs: &[Tab],
    fold: Option<(Side, &str)>,
) -> bool {
    match tabs {
        [] => false,
        [one] => headed(app, ui, *one, fold),
        [first, rest @ ..] => {
            let whole = ui.max_rect().height();
            let most = (whole - PANEL_LEAST).max(PANEL_LEAST);
            let at_first = (whole * UPPER_SHARE).round().clamp(PANEL_LEAST, most);
            let mut folded = false;
            egui::Panel::top(app.shell.id(key))
                .resizable(true)
                .default_size(at_first)
                .size_range(PANEL_LEAST..=most)
                .frame(egui::Frame::new())
                .show(ui, |ui| folded = headed(app, ui, *first, fold));
            folded | stack(app, ui, &format!("{key}+"), rest, None)
        }
    }
}

/// A folded dock: its strip of its panels' icons under the chevron; the
/// panel clicked (`Some(None)`: the chevron).
fn strip(app: &Viewer, ui: &mut Ui, dock: &str, side: Side, tabs: &[Tab]) -> Option<Option<Tab>> {
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
    shown.inner.map(|click| match click {
        kit::StripClick::Unfold => None,
        kit::StripClick::Panel(i) => tabs.get(i).copied(),
    })
}

/// The widest a side dock is dragged to: two fifths of the window.
fn side_most(ui: &Ui, least: f32) -> f32 {
    (ui.ctx().content_rect().width() * 0.4).max(least)
}

/// The Left Dock: Resources over the Outliner, or its strip.
pub(crate) fn left_dock(app: &mut Viewer, ui: &mut Ui) {
    let tabs = panels_at(Place::Left, app.shell.yields.wide);
    if app.shell.left_folded() {
        if strip(app, ui, LEFT_DOCK, Side::Left, &tabs).is_some() {
            app.shell.left_folded = Some(false);
        }
        return;
    }
    let t = mg_uikit::tokens(ui.ctx());
    // (Wider in Gallery mode, three pictures a row: the board's 440.)
    let gallery = app.settings.resources_view == crate::settings::ResourcesView::Gallery;
    let width = match gallery {
        true => app.shell.yields.left_dock_width.max(GALLERY_DOCK),
        false => app.shell.yields.left_dock_width,
    };
    let most = side_most(ui, SIDE_LEAST);
    let wide = app.shell.yields.wide;
    let mut folded = false;
    egui::Panel::left(app.shell.id(("left-dock", wide, gallery)))
        .resizable(true)
        .default_size(width.min(most))
        .size_range(SIDE_LEAST..=most)
        .frame(egui::Frame::new().fill(t.panel))
        .show(ui, |ui| {
            mg_uikit::name_group(ui, LEFT_DOCK);
            folded = stack(app, ui, "left", &tabs, Some((Side::Left, LEFT_DOCK)));
        });
    if folded {
        app.shell.left_folded = Some(true);
    }
}

/// The Right Dock: Inspector over Lighting; their tabs in a narrow
/// window; two columns in the wide layout; or its strip.
pub(crate) fn right_dock(app: &mut Viewer, ui: &mut Ui) {
    let y = app.shell.yields;
    let columns: Vec<Vec<Tab>> = match y.wide {
        true => vec![panels_at(Place::Right(0), true), panels_at(Place::Right(1), true)],
        false => vec![panels_at(Place::Right(0), false)],
    };
    if app.shell.right_folded() {
        let all: Vec<Tab> = columns.concat();
        if let Some(chosen) = strip(app, ui, RIGHT_DOCK, Side::Right, &all) {
            app.shell.right_folded = Some(false);
            if let Some(tab) = chosen {
                app.shell.right_tab = tab;
            }
        }
        return;
    }
    let t = mg_uikit::tokens(ui.ctx());
    let narrow = y.right_dock_narrow;
    // (Each form of the dock keeps its own width: a window narrowed to
    // the tabbed dock and widened again finds it as it was.)
    let form = match (narrow, y.wide) {
        (true, _) => 0,
        (false, false) => 1,
        (false, true) => 2,
    };
    let least = if y.wide { 2.0 * kit::RIGHT_DOCK_NARROW + 1.0 } else { SIDE_LEAST };
    let mut panel = egui::Panel::right(app.shell.id(("right-dock", form)))
        .frame(egui::Frame::new().fill(t.panel));
    panel = match narrow {
        true => panel.exact_size(kit::RIGHT_DOCK_NARROW).resizable(false),
        false => panel
            .resizable(true)
            .default_size(y.right_dock_width)
            .size_range(least..=side_most(ui, least)),
    };
    let mut folded = false;
    panel.show(ui, |ui| {
        let fold = Some((Side::Right, RIGHT_DOCK));
        if y.right_dock_tabbed {
            let tabs = &columns[0];
            let front = app.shell.right_front();
            let mut i = tabs.iter().position(|t| *t == front).unwrap_or(0);
            let button = IconButton::new(Icon::ChevronRight, "").icon_only(true).width(ui);
            let mut bar = DockTabs::new(RIGHT_DOCK).tools_width(button);
            for tab in tabs {
                bar = bar.with(DockTab::new(tab.title()).icon(tab.icon()));
            }
            let out =
                bar.show(ui, &mut i, |ui| kit::fold_button(ui, Side::Right, RIGHT_DOCK).clicked());
            folded = out.inner;
            app.shell.right_tab = tabs[i];
            body(app, ui, tabs[i]);
            return;
        }
        mg_uikit::name_group(ui, RIGHT_DOCK);
        if let [first, second] = columns.as_slice() {
            // Two columns, a line between them (`Yields::right_first_column`).
            let width = Yields::right_first_column(ui.max_rect().width());
            egui::Panel::left(app.shell.id("right-column"))
                .exact_size(width)
                .resizable(false)
                .frame(egui::Frame::new())
                .show(ui, |ui| folded = stack(app, ui, "right-0", first, fold));
            stack(app, ui, "right-1", second, None);
        } else {
            folded = stack(app, ui, "right", &columns[0], fold);
        }
    });
    if folded {
        app.shell.right_folded = Some(true);
    }
}

/// The Bottom Dock, under the editor area, unless closed: its panels as
/// tabs.
pub(crate) fn bottom_dock(app: &mut Viewer, ui: &mut Ui) {
    let tabs = panels_at(Place::Bottom, app.shell.yields.wide);
    if !app.shell.bottom_open || tabs.is_empty() {
        return;
    }
    let t = mg_uikit::tokens(ui.ctx());
    let most = (ui.ctx().content_rect().height() * 0.5).max(BOTTOM_LEAST);
    egui::Panel::bottom(app.shell.id("bottom-dock"))
        .resizable(true)
        .default_size(metrics::BOTTOM_DOCK)
        .size_range(BOTTOM_LEAST..=most)
        .frame(egui::Frame::new().fill(t.panel))
        .show(ui, |ui| {
            let front = app.shell.bottom_front();
            let mut i = tabs.iter().position(|t| *t == front).unwrap_or(0);
            let mut bar = DockTabs::new(BOTTOM_DOCK);
            for tab in &tabs {
                bar = bar.with(DockTab::new(tab.title()).icon(tab.icon()));
            }
            let _ = bar.show(ui, &mut i, |_| {});
            app.shell.bottom_tab = tabs[i];
            body(app, ui, tabs[i]);
        });
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
/// close button (on the tab in front of its group and the one under the
/// pointer, for Texture and ASCII), the lines between the tabs and the
/// accent line along the top of the focused group's tab in front.
fn doc_tab_parts(app: &mut Viewer, ui: &mut Ui, buttons: &[TabButton]) {
    use mg_uikit::icons::ButtonState;
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
            let builder = egui::UiBuilder::new().layer_id(b.layer).max_rect(parts.close);
            let r = ui
                .scope_builder(builder, |ui| {
                    ui.set_clip_rect(b.shown);
                    ui.interact(parts.close, id, egui::Sense::click())
                })
                .inner;
            let name = format!("Close {}", b.tab.title());
            r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name));
            state = Some(if r.is_pointer_button_down_on() {
                ButtonState::Pressed
            } else if r.hovered() || r.has_focus() {
                ButtonState::Hovered
            } else {
                ButtonState::Rest
            });
            let r = r.on_hover_text("Close this tab");
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
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, size)),
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| v.ui(ui));
            out.textures_delta.clear();
            v.run_actions();
        }
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
}
