//! The side panels: the resource browser, the node outliner, the
//! inspector, the animation timeline and the log.

use std::collections::HashMap;
use std::sync::Arc;

use egui::Ui;
use mg_core::ResType;
use mg_mdl::{MeshExtra, NodeKind};
use mg_resman::ResKey;
use mgv_library::Kind;
use mgv_stage::{ActorId, PlayMode};

use crate::gallery::Cells;
use crate::{Action, Level, Selection, Viewer};
use mg_uikit::{TokensExt, field_label, section_heading};

/// A section's heading after other content: the kit's [`section_heading`]
/// under its `SECTION_GAP` (the kit has the heading, not the room above it).
pub(crate) fn next_section(ui: &mut egui::Ui, text: impl Into<String>) -> egui::Response {
    ui.add_space(mg_uikit::form::SECTION_GAP);
    section_heading(ui, text)
}

/// Which resources the browser lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Filter {
    #[default]
    Models,
    Textures,
    Blueprints,
    Walkmeshes,
    Materials,
    /// `appearance.2da` rows, as creatures.
    Creatures,
    All,
}

impl Filter {
    const ALL: [Filter; 7] = [
        Filter::Models,
        Filter::Creatures,
        Filter::Textures,
        Filter::Blueprints,
        Filter::Walkmeshes,
        Filter::Materials,
        Filter::All,
    ];

    fn name(self) -> &'static str {
        match self {
            Filter::Models => "Models",
            Filter::Textures => "Textures",
            Filter::Blueprints => "Blueprints",
            Filter::Walkmeshes => "Walkmeshes",
            Filter::Materials => "Materials and TXI",
            Filter::Creatures => "Creatures (appearance)",
            Filter::All => "Everything shown",
        }
    }

    fn admits(self, t: ResType) -> bool {
        let k = Kind::of(t);
        match self {
            Filter::Models => k == Kind::Model,
            Filter::Textures => matches!(k, Kind::Texture | Kind::Plt),
            Filter::Blueprints => k == Kind::Blueprint,
            Filter::Walkmeshes => k == Kind::Walkmesh,
            Filter::Materials => matches!(k, Kind::Material | Kind::Txi),
            Filter::Creatures => false,
            Filter::All => !matches!(k, Kind::Other | Kind::Archive | Kind::TwoDa),
        }
    }
}

/// What a listing was made for: the library's generation, the query and the
/// filter.
type Listed = (u64, String, Filter);

/// The resources listed and the layers they are from, shared with the
/// galleries.
pub(crate) type Items = Arc<Vec<(ResKey, String)>>;

/// Keys in a list that a click gave the keyboard (its `id`, registered by
/// [`list_widget`]): Up and Down step, Page Up and Down move a page, Home
/// and End go to the ends. The row to go to, if it moved.
fn step_keys(
    ui: &Ui,
    id: egui::Id,
    current: Option<usize>,
    len: usize,
    page: usize,
) -> Option<usize> {
    if len == 0 || !ui.memory(|m| m.has_focus(id)) {
        return None;
    }
    let filter = egui::EventFilter { vertical_arrows: true, ..Default::default() };
    ui.memory_mut(|m| m.set_focus_lock_filter(id, filter));
    use egui::Key;
    let pressed = |k| ui.input(|i| i.key_pressed(k));
    let last = len - 1;
    let page = page.max(1);
    let next = match current {
        _ if pressed(Key::Home) => 0,
        _ if pressed(Key::End) => last,
        None if [Key::ArrowDown, Key::ArrowUp, Key::PageDown, Key::PageUp]
            .into_iter()
            .any(pressed) =>
        {
            0
        }
        Some(c) if pressed(Key::ArrowDown) => (c + 1).min(last),
        Some(c) if pressed(Key::ArrowUp) => c.saturating_sub(1),
        Some(c) if pressed(Key::PageDown) => (c + page).min(last),
        Some(c) if pressed(Key::PageUp) => c.saturating_sub(page),
        _ => return None,
    };
    (Some(next) != current).then_some(next)
}

/// The list as a widget that can hold the keyboard (focus only, so the rows
/// still take the clicks; a focused widget needs an accessibility node).
fn list_widget(ui: &mut Ui, id: egui::Id, rect: egui::Rect, label: &str) {
    let r = ui.interact(rect, id, egui::Sense::focusable_noninteractive());
    r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Unknown, true, label));
}

/// The scroll offset that brings row `index` into a view at `offset` of
/// `height` (rows `pitch` apart), if it is out of it.
fn keep_visible(view: (f32, f32), index: usize, pitch: f32) -> Option<f32> {
    let (offset, height) = view;
    let (top, bottom) = (index as f32 * pitch, (index + 1) as f32 * pitch);
    if top < offset {
        Some(top)
    } else if bottom > offset + height {
        Some((bottom - height).max(0.0))
    } else {
        None
    }
}

/// The resource browser's state.
#[derive(Debug, Default)]
pub struct Browser {
    pub query: String,
    pub filter: Filter,
    /// The listing and what it was made for (generation, query, filter).
    listing: Option<(Listed, Items)>,
    /// The resource chosen (the list's, the gallery's).
    pub selected: Option<ResKey>,
    /// The creature chosen in the gallery (an `appearance.2da` row).
    pub chosen_row: Option<usize>,
    /// The list's scroll offset and height last frame.
    view: (f32, f32),
    /// A row was clicked: the list takes the keyboard.
    take_keys: bool,
    /// Where each gallery's grid stood when last drawn.
    pub drawn: HashMap<crate::gallery::Place, crate::gallery::Drawn>,
    /// The Gallery tab's header controls as last laid out (each named).
    pub tab_header: Vec<(&'static str, egui::Rect)>,
    /// A test's long list, listed whatever the library does.
    #[cfg(test)]
    injected: Option<Items>,
}

impl Browser {
    /// Lists again on the next frame.
    pub fn invalidate(&mut self) {
        self.listing = None;
    }

