//! The ASCII editor: a model's text with highlighting, line numbers,
//! diagnostics and the node outline. Edits reload the model in the view
//! after a short pause (the text goes into the library's in-memory layer,
//! so the file on disk is untouched until Save).
//!
//! The text area is [`CodeEditor`]: only the lines in sight are laid out,
//! so the game's largest models edit as quickly as the smallest.

use std::path::{Path, PathBuf};

use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontId, Ui};
use mg_mdl::keywords::{self, Class};
use mg_mdl::lint::{Diagnostic, Severity};
use mg_mdl::outline::Outline;
use mg_resman::ResKey;
use mg_uikit::header::{EditorHeader, Item};
use mg_uikit::shell::PanelHeader;
use mg_uikit::{Badge, BadgeKind, Icon, Tokens, TokensExt};

use crate::code::CodeEditor;
use crate::{Action, Viewer};

/// An ASCII model being edited.
#[derive(Debug)]
pub struct Buffer {
    /// The text and the editor's state.
    pub editor: CodeEditor,
    /// The file it was read from or saved to.
    pub path: Option<PathBuf>,
    /// The resource it stands for (its edits go into the library under it).
    pub key: Option<ResKey>,
    /// Where Save As suggests (a decompiled model's: beside its binary).
    pub suggested_path: Option<PathBuf>,
    /// The text as last read from or written to disk.
    saved: String,
    /// The editor's revision when last saved or loaded.
    saved_revision: u64,
    /// Not on disk at all yet (decompiled).
    new: bool,
    /// The file's line ends were CRLF.
    crlf: bool,
    pub outline: Outline,
    /// Where the reader put each node: its lines (the toolset's source
    /// map) and its name, by its index in the model read from the text.
    pub source: Option<(mg_mdl::ascii::SourceMap, Vec<String>)>,
    pub diagnostics: Vec<Diagnostic>,
    /// When the text last changed (window clock), until it is applied.
    edited_at: Option<f64>,
    /// Seconds to wait after the last change before reloading.
    pub delay: f64,
    /// The file changed on disk while there were unsaved edits.
    pub disk_changed: bool,
    /// The cursor's line (0-based), as of the last frame.
    pub cursor_line: Option<usize>,
}

impl Buffer {
    /// A buffer of `text` (CRLF line ends shown as plain ones and restored
    /// on save). `new`: not saved anywhere yet.
    pub fn new(text: String, path: Option<PathBuf>, key: Option<ResKey>, new: bool) -> Buffer {
        let crlf = text.contains("\r\n");
        let text = if crlf { text.replace("\r\n", "\n") } else { text };
        let editor = CodeEditor::new(&text);
        let mut b = Buffer {
            saved_revision: editor.revision,
            editor,
            saved: text,
            path,
            key,
            suggested_path: None,
            new,
            crlf,
            outline: Outline::default(),
            source: None,
            diagnostics: Vec::new(),
            edited_at: None,
            delay: 0.15,
            disk_changed: false,
            cursor_line: None,
        };
        b.refresh_analysis();
        b
    }

    /// The text (lines joined with `\n`).
    pub fn text(&self) -> String {
        self.editor.text()
    }

    /// Replaces the text (as an edit: the view reloads after the pause).
    pub fn set_text(&mut self, text: &str, now: f64) {
        self.editor.set_text(text);
        self.edited(now);
    }

    pub fn is_dirty(&self) -> bool {
        self.new || self.editor.revision != self.saved_revision
    }

    /// Recomputes the outline, the source map and the diagnostics.
    pub fn refresh_analysis(&mut self) {
        let text = self.text();
        self.outline = mg_mdl::outline::outline(&text);
        self.source = mg_mdl::ascii::read_mapped(text.as_bytes())
            .ok()
            .map(|(m, map)| (map, m.nodes.into_iter().map(|n| n.name).collect()));
        self.diagnostics = mg_mdl::lint::check(&text);
    }

    /// The model node whose block holds a line (0-based), and its name.
    pub fn node_at(&self, line: usize) -> Option<(usize, &str)> {
        let (map, names) = self.source.as_ref()?;
        let i = map.node_at(line + 1)?;
        Some((i, names.get(i)?.as_str()))
    }