    /// Lists `items` as though the library had, from now on (a test's
    /// long list; opening one bumps the library's generation).
    #[cfg(test)]
    pub(crate) fn inject(&mut self, items: Items) {
        self.injected = Some(items);
    }
}

/// The search field (Find in names).
pub(crate) fn search_field(app: &mut Viewer, ui: &mut Ui, width: f32) {
    let edit = egui::TextEdit::singleline(&mut app.browser.query).hint_text("Search names");
    ui.add(edit.desired_width(width));
}

/// The type filter.
pub(crate) fn filter_combo(app: &mut Viewer, ui: &mut Ui, salt: &str, width: f32) {
    let b = &mut app.browser;
    let combo = egui::ComboBox::from_id_salt(salt).width(width);
    combo.selected_text(b.filter.name()).show_ui(ui, |ui| {
        for f in Filter::ALL {
            ui.selectable_value(&mut b.filter, f, f.name());
        }
    });
}

/// What the browser lists as its filter and query stand: the resources
/// (listed again when the library, the query or the filter changed), or
/// the creatures (`appearance.2da`'s rows with a race; none without the
/// table).
pub(crate) fn current(app: &mut Viewer) -> Cells {
    let b = &mut app.browser;
    if b.filter == Filter::Creatures {
        return Cells::Creatures(Arc::new(creature_rows(app).unwrap_or_default()));
    }
    #[cfg(test)]
    if let Some(items) = &b.injected {
        return Cells::Resources { items: items.clone(), detail: b.filter != Filter::Models };
    }
    let want = (app.lib.generation(), b.query.to_ascii_lowercase(), b.filter);
    if b.listing.as_ref().is_none_or(|(k, _)| *k != want) {
        let rm = app.lib.resman();
        let layers = rm.layers();
        let items: Vec<(ResKey, String)> = rm
            .entries()
            .into_iter()
            .filter(|(k, _)| want.2.admits(k.restype))
            .filter(|(k, _)| {
                want.1.is_empty() || k.resref.to_lowercase().to_string().contains(&want.1)
            })
            .map(|(k, layer)| (k, layers[layer].label.clone()))
            .collect();
        b.listing = Some((want, Arc::new(items)));
    }
    let items = b.listing.as_ref().expect("made above").1.clone();
    Cells::Resources { items, detail: b.filter != Filter::Models }
}

/// The least the size slider is given beside the count; a narrower room
/// puts it on a row of its own.
const SLIDER_LEAST: f32 = 64.0;

/// The count, the List and Gallery buttons and (in Gallery mode) the
/// pictures' size slider: the board "Resources as a Gallery"'s row.
fn count_row(app: &mut Viewer, ui: &mut Ui, count: &str) {
    use crate::settings::ResourcesView;
    use mg_uikit::{Icon, IconButton};
    let gallery = app.settings.resources_view == ResourcesView::Gallery;
    let mut below = false;
    ui.horizontal(|ui| {
        ui.weak(count);
        let list = IconButton::new(Icon::List, "List").icon_only(true).selected(!gallery);
        if ui.add(list).on_hover_text("Show the resources by name").clicked() {
            app.settings.resources_view = ResourcesView::List;
        }
        let pictures = IconButton::new(Icon::Gallery, "Gallery").icon_only(true).selected(gallery);
        if ui.add(pictures).on_hover_text("Show the resources as pictures").clicked() {
            app.settings.resources_view = ResourcesView::Gallery;
        }
        if gallery {
            let room = ui.available_width() - ui.spacing().item_spacing.x;
            match room >= SLIDER_LEAST {
                true => crate::gallery::size_slider(app, ui, room),
                false => below = true,
            }
        }
    });
    if below {
        let room = ui.available_width();
        crate::gallery::size_slider(app, ui, room);
    }
}

pub(crate) fn browser(app: &mut Viewer, ui: &mut Ui) {
    ui.horizontal(|ui| {
        search_field(app, ui, 140.0);
        filter_combo(app, ui, "filter", ui.spacing().combo_width);
    });
    let cells = current(app);
    let gallery = app.settings.resources_view == crate::settings::ResourcesView::Gallery;
    match &cells {
        Cells::Creatures(rows) => {
            if app.lib.game().table("appearance").is_err() {
                ui.weak("No appearance.2da (no game folder?)");
                return;
            }
            count_row(app, ui, &format!("{} creatures", rows.len()));
            match gallery {
                true => crate::gallery::grid(app, ui, &cells, crate::gallery::Place::Panel),
                false => creatures(app, ui, rows),
            }
        }
        Cells::Resources { items, .. } => {
            count_row(app, ui, &format!("{} resources", items.len()));
            match gallery {
                true => crate::gallery::grid(app, ui, &cells, crate::gallery::Place::Panel),
                false => resources(app, ui, items),
            }
        }
    }
}

/// The resources as a list; a click opens one.
fn resources(app: &mut Viewer, ui: &mut Ui, items: &[(ResKey, String)]) {
    // What is shown, so a click on it does not open it again.
    let shown = app.doc.as_ref().and_then(|d| d.opened.key.filter(|_| d.creature.is_none()));
    let b = &mut app.browser;
    let row = ui.text_style_height(&egui::TextStyle::Body) + 2.0;
    let pitch = row + ui.spacing().item_spacing.y;
    let mut open = None;
    // After a click, the arrow keys step through the list. (The list takes
    // the keyboard the frame after: egui gives a click's frame to the row.)
    let list = ui.scope_id().with("resource-list");
    if std::mem::take(&mut b.take_keys) {
        ui.memory_mut(|m| m.request_focus(list));
    }
    let index = b.selected.and_then(|k| items.iter().position(|(x, _)| *x == k));
    let page = (b.view.1 / pitch) as usize;
    let stepped = step_keys(ui, list, index, items.len(), page.saturating_sub(1));
    let mut area = egui::ScrollArea::vertical().auto_shrink([false, false]);
    if let Some(offset) = stepped.and_then(|i| keep_visible(b.view, i, pitch)) {
        area = area.vertical_scroll_offset(offset);
    }
    if let Some(i) = stepped {
        b.selected = Some(items[i].0);
        if shown != Some(items[i].0) {
            open = Some(items[i].0);
        }
    }
    let out = area.show_rows(ui, row, items.len(), |ui, range| {
        for (key, layer) in &items[range] {
            let selected = b.selected == Some(*key);
            let r = ui
                .selectable_label(selected, key.to_string())
                .on_hover_text(format!("from {layer}\nclick to open; then ↑ ↓ step"));
            if r.clicked() {
                b.selected = Some(*key);
                b.take_keys = true;
                if shown != Some(*key) {
                    open = Some(*key);
                }
            }
        }
    });
    b.view = (out.state.clamped_offset().y, out.inner_rect.height());
    list_widget(ui, list, out.inner_rect, "Resources");
    if let Some(k) = open {
        app.actions.push(Action::OpenResource(k));
    }
}

/// `appearance.2da`'s rows with a label and a race, as the query finds
/// them (by the row's number and label): each row and its label.
fn creature_rows(app: &Viewer) -> Option<Vec<(usize, String)>> {
    let q = app.browser.query.to_ascii_lowercase();
    let t = app.lib.game().table("appearance").ok()?;
    let rows = (0..t.len())
        .filter_map(|r| {
            let label = t.get(r, "LABEL").filter(|l| *l != "****")?;
            t.get(r, "RACE").filter(|v| *v != "****")?;
            let text = format!("{r} {label}");
            (q.is_empty() || text.to_ascii_lowercase().contains(&q)).then(|| (r, label.to_string()))
        })
        .collect();
    Some(rows)
}

/// `appearance.2da` rows; a click shows the creature.
fn creatures(app: &mut Viewer, ui: &mut Ui, rows: &[(usize, String)]) {
    // The appearance shown, so a click on it does not show it again.
    let shown = app.doc.as_ref().and_then(|d| d.creature).map(|c| usize::from(c.appearance));
    let row = ui.text_style_height(&egui::TextStyle::Body) + 2.0;
    let pitch = row + ui.spacing().item_spacing.y;
    let mut open = None;
    let list = ui.scope_id().with("creature-list");
    if std::mem::take(&mut app.browser.take_keys) {
        ui.memory_mut(|m| m.request_focus(list));
    }
    let view = app.browser.view;
    let index = shown.and_then(|s| rows.iter().position(|(r, _)| *r == s));
    let page = ((view.1 / pitch) as usize).saturating_sub(1);
    let stepped = step_keys(ui, list, index, rows.len(), page);
    let mut take_keys = false;
    let mut area = egui::ScrollArea::vertical().auto_shrink([false, false]);
    if let Some(i) = stepped {
        open = Some(rows[i].0);
        if let Some(offset) = keep_visible(view, i, pitch) {
            area = area.vertical_scroll_offset(offset);
        }
    }
    let out = area.show_rows(ui, row, rows.len(), |ui, range| {
        for (r, label) in &rows[range] {
            let selected = shown == Some(*r);
            let click = ui
                .selectable_label(selected, format!("{r} {label}"))
                .on_hover_text("click to show; then ↑ ↓ step");
            if click.clicked() {
                take_keys = true;
                if !selected {
                    open = Some(*r);
                }
            }
        }
    });
    app.browser.view = (out.state.clamped_offset().y, out.inner_rect.height());
    app.browser.take_keys |= take_keys;
    list_widget(ui, list, out.inner_rect, "Creatures");
    if let Some(r) = open {
        app.actions.push(Action::OpenCreature(mgv_stage::subject::CreatureLook::new(r as u16)));
    }
}

/// The node tree of everything on the stage.
pub(crate) fn outliner(app: &mut Viewer, ui: &mut Ui) {
    let Some(g) = &app.gfx else { return };
    if g.stage.is_empty() {
        ui.weak("Nothing shown.");
        return;
    }
    let mut picked = None;
    egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
        for (i, a) in g.stage.actors().iter().enumerate() {
            let actor = ActorId(i);
            let model = &a.model.model;
            let label = if i == 0 { a.name.clone() } else { format!("+ {}", a.name) };
            egui::CollapsingHeader::new(label).id_salt(("actor", i)).default_open(i == 0).show(
                ui,
                |ui| {
                    // Roots of the model's tree, then children under them.
                    for (n, node) in
                        model.nodes.iter().enumerate().filter(|(_, n)| n.parent.is_none())
                    {
                        let _ = node;
                        tree(ui, model, actor, n, app.selection, &mut picked);
                    }
                },
            );
        }
    });
    if let Some(sel) = picked {
        app.select(Some(sel), false);
    }
}