    /// The first line (0-based) of the block of model node `index`, if it
    /// is named `name` (the text read as the model on the stage was).
    pub fn node_line(&self, index: usize, name: &str) -> Option<usize> {
        let (map, names) = self.source.as_ref()?;
        let span = map.nodes.get(index).filter(|s| s.start > 0)?;
        names.get(index).filter(|n| n.eq_ignore_ascii_case(name))?;
        Some(span.start - 1)
    }

    /// Notes an edit at `now`.
    pub fn edited(&mut self, now: f64) {
        self.edited_at = Some(now);
    }

    /// Whether the pause after the last edit is over (once per edit run).
    pub fn reload_due(&mut self, now: f64) -> bool {
        match self.edited_at {
            Some(t) if now - t >= self.delay => {
                self.edited_at = None;
                true
            }
            _ => false,
        }
    }

    /// Whether an edit is waiting to be applied.
    pub fn pending(&self) -> bool {
        self.edited_at.is_some()
    }

    /// Whether `text` is what was last read or written.
    pub fn matches_disk(&self, text: &str) -> bool {
        text.replace("\r\n", "\n") == self.saved
    }

    /// Takes the file's new text (no unsaved edits here).
    pub fn replace_from_disk(&mut self, text: String) {
        self.crlf = text.contains("\r\n");
        let text = if self.crlf { text.replace("\r\n", "\n") } else { text };
        self.editor.set_text(&text);
        self.saved = text;
        self.saved_revision = self.editor.revision;
        self.new = false;
        self.disk_changed = false;
        self.refresh_analysis();
    }

    /// Writes the text (in its original line ends) and remembers the path.
    pub fn save(&mut self, path: &Path) -> std::io::Result<()> {
        let text = self.text();
        let out = if self.crlf { text.replace('\n', "\r\n") } else { text.clone() };
        // Through a temporary file, so a failed write leaves the old file.
        let tmp = path.with_extension("mdl.mgv-tmp");
        std::fs::write(&tmp, out.as_bytes())?;
        std::fs::rename(&tmp, path)?;
        self.saved = text;
        self.saved_revision = self.editor.revision;
        self.path = Some(path.to_path_buf());
        self.new = false;
        self.disk_changed = false;
        Ok(())
    }

    /// The worst diagnostic on each line, as gutter colours.
    fn marks(&self, t: &Tokens) -> std::collections::HashMap<usize, Color32> {
        let mut m: std::collections::HashMap<usize, Severity> = std::collections::HashMap::new();
        for d in &self.diagnostics {
            let e = m.entry(d.line).or_insert(d.severity);
            *e = (*e).max(d.severity);
        }
        m.into_iter().map(|(l, s)| (l, severity_color(s, t))).collect()
    }
}

/// Colours of the highlighting: the theme's `mdl-*` tokens.
struct Palette {
    plain: Color32,
    structure: Color32,
    keyword: Color32,
    keyed: Color32,
    node_type: Color32,
    number: Color32,
    comment: Color32,
}

impl Palette {
    fn new(t: &Tokens) -> Palette {
        Palette {
            plain: t.mdl_plain,
            structure: t.mdl_structure,
            keyword: t.mdl_keyword,
            keyed: t.mdl_keyed,
            node_type: t.mdl_node_type,
            number: t.mdl_number,
            comment: t.mdl_comment,
        }
    }
}