fn tree(
    ui: &mut Ui,
    model: &mg_mdl::Model,
    actor: ActorId,
    n: usize,
    selection: Option<Selection>,
    picked: &mut Option<Selection>,
) {
    let node = &model.nodes[n];
    let sel = Selection { actor, node: n };
    let text = format!("{} ({})", node.name, node.kind.type_name());
    let selected = selection == Some(sel);
    if node.children.is_empty() {
        if ui.selectable_label(selected, text).clicked() {
            *picked = Some(sel);
        }
        return;
    }
    let id = ui.make_persistent_id(("node", actor.0, n));
    egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, n == 0)
        .show_header(ui, |ui| {
            if ui.selectable_label(selected, text).clicked() {
                *picked = Some(sel);
            }
        })
        .body(|ui| {
            for &c in &node.children {
                tree(ui, model, actor, c, selection, picked);
            }
        });
}

/// The Inspector's sections, in their order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Section {
    /// What is open: its name, kind, file, origin, format.
    Doc,
    /// A creature's looks (the board's two-column grid).
    Creature,
    /// A model's PLT colors where it is shown on its own.
    Plt,
    /// An MTR's slots, parameters and flags.
    Material,
    /// A tileset's tiles.
    Tileset,
    /// The model shown.
    Model,
    /// The node selected.
    Node,
}

/// Where the tests find the width the Inspector's sections get.
#[cfg(test)]
pub(crate) const INSPECTOR_ROOM: &str = "inspector room";

/// What the open thing is, and the selected node: its sections flowing
/// into columns where the Inspector is wide (the kit's `FormColumns`: a
/// column of 440 at least; the Inspector dragged to the Bottom Dock),
/// stacked where it is not.
pub(crate) fn inspector(app: &mut Viewer, ui: &mut Ui) {
    let palettes = app.palettes.layers(&app.lib);
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        // (The room the sections get, for the tests of the pair grid.)
        #[cfg(test)]
        ui.ctx()
            .data_mut(|d| d.insert_temp(egui::Id::unique(INSPECTOR_ROOM), ui.available_width()));
        if app.doc.is_none() {
            ui.weak("Nothing open.");
            return;
        }
        let sections = sections(app);
        let columns = mg_uikit::form::FormColumns::new("inspector-sections").keys(&sections);
        columns.show(ui, sections.len(), |ui, i| match sections[i] {
            Section::Doc => doc_section(app, ui),
            Section::Creature => creature_section(app, ui, &palettes),
            Section::Plt => plt_section(app, ui, &palettes),
            Section::Material => {
                if let Some(doc) = &app.doc {
                    material(ui, &doc.opened.data);
                }
            }
            Section::Tileset => {
                let codepage = app.lib.game().language.codepage();
                let picked = app.doc.as_ref().and_then(|d| tileset(ui, codepage, &d.opened.data));
                if let Some(k) = picked {
                    app.actions.push(Action::OpenResource(k));
                }
            }
            Section::Model => model_section(app, ui),
            Section::Node => node_section(app, ui),
        });
    });
}