/// The text as coloured sections: the first word of a line by what it is
/// (a word after `node` is the node type), numbers, comments.
pub fn highlight(text: &str, font: FontId, tokens: &Tokens) -> LayoutJob {
    let p = Palette::new(tokens);
    let mut job = LayoutJob::default();
    let fmt = |c: Color32| TextFormat::simple(font.clone(), c);
    for line in text.split_inclusive('\n') {
        let (code, comment) = match line.find('#') {
            Some(i) => line.split_at(i),
            None => (line, ""),
        };
        let mut word_index = 0;
        let mut after_node = false;
        let mut rest = code;
        while !rest.is_empty() {
            let space = rest.len() - rest.trim_start().len();
            if space > 0 {
                job.append(&rest[..space], 0.0, fmt(p.plain));
                rest = &rest[space..];
                continue;
            }
            let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            let word = &rest[..end];
            let color = match keywords::class(word) {
                Class::Number => p.number,
                Class::Structure if word_index == 0 => p.structure,
                Class::NodeType if after_node => p.node_type,
                Class::Keyed if word_index == 0 => p.keyed,
                Class::Keyword if word_index == 0 => p.keyword,
                _ => p.plain,
            };
            after_node = word_index == 0 && word.eq_ignore_ascii_case("node");
            job.append(word, 0.0, fmt(color));
            word_index += 1;
            rest = &rest[end..];
        }
        if !comment.is_empty() {
            job.append(comment, 0.0, fmt(p.comment));
        }
    }
    job
}

fn severity_color(s: Severity, t: &Tokens) -> Color32 {
    match s {
        Severity::Error => t.error,
        Severity::Warning => t.warn,
        Severity::Info => t.info,
    }
}

/// The ASCII editor's header's name (a group, and its record).
pub const HEADER: &str = "ASCII header";
/// The diagnostics strip's name (its title row).
pub const DIAGNOSTICS: &str = "Diagnostics";
/// The widest the header's state text is drawn (cut with "…" past it,
/// whole in its tip).
const STATE_MOST: f32 = 320.0;

/// The errors and the warnings among `diagnostics`.
pub fn counts(diagnostics: &[Diagnostic]) -> (usize, usize) {
    diagnostics.iter().fold((0, 0), |(e, w), d| match d.severity {
        Severity::Error => (e + 1, w),
        Severity::Warning => (e, w + 1),
        Severity::Info => (e, w),
    })
}

/// "1 error", "2 errors".
pub fn plural(n: usize, what: &str) -> String {
    if n == 1 { format!("{n} {what}") } else { format!("{n} {what}s") }
}

/// The buffer's state: where it is kept (or that it is not yet), and
/// whether it has edits not saved.
fn state_text(buffer: &Buffer) -> String {
    let name = buffer
        .path
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "decompiled, not saved".into());
    if buffer.is_dirty() && buffer.path.is_some() { format!("{name} · edited") } else { name }
}

/// The state's width on the header's row: its words (and the spinner
/// while an edit waits to be applied), at most [`STATE_MOST`].
fn state_width(ui: &Ui, text: &str, pending: bool) -> f32 {
    let font = FontId::proportional(mg_uikit::metrics::BODY_SIZE);
    let words = ui.painter().layout_no_wrap(text.to_owned(), font, Color32::WHITE).size().x;
    let spinner = if pending { mg_uikit::metrics::ICON + 4.0 } else { 0.0 };
    (words + spinner + 2.0).min(STATE_MOST).ceil()
}

/// The state, in weak words cut to `width`, the spinner before them while
/// an edit waits; named "State" and whole in its tip.
fn state_ui(ui: &mut Ui, text: &str, pending: bool, width: f32) {
    if pending {
        ui.add(egui::Spinner::new().size(mg_uikit::metrics::ICON));
    }
    let room = (width - if pending { mg_uikit::metrics::ICON + 4.0 } else { 0.0 }).max(0.0);
    let label = egui::Label::new(egui::RichText::new(text).color(ui.tokens().text_2)).truncate();
    ui.allocate_ui(egui::vec2(room, ui.available_height()), |ui| ui.add(label))
        .inner
        .on_hover_text(text);
}

fn editor_id() -> egui::Id {
    egui::Id::new("mgv-ascii-editor")
}