/// The sections the open thing has.
fn sections(app: &Viewer) -> Vec<Section> {
    let Some(doc) = &app.doc else { return Vec::new() };
    let mut s = vec![Section::Doc];
    if doc.creature.is_some() {
        s.push(Section::Creature);
    }
    if plt_actor(app).is_some() {
        s.push(Section::Plt);
    }
    match doc.opened.kind {
        Kind::Material => s.push(Section::Material),
        Kind::Tileset => s.push(Section::Tileset),
        _ => {}
    }
    let shown = app.gfx.as_ref().zip(doc.shown.as_ref());
    if shown.and_then(|(g, sh)| g.stage.actor(sh.base)).is_some() {
        s.push(Section::Model);
        if app.selection.is_some() {
            s.push(Section::Node);
        }
    }
    s
}

/// The actor of a model opened on its own whose textures are PLTs (their
/// colors are the Inspector's to set, as a creature or an item would).
fn plt_actor(app: &Viewer) -> Option<&mgv_stage::Actor> {
    let doc = app.doc.as_ref().filter(|d| d.creature.is_none() && d.opened.kind == Kind::Model)?;
    let a = app.gfx.as_ref()?.stage.actor(doc.shown.as_ref()?.base)?;
    let textures = a.look.textures.as_deref();
    mgv_stage::textures::uses_plt(app.lib.resman(), &a.model.model, textures).then_some(a)
}

/// The open thing's name, kind, file, origin and format.
fn doc_section(app: &Viewer, ui: &mut Ui) {
    let Some(doc) = &app.doc else { return };
    egui::Grid::new("doc").num_columns(2).striped(true).show(ui, |ui| {
        field_label(ui, "Name");
        ui.label(doc.name());
        ui.end_row();
        field_label(ui, "Kind");
        ui.label(format!("{:?}", doc.opened.kind));
        ui.end_row();
        if let Some(p) = &doc.opened.path {
            field_label(ui, "File");
            ui.label(p.display().to_string());
            ui.end_row();
        }
        if let Some(k) = doc.opened.key
            && let Some(o) = app.lib.origin(&k)
        {
            field_label(ui, "From");
            ui.label(o);
            ui.end_row();
        }
        if doc.opened.kind == Kind::Model {
            field_label(ui, "Format");
            ui.label(if mg_mdl::is_binary(&doc.opened.data) { "compiled" } else { "ASCII" });
            ui.end_row();
        }
    });
}

/// A pair's label's width in the Inspector's two-column grid (the board's
/// `.pair`: `68px auto 68px auto`, 8 apart).
pub(crate) const PAIR_LABEL: f32 = 68.0;
/// A pair's value's width: a number field (`.nfield`), or a swatch (28)
/// and its combo (52), 4 apart (`.pick`).
pub(crate) const PAIR_VALUE: f32 = 84.0;
/// The room between a grid's columns and its rows.
const PAIR_GAP: f32 = 8.0;

/// How many label and value pairs stand on a row of the grid in a room
/// `width` wide: two where both fit (328, in the Right Dock at its 352,
/// `shell::RIGHT_DOCK_WIDTH`), one in the tabbed Right Dock (280) or a
/// dock dragged narrower.
pub(crate) fn pairs_per_row(width: f32) -> usize {
    if width >= 2.0 * (PAIR_LABEL + PAIR_VALUE) + 3.0 * PAIR_GAP { 2 } else { 1 }
}

/// A field of the pair grid: its label and its value's control.
type Field<'a> = (String, Box<dyn FnOnce(&mut Ui) + 'a>);

/// The board's `.pair` grid: each field's label (cut to [`PAIR_LABEL`])
/// and value, [`pairs_per_row`] pairs a row, in their order across the
/// rows; `alone` (Gender) on a row of its own first.
fn pairs<'a>(ui: &mut Ui, id: &str, alone: Option<Field<'a>>, fields: Vec<Field<'a>>) {
    let per_row = pairs_per_row(ui.available_width());
    let label = |ui: &mut Ui, text: &str| {
        let h = ui.spacing().interact_size.y;
        let layout = egui::Layout::left_to_right(egui::Align::Center);
        ui.allocate_ui_with_layout(egui::vec2(PAIR_LABEL, h), layout, |ui| {
            ui.set_width(PAIR_LABEL);
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
            field_label(ui, text);
        });
    };
    // `alone` spans the row (the board's Gender), so it stands before the
    // grid and leaves its columns their own widths.
    if let Some((text, add)) = alone {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = PAIR_GAP;
            label(ui, &text);
            add(ui);
        });
        ui.add_space(6.0 - ui.spacing().item_spacing.y);
    }
    egui::Grid::new(id)
        .num_columns(2 * per_row)
        .spacing([PAIR_GAP, 6.0])
        .min_col_width(PAIR_LABEL)
        .show(ui, |ui| {
            for (i, (text, add)) in fields.into_iter().enumerate() {
                label(ui, &text);
                add(ui);
                if (i + 1) % per_row == 0 {
                    ui.end_row();
                }
            }
        });
}

/// A number field of the pair grid ([`PAIR_VALUE`] wide), `range` its
/// values.
fn number<'a, N: egui::emath::Numeric>(
    v: &'a mut N,
    range: std::ops::RangeInclusive<N>,
) -> Box<dyn FnOnce(&mut Ui) + 'a> {
    Box::new(move |ui: &mut Ui| {
        let h = ui.spacing().interact_size.y;
        ui.add_sized([PAIR_VALUE, h], egui::DragValue::new(v).range(range));
    })
}

/// A creature's looks (the board "Creature, Colors": the Inspector's
/// Creature section, in the two-column grid): Gender on its row, then
/// Phenotype, the skin's, hair's and tattoos' colors, Head, Body parts,
/// Wings and Tail. A change shows the creature anew.
fn creature_section(app: &mut Viewer, ui: &mut Ui, palettes: &[Option<Arc<mg_image::Rgba>>; 10]) {
    let Some(mut look) = app.doc.as_ref().and_then(|d| d.creature) else { return };
    section_heading(ui, "Creature");
    if creature_form(ui, &mut look, palettes) {
        app.actions.push(Action::OpenCreature(look));
    }
}

/// [`creature_section`]'s grid; whether the look changed.
pub(crate) fn creature_form(
    ui: &mut Ui,
    look: &mut mgv_stage::subject::CreatureLook,
    palettes: &[Option<Arc<mg_image::Rgba>>; 10],
) -> bool {
    let before = *look;
    let l = look;
    let gender: Field<'_> = (
        "Gender".into(),
        Box::new(|ui: &mut Ui| {
            let choices = [(0u8, "male", ""), (1u8, "female", "")];
            mg_uikit::segmented(ui, "Gender", &mut l.gender, &choices);
        }),
    );
    let [skin, hair, tattoo1, tattoo2] = &mut l.colors;
    let mut fields: Vec<Field<'_>> = vec![("Phenotype".into(), number(&mut l.phenotype, 0..=255))];
    // The look's colors and their PLT layers.
    for (c, (label, layer)) in [skin, hair, tattoo1, tattoo2].into_iter().zip([
        ("Skin color", 0),
        ("Hair color", 1),
        ("Tattoo 1", 8),
        ("Tattoo 2", 9),
    ]) {
        let palette = palettes[layer].as_deref();
        fields.push((
            label.into(),
            Box::new(move |ui: &mut Ui| {
                crate::palette::pick(ui, palette, label, c);
            }),
        ));
    }
    fields.push(("Head".into(), number(&mut l.head, 1..=255)));
    fields.push(("Body parts".into(), number(&mut l.body, 1..=255)));
    fields.push(("Wings".into(), number(&mut l.wings, 0..=255)));
    fields.push(("Tail".into(), number(&mut l.tail, 0..=255)));
    pairs(ui, "look", Some(gender), fields);
    *l != before
}

/// A model on its own with PLT textures: their ten layers' colors (which
/// a creature or an item would give them) in the two-column grid, and
/// Reset Colors under them.
fn plt_section(app: &mut Viewer, ui: &mut Ui, palettes: &[Option<Arc<mg_image::Rgba>>; 10]) {
    let Some(a) = plt_actor(app) else { return };
    let before = a.look.colors;
    let mut colors = before.unwrap_or_default();
    section_heading(ui, "PLT colors");
    let fields: Vec<Field<'_>> = (mgv_stage::textures::PLT_LAYERS.iter().zip(&mut colors))
        .enumerate()
        .map(|(i, (label, c))| -> Field<'_> {
            let palette = palettes[i].as_deref();
            (
                (*label).into(),
                Box::new(move |ui: &mut Ui| {
                    crate::palette::pick(ui, palette, label, c);
                }),
            )
        })
        .collect();
    pairs(ui, "plt colors", None, fields);
    let mut after = (before.is_some() || colors != [0; 10]).then_some(colors);
    ui.add_space(PAIR_GAP);
    if ui.add_enabled(before.is_some(), egui::Button::new("Reset Colors")).clicked() {
        after = None;
    }
    if after != before {
        app.actions.push(Action::SetColors(after));
    }
}

/// The model shown: its name, classification, supermodel, nodes, faces
/// and vertices, animations.
fn model_section(app: &Viewer, ui: &mut Ui) {
    let Some(g) = &app.gfx else { return };
    let Some(base) = app.doc.as_ref().and_then(|d| d.shown.as_ref()).map(|s| s.base) else {
        return;
    };
    let Some(a) = g.stage.actor(base) else { return };
    let m = &a.model.model;
    section_heading(ui, "Model");
    egui::Grid::new("model").num_columns(2).striped(true).show(ui, |ui| {
        field_label(ui, "Model");
        ui.label(&m.name);
        ui.end_row();
        field_label(ui, "Classification");
        ui.label(format!("{:?}", m.classification));
        ui.end_row();
        field_label(ui, "Supermodel");
        ui.label(m.supermodel.as_deref().unwrap_or("none"));
        ui.end_row();
        field_label(ui, "Nodes");
        ui.label(m.nodes.len().to_string());
        ui.end_row();
        let (faces, verts) = m
            .nodes
            .iter()
            .filter_map(|n| n.mesh())
            .fold((0, 0), |(f, v), mesh| (f + mesh.faces.len(), v + mesh.vertices.len()));
        field_label(ui, "Faces / vertices");
        ui.label(format!("{faces} / {verts}"));
        ui.end_row();
        field_label(ui, "Animations");
        ui.label(format!("{} ({} own)", a.animations.0.len(), m.animations.len()));
        ui.end_row();
    });
}

/// The node selected.
fn node_section(app: &Viewer, ui: &mut Ui) {
    let Some(g) = &app.gfx else { return };
    let Some(sel) = app.selection else { return };
    let Some(sa) = g.stage.actor(sel.actor) else { return };
    let Some(node) = sa.model.model.nodes.get(sel.node) else { return };
    section_heading(ui, &node.name);
    egui::Grid::new("node").num_columns(2).striped(true).show(ui, |ui| {
        let row = |ui: &mut Ui, k: &str, v: String| {
            field_label(ui, k);
            ui.label(v);
            ui.end_row();
        };
        row(ui, "Type", node.kind.type_name().into());
        row(
            ui,
            "Parent",
            node.parent.map_or("none".into(), |p| sa.model.model.nodes[p].name.clone()),
        );
        row(ui, "Position", fmt3(node.position));
        let aa = mg_mdl::ascii_write::axis_angle(node.orientation);
        row(
            ui,
            "Orientation",
            format!("{} rotated {:.1}°", fmt3([aa[0], aa[1], aa[2]]), aa[3].to_degrees()),
        );
        if node.scale != 1.0 {
            row(ui, "Scale", format!("{}", node.scale));
        }
        for c in &node.controllers {
            let v: Vec<String> = c.row(0).iter().map(|x| format!("{x:.4}")).collect();
            row(ui, &c.name, v.join(" "));
        }
        match &node.kind {
            NodeKind::Mesh(mesh) => {
                row(
                    ui,
                    "Faces / vertices",
                    format!("{} / {}", mesh.faces.len(), mesh.vertices.len()),
                );
                for (i, t) in mesh.textures.iter().enumerate() {
                    if let Some(t) = t {
                        let found = mg_render::Assets::texture(app.lib.resman(), t).is_some();
                        let label =
                            if i == 0 { "Bitmap".to_string() } else { format!("Texture {i}") };
                        row(ui, &label, format!("{t}{}", if found { "" } else { " (not found)" }));
                    }
                }
                if let Some(mtr) = &mesh.material {
                    row(ui, "Material", mtr.clone());
                }
                row(ui, "Render / shadow", format!("{} / {}", mesh.render, mesh.shadow));
                if mesh.transparency_hint != 0 {
                    row(ui, "Transparency hint", mesh.transparency_hint.to_string());
                }
                match &mesh.extra {
                    MeshExtra::Skin(s) => row(ui, "Bones", s.bones.len().to_string()),
                    MeshExtra::Dangly(d) => row(
                        ui,
                        "Dangly",
                        format!(
                            "displacement {} tightness {} period {}",
                            d.displacement, d.tightness, d.period
                        ),
                    ),
                    MeshExtra::Anim(am) => row(ui, "Sample period", am.sample_period.to_string()),
                    MeshExtra::Aabb(t) => row(ui, "AABB entries", t.len().to_string()),
                    MeshExtra::None => {}
                }
            }
            NodeKind::Emitter(e) => {
                row(
                    ui,
                    "Update / render / blend",
                    format!("{} / {} / {}", e.update, e.render, e.blend),
                );
                row(ui, "Texture", e.texture.clone().unwrap_or_else(|| "none".into()));
                if let Some(c) = &e.chunk {
                    row(ui, "Chunk", c.clone());
                }
                row(ui, "Grid", format!("{} × {}", e.xgrid, e.ygrid));
                let flags: Vec<&str> = mg_mdl::EMITTER_FLAGS
                    .iter()
                    .filter(|(_, b)| e.flags & b != 0)
                    .map(|(n, _)| *n)
                    .collect();
                row(ui, "Flags", flags.join(", "));
            }
            NodeKind::Light(l) => {
                row(ui, "Priority", l.priority.to_string());
                row(ui, "Ambient only", l.ambient_only.to_string());
                row(ui, "Shadow", l.shadow.to_string());
            }
            NodeKind::Reference(r) => {
                row(ui, "Model", r.model.clone().unwrap_or_else(|| "none".into()));
            }
            NodeKind::Dummy | NodeKind::Camera => {}
        }
    });
}