/// The editor tab.
pub fn ui(app: &mut Viewer, ui: &mut Ui) {
    let now = ui.input(|i| i.time);
    let tokens = ui.tokens();
    let Some(buffer) = &mut app.buffer else {
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            ui.label("ASCII models open here.");
            ui.weak(
                "Open an ASCII model, or decompile a compiled one (Model › Decompile, Ctrl+D).",
            );
            if app.doc.as_ref().is_some_and(|d| mg_mdl::is_binary(&d.opened.data))
                && ui.button("Decompile").clicked()
            {
                app.actions.push(Action::Decompile);
            }
        });
        return;
    };

    // The editor's header: Save, Compile and Compile and View, and the
    // buffer's state at the right (proposal section 9.17).
    let state = state_text(buffer);
    let pending = buffer.pending();
    let state_width = state_width(ui, &state, pending);
    let show_state = move |ui: &mut Ui| state_ui(ui, &state, pending, state_width);
    let out = EditorHeader::new(HEADER)
        .left(Item::button(Icon::Save, "Save").primary().tip("Save the ASCII model (Ctrl+S)"))
        .left(Item::button(Icon::Compile, "Compile").primary().tip("Compile it (Ctrl+B)"))
        .left(
            Item::button(Icon::CompileView, "Compile and View")
                .tip("Compile it and show the compiled model (Ctrl+Shift+B)"),
        )
        .right(Item::custom("State", state_width, show_state.clone()).more(state_width, show_state))
        .show(ui);
    if out.clicked("Save") {
        app.actions.push(Action::Save);
    }
    if out.clicked("Compile") {
        app.actions.push(Action::Compile { view_result: false });
    }
    if out.clicked("Compile and View") {
        app.actions.push(Action::Compile { view_result: true });
    }
    if buffer.disk_changed {
        ui.horizontal(|ui| {
            ui.colored_label(
                severity_color(Severity::Warning, &tokens),
                "The file changed on disk.",
            );
            if ui.button("Load it (drop my edits)").clicked()
                && let Some(p) = buffer.path.clone()
                && let Ok(t) = std::fs::read(&p)
            {
                buffer.replace_from_disk(String::from_utf8_lossy(&t).into_owned());
                buffer.edited(now);
            }
            if ui.button("Keep mine").clicked() {
                buffer.disk_changed = false;
            }
        });
    }

    // The diagnostics, a strip under the text: its title row with the
    // error and warning counts, the lines (a click goes to the line).
    let mut jump: Option<usize> = app.jump_to_line.take();
    let (errors, warnings) = counts(&buffer.diagnostics);
    egui::Panel::bottom("diagnostics")
        .resizable(true)
        .default_size(110.0)
        .size_range(mg_uikit::metrics::DOCK_TABS + 1.0..=f32::INFINITY)
        .frame(egui::Frame::new().fill(tokens.panel))
        .show(ui, |ui| {
            PanelHeader::new(DIAGNOSTICS).show(ui, |ui| {
                let kind = |n: usize, k: BadgeKind| if n > 0 { k } else { BadgeKind::Plain };
                ui.add(Badge::new(plural(errors, "error")).kind(kind(errors, BadgeKind::Error)));
                ui.add_space(4.0);
                ui.add(
                    Badge::new(plural(warnings, "warning")).kind(kind(warnings, BadgeKind::Warn)),
                );
            });
            egui::Frame::new().inner_margin(egui::Margin::symmetric(6, 2)).show(ui, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    if buffer.diagnostics.is_empty() {
                        ui.weak("No problems found.");
                    }
                    for d in &buffer.diagnostics {
                        let text = format!("{}: {}", d.line + 1, d.message);
                        let r = ui.add(
                            egui::Label::new(
                                egui::RichText::new(text)
                                    .color(severity_color(d.severity, &tokens)),
                            )
                            .sense(egui::Sense::click()),
                        );
                        if r.clicked() {
                            jump = Some(d.line);
                        }
                    }
                });
            });
        });

    // The text.
    if let Some(line) = jump {
        buffer.editor.jump = Some(line);
    }
    let font = egui::TextStyle::Monospace.resolve(ui.style());
    let marks = buffer.marks(&tokens);
    let line_font = font.clone();
    let highlight_line = move |line: &str| highlight(line, line_font.clone(), &tokens);
    let out = buffer.editor.show(ui, editor_id(), &font, &highlight_line, &marks);
    if out.changed {
        buffer.edited(now);
        ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64(buffer.delay));
    }
    // The node under the cursor is selected in the view.
    let cursor_line = Some(buffer.editor.cursor.line);
    if cursor_line != buffer.cursor_line {
        buffer.cursor_line = cursor_line;
        if let Some(line) = cursor_line {
            if let Some((index, name)) = buffer.node_at(line) {
                let name = name.to_string();
                app.select_node_at(index, &name);
            } else if let Some(n) = buffer.outline.node_at(line).filter(|n| n.animation.is_none()) {
                let name = n.name.clone();
                app.select_node_named(&name);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `mdl-*` tokens of the dark theme are the colors the editor had
    /// before it took them; the light ones are its light palette.
    #[test]
    fn the_syntax_colors_are_the_tokens() {
        let rgb = |c: Color32| (c.r(), c.g(), c.b());
        let dark = Palette::new(&Tokens::DARK);
        assert_eq!(rgb(dark.plain), (0xD4, 0xD4, 0xD4));
        assert_eq!(rgb(dark.structure), (0xC5, 0x86, 0xC0));
        assert_eq!(rgb(dark.keyword), (0x9C, 0xDC, 0xFE));
        assert_eq!(rgb(dark.keyed), (0x4E, 0xC9, 0xB0));
        assert_eq!(rgb(dark.node_type), (0xDC, 0xDC, 0xAA));
        assert_eq!(rgb(dark.number), (0xB5, 0xCE, 0xA8));
        assert_eq!(rgb(dark.comment), (0x6A, 0x99, 0x55));
        let light = Palette::new(&Tokens::LIGHT);
        assert_eq!(rgb(light.plain), (0x20, 0x20, 0x20));
        assert_eq!(rgb(light.keyword), (0x00, 0x10, 0x80));
    }

    /// Nodes of the same name: the cursor's block is the node of its
    /// place in the model, not the first of the name.
    #[test]
    fn the_source_map_tells_nodes_of_one_name_apart() {
        let text = "newmodel m\nbeginmodelgeom m\nnode dummy m\n  parent NULL\nendnode\n\
                    node dummy a\n  parent m\nendnode\nnode dummy dup\n  parent a\nendnode\n\
                    node dummy dup\n  parent m\nendnode\nendmodelgeom m\n";
        let b = Buffer::new(text.into(), None, None, false);
        // Pre-order: m, a, dup (under a), dup (under m).
        assert_eq!(b.node_at(9), Some((2, "dup")));
        assert_eq!(b.node_at(12), Some((3, "dup")));
        assert_eq!(b.node_line(3, "dup"), Some(11));
        assert_eq!(b.node_line(3, "other"), None, "a model read from other text");
        assert_eq!(b.node_at(0), None);
    }

    /// The diagnostics strip's counts: errors and warnings apart, said in
    /// words.
    #[test]
    fn the_strip_counts_errors_and_warnings() {
        let text = "newmodel m\nbeginmodelgeom m\nnode dummy m\n  parent NULL\nendnode\n\
                    node\nendnode\nendnode\nendmodelgeom m\ndonemodel m\n";
        let d = mg_mdl::lint::check(text);
        let (e, w) = counts(&d);
        assert!(e >= 1 && w >= 1, "{d:?}");
        assert_eq!(counts(&[]), (0, 0));
        assert_eq!(plural(1, "error"), "1 error");
        assert_eq!(plural(0, "warning"), "0 warnings");
    }

    #[test]
    fn highlighting_keeps_every_character() {
        let text = "node trimesh box # c\n  verts 2\n    0 1 2\n  positionkey 1\nendnode";
        let job = highlight(text, FontId::monospace(12.0), &Tokens::DARK);
        assert_eq!(job.text, text);
    }

    #[test]
    fn buffers_track_edits_and_line_ends() {
        let mut b = Buffer::new("a\r\nb\r\n".into(), None, None, false);
        assert_eq!(b.text(), "a\nb\n");
        assert!(!b.is_dirty());
        b.set_text("a\nb\nc", 1.0);
        assert!(b.is_dirty());
        assert!(!b.reload_due(1.05));
        assert!(b.reload_due(1.2));
        assert!(!b.reload_due(1.3), "once per edit");
        let dir = std::env::temp_dir().join(format!("mgv-ui-buffer-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("x.mdl");
        b.save(&p).unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"a\r\nb\r\nc");
        assert!(!b.is_dirty());
        assert!(b.matches_disk("a\r\nb\r\nc"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