/// An MTR's slots, parameters and flags.
fn material(ui: &mut Ui, data: &[u8]) {
    const SLOTS: [&str; 6] =
        ["diffuse", "normal", "specular", "roughness", "height", "self-illumination"];
    let m = mg_image::mtr::Mtr::parse(data);
    section_heading(ui, "Material");
    egui::Grid::new("mtr").num_columns(2).striped(true).show(ui, |ui| {
        for (i, t) in m.textures.iter().enumerate() {
            let Some(t) = t else { continue };
            field_label(
                ui,
                format!("Texture {i}{}", SLOTS.get(i).map_or(String::new(), |s| format!(" ({s})"))),
            );
            ui.label(t);
            ui.end_row();
        }
        field_label(ui, "Render hint");
        ui.label(format!("{:?}", m.renderhint));
        ui.end_row();
        for (name, p) in &m.params {
            field_label(ui, name);
            ui.label(match p {
                mg_image::mtr::Param::Float(v) => {
                    v.iter().map(|x| format!("{x}")).collect::<Vec<_>>().join(" ")
                }
                mg_image::mtr::Param::Int(i) => i.to_string(),
            });
            ui.end_row();
        }
        if let (Some(vs), Some(fs)) = (&m.shader_vs, &m.shader_fs) {
            field_label(ui, "Custom shaders");
            ui.label(format!("{vs} / {fs} (not run: the viewer draws with its own)"));
            ui.end_row();
        }
        for (flag, on) in [("Transparent", m.transparency), ("Two-sided", m.twosided)] {
            if on {
                field_label(ui, flag);
                ui.label("yes");
                ui.end_row();
            }
        }
    });
}

/// A tileset's tiles; the one clicked is returned (its model).
fn tileset(ui: &mut Ui, codepage: mg_core::Codepage, data: &[u8]) -> Option<ResKey> {
    let set = match mg_set::Tileset::parse(data, codepage) {
        Ok(s) => s,
        Err(e) => {
            ui.colored_label(ui.visuals().error_fg_color, e.to_string());
            return None;
        }
    };
    section_heading(ui, "Tileset");
    ui.label(format!(
        "{} tiles, {} groups, {} terrains, {} crossers",
        set.tiles.len(),
        set.groups.len(),
        set.terrains.len(),
        set.crossers.len()
    ));
    for w in set.warnings.iter().take(5) {
        ui.weak(w);
    }
    let mut picked = None;
    for (i, t) in set.tiles.iter().enumerate() {
        let corners: Vec<&str> = t.corners.iter().map(|(c, _)| c.as_str()).collect();
        let r = ui
            .selectable_label(false, format!("{i}: {}", t.model.to_ascii_lowercase()))
            .on_hover_text(format!(
                "corners {}\n{} doors{}\nclick to show",
                corners.join(", "),
                t.doors.len(),
                t.walkmesh.as_deref().map_or(String::new(), |w| format!(", walkmesh {w}"))
            ));
        if r.clicked() {
            picked = ResKey::parse(&t.model, ResType::MDL);
        }
    }
    picked
}

fn fmt3(v: [f32; 3]) -> String {
    format!("{:.4} {:.4} {:.4}", v[0], v[1], v[2])
}

/// Animations shorter than this (a frame or two at 30 fps, as placeables'
/// `default` and `dead`) are shown as a pose, not on a slider.
const SINGLE_POSE: f32 = 0.1;
/// The animation combo's width (the board's `.anim-combo`).
const ANIM_COMBO: f32 = 150.0;
/// The speed slider's width (`.speed`).
const SPEED_SLIDER: f32 = 180.0;
/// A number field's width beside a slider (`.speed-num`).
const NUMBER_FIELD: f32 = 56.0;
/// The ruler's height under the scrub bar (its ticks and their seconds).
const RULER: f32 = 16.0;
/// The least room between two of the ruler's ticks (the board's 0.5 s
/// steps on a bar some 880 wide through 2.67 s).
const RULER_LEAST: f32 = 120.0;

/// The Timeline (the board's Bottom Dock, `.transport` over `.scrub`):
/// one transport row (the animation, Pause or Play joined to Restart,
/// Loop or Once, and at its right end the speed's slider and number),
/// then the scrub bar the width of the dock with the time's number and
/// the length after it, and the seconds ruler under it. An animation of a
/// frame or two is a pose, said so in place of the bar.
pub(crate) fn timeline(app: &mut Viewer, ui: &mut Ui) {
    use mg_uikit::{Icon, IconButton, Join};
    let Some(g) = &mut app.gfx else { return };
    let Some(base) = app.doc.as_ref().and_then(|d| d.shown.as_ref()).map(|s| s.base) else {
        ui.weak("Nothing shown.");
        return;
    };
    let Some(a) = g.stage.actor_mut(base) else { return };
    let names: Vec<String> = a.animations.names().map(str::to_string).collect();
    let current = a.player.animation.clone();
    let found = current.as_deref().and_then(|n| a.animations.find(n));
    let length = found.map_or(0.0, |x| x.length);
    let events: Vec<(f32, String)> = found.map(|x| x.events.clone()).unwrap_or_default();
    ui.horizontal(|ui| {
        let combo = egui::ComboBox::from_id_salt("anim")
            .selected_text(current.clone().unwrap_or_else(|| "(rest pose)".into()))
            .width(ANIM_COMBO)
            .height(400.0)
            .show_ui(ui, |ui| {
                if ui.selectable_label(current.is_none(), "(rest pose)").clicked() {
                    a.player.play(None, PlayMode::Loop);
                }
                for n in &names {
                    if ui.selectable_label(current.as_deref() == Some(n), n).clicked() {
                        let mode = a.player.mode;
                        a.player.play(Some(n), mode);
                    }
                }
            });
        combo
            .response
            .widget_info(|| egui::WidgetInfo::labeled(egui::Role::ComboBox, true, "Animation"));
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.x = mg_uikit::joined_spacing(ui);
            let (icon, words) = match a.player.playing {
                true => (Icon::Pause, "Pause"),
                false => (Icon::Play, "Play"),
            };
            if ui.add(IconButton::new(icon, words).joined(Join::Next)).clicked() {
                a.player.playing = !a.player.playing;
            }
            let restart = IconButton::new(Icon::Restart, "Restart").joined(Join::Previous);
            if ui.add(restart).on_hover_text("From the start, emitters too").clicked() {
                a.player.time = 0.0;
                a.restart_particles();
            }
        });
        let modes = [(PlayMode::Loop, "Loop", ""), (PlayMode::Once, "Once", "")];
        mg_uikit::segmented(ui, "Play mode", &mut a.player.mode, &modes);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let number = egui::DragValue::new(&mut a.player.speed)
                .range(0.05..=4.0)
                .speed(0.01)
                .fixed_decimals(2);
            ui.add_sized([NUMBER_FIELD, ui.spacing().interact_size.y], number)
                .on_hover_text("How fast it plays: 1 is the game's speed");
            // (Room for the slider where the row has it; else the number
            // alone.)
            if ui.available_width() > SPEED_SLIDER + 80.0 {
                ui.spacing_mut().slider_width = SPEED_SLIDER;
                let slider = egui::Slider::new(&mut a.player.speed, 0.05..=4.0)
                    .logarithmic(true)
                    .show_value(false);
                mg_uikit::slider(ui, slider)
                    .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Slider, true, "Speed"));
            }
            ui.label("Speed");
        });
    });
    if length > 0.0 && length < SINGLE_POSE {
        // A loop of a frame or two would throw the slider from end to end
        // every frame.
        ui.weak(format!("A single pose ({length:.2} s)"));
    } else if length > 0.0 {
        let t = match a.player.mode {
            PlayMode::Loop => a.player.time.rem_euclid(length),
            PlayMode::Once => a.player.time.min(length),
        };
        if let Some(t) = scrub(ui, t, length, &events) {
            a.player.time = t;
        }
        if !events.is_empty() {
            let list: Vec<String> = events.iter().map(|(t, n)| format!("{n} {t:.2}s")).collect();
            ui.weak(format!("Events: {}", list.join(", ")));
        }
    }
}

/// The scrub bar (the board's `.scrub`): a slider through the animation
/// `length` seconds long the width of the room, at `time`, its events
/// marked on it in `warn`; after it the time as a number and "of 2.67 s";
/// under it the seconds ruler, a tick at each step of the length (a
/// step that leaves room for its words: 0.5 s for an animation of a few
/// seconds). The time chosen, where the bar or the number was moved.
/// (The bar's events: the moves' "Not Drawn" leaves a list of animations
/// beside the transport out, the combo is the Timeline's only one.)
pub(crate) fn scrub(ui: &mut Ui, time: f32, length: f32, events: &[(f32, String)]) -> Option<f32> {
    let t = ui.tokens();
    let mut time = time;
    let mut changed = false;
    let of = format!("of {length:.2} s");
    let font = egui::FontId::proportional(mg_uikit::metrics::BODY_SIZE);
    let of_width = ui.painter().layout_no_wrap(of.clone(), font, t.text_3).size().x;
    let gap = ui.spacing().item_spacing.x;
    let after = NUMBER_FIELD + of_width + 3.0 * gap;
    let rail = ui
        .horizontal(|ui| {
            ui.spacing_mut().slider_width = (ui.available_width() - after).max(40.0);
            let slider = egui::Slider::new(&mut time, 0.0..=length).show_value(false);
            let r = mg_uikit::slider(ui, slider);
            r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Slider, true, "Time"));
            changed |= r.changed();
            let number =
                egui::DragValue::new(&mut time).range(0.0..=length).speed(0.01).fixed_decimals(2);
            let n = ui.add_sized([NUMBER_FIELD, ui.spacing().interact_size.y], number);
            changed |= n.changed();
            ui.label(egui::RichText::new(&of).color(t.text_3));
            // (Where the slider's values lie along it: inside its thumb's
            // half at each end, as egui places them.)
            let rect = r.rect;
            let radius = rect.height() / 2.5;
            let radius = match ui.visuals().handle_shape {
                egui::style::HandleShape::Circle => radius,
                egui::style::HandleShape::Rect { aspect_ratio } => radius * aspect_ratio,
            };
            (rect, rect.x_range().shrink(radius))
        })
        .inner;
    let (bar, along) = rail;
    let x = |s: f32| along.min + along.span() * (s / length).clamp(0.0, 1.0);
    let painter = ui.painter();
    for (at, _) in events {
        let x = x(*at);
        let stroke = egui::Stroke::new(2.0, t.warn);
        painter.vline(x, egui::Rangef::new(bar.bottom() - 4.0, bar.bottom() + 2.0), stroke);
    }
    // The ruler.
    let (ruler, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), RULER), egui::Sense::hover());
    let step = ruler_step(length, along.span());
    let small = egui::FontId::proportional(11.0);
    let line = egui::Stroke::new(1.0, t.line_soft);
    let painter = ui.painter_at(ruler);
    let mut i = 0;
    loop {
        let s = step * i as f32;
        if s > length + step * 1e-3 {
            break;
        }
        let x = x(s).round() + 0.5;
        painter.vline(x, egui::Rangef::new(ruler.top() + 2.0, ruler.bottom()), line);
        let words = ruler_words(s, step);
        painter.text(
            egui::pos2(x + 3.0, ruler.top() + 3.0),
            egui::Align2::LEFT_TOP,
            words,
            small.clone(),
            t.text_3,
        );
        i += 1;
    }
    changed.then_some(time.clamp(0.0, length))
}

/// The ruler's step for an animation `length` seconds long on a bar
/// `width` points long: the least of the usual steps whose ticks stand
/// [`RULER_LEAST`] apart.
pub(crate) fn ruler_step(length: f32, width: f32) -> f32 {
    const STEPS: [f32; 12] = [0.05, 0.1, 0.2, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0, 15.0, 30.0, 60.0];
    let per_second = width / length.max(1e-3);
    STEPS.into_iter().find(|s| s * per_second >= RULER_LEAST).unwrap_or(60.0)
}

/// A tick's words: its seconds, with as many decimals as the step has
/// ("0", "0.5", "1.0", "1.25").
fn ruler_words(s: f32, step: f32) -> String {
    if s == 0.0 {
        return "0".into();
    }
    let decimals = if step >= 1.0 {
        0
    } else if (step * 10.0).fract().abs() < 1e-4 {
        1
    } else {
        2
    };
    format!("{s:.decimals$}")
}

pub(crate) fn log(app: &mut Viewer, ui: &mut Ui) {
    egui::ScrollArea::vertical().auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
        for l in &app.log {
            let color = match l.level {
                Level::Info => ui.visuals().text_color(),
                Level::Warning => ui.visuals().warn_fg_color,
                Level::Error => ui.visuals().error_fg_color,
            };
            ui.colored_label(color, &l.text);
        }
    });
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// The rectangle of the widget named `label` in a frame's accesskit
    /// output.
    pub(crate) fn named_rect(out: &egui::FullOutput, label: &str) -> Option<egui::Rect> {
        let update = out.platform_output.accesskit_update.as_ref()?;
        update.nodes.iter().find(|(_, n)| n.label() == Some(label)).and_then(|(_, n)| {
            let b = n.bounds()?;
            Some(egui::Rect::from_min_max(
                egui::pos2(b.x0 as f32, b.y0 as f32),
                egui::pos2(b.x1 as f32, b.y1 as f32),
            ))
        })
    }

    /// A frame of `add` in a window 800 by 200 given `events`.
    fn frame(
        ctx: &egui::Context,
        events: Vec<egui::Event>,
        add: &mut dyn FnMut(&mut Ui),
    ) -> egui::FullOutput {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 200.0),
            )),
            events,
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            mg_uikit::apply(ui.ctx(), mg_uikit::Theme::Dark);
            egui::CentralPanel::default().show(ui, |ui| add(ui));
        });
        out.textures_delta.clear();
        out
    }

    /// The Creature grid: Gender alone on its row, then two pairs a row
    /// where they fit (Skin color beside Phenotype, Hair color under it),
    /// one a row in a narrower dock; a picked color changes the look.
    #[test]
    fn the_creature_grid_stands_two_pairs_a_row_where_they_fit() {
        let palette = Arc::new(crate::palette::tests::palette());
        let palettes: [Option<Arc<mg_image::Rgba>>; 10] =
            std::array::from_fn(|_| Some(palette.clone()));
        for (width, per_row) in [(328.0, 2), (268.0, 1)] {
            let ctx = egui::Context::default();
            ctx.enable_accesskit();
            let mut look = mgv_stage::subject::CreatureLook::new(6);
            let mut changed = false;
            let mut draw = |ui: &mut Ui| {
                ui.allocate_ui(egui::vec2(width, 180.0), |ui| {
                    ui.set_max_width(width);
                    changed |= creature_form(ui, &mut look, &palettes);
                });
            };
            frame(&ctx, Vec::new(), &mut draw);
            let out = frame(&ctx, Vec::new(), &mut draw);
            let at = |name: &str| named_rect(&out, name).unwrap_or_else(|| panic!("{name}"));
            let (gender, skin, hair) =
                (at("Gender"), at("Skin color swatch"), at("Hair color swatch"));
            assert!(skin.top() > gender.bottom(), "Gender alone first");
            assert!(hair.top() > skin.bottom(), "Hair under Skin: {skin:?} {hair:?}");
            if per_row == 2 {
                // Skin color in the second pair, beside Phenotype.
                assert!(skin.left() > hair.left() + PAIR_LABEL + PAIR_VALUE, "{skin:?} {hair:?}");
            } else {
                assert!((hair.left() - skin.left()).abs() < 1.0, "{skin:?} {hair:?}");
            }
            assert!(!changed);
        }
    }

    /// A click on the scrub bar seeks to the time under it, the bar the
    /// width of the room; the number after it says it, and moves it too.
    #[test]
    fn the_scrub_bar_seeks() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let length = 2.0;
        let mut time = 0.5;
        let mut draw = |ui: &mut Ui| {
            if let Some(t) = scrub(ui, time, length, &[(1.0, "hit".into())]) {
                time = t;
            }
        };
        frame(&ctx, Vec::new(), &mut draw);
        let out = frame(&ctx, Vec::new(), &mut draw);
        let bar = named_rect(&out, "Time").expect("the scrub bar");
        assert!(bar.width() > 600.0, "the width of the room: {bar:?}");
        let at = egui::pos2(bar.left() + bar.width() * 0.75, bar.center().y);
        let button = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(&ctx, vec![egui::Event::PointerMoved(at)], &mut draw);
        frame(&ctx, vec![button(true)], &mut draw);
        frame(&ctx, vec![button(false)], &mut draw);
        drop(draw);
        // (Within the thumb's half at the ends, as egui places values.)
        assert!((time - 1.5).abs() < 0.05, "seeked to {time}");
    }

    /// The ruler's ticks stand at steps that leave room for their words:
    /// the board's 0.5 s on a bar some 880 wide through 2.67 s, finer on a
    /// longer bar, coarser for a long animation; their words as the step.
    #[test]
    fn the_ruler_steps_as_the_board_s() {
        assert_eq!(ruler_step(2.67, 880.0), 0.5);
        assert_eq!(ruler_step(2.67, 3000.0), 0.2);
        assert_eq!(ruler_step(30.0, 880.0), 5.0);
        assert_eq!(ruler_step(0.3, 400.0), 0.1);
        assert_eq!(ruler_words(0.0, 0.5), "0");
        assert_eq!(ruler_words(1.0, 0.5), "1.0");
        assert_eq!(ruler_words(1.25, 0.25), "1.25");
        assert_eq!(ruler_words(10.0, 5.0), "10");
    }
}
