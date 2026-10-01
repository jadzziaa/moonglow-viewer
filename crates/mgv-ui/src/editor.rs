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
use mg_resman::ResKey;
use mgv_mdl::keywords::{self, Class};
use mgv_mdl::{Diagnostic, Outline, Severity};

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

    /// Recomputes the outline and diagnostics.
    pub fn refresh_analysis(&mut self) {
        let text = self.text();
        self.outline = mgv_mdl::outline(&text);
        self.diagnostics = mgv_mdl::lint::check(&text);
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
    fn marks(&self) -> std::collections::HashMap<usize, Color32> {
        let mut m: std::collections::HashMap<usize, Severity> = std::collections::HashMap::new();
        for d in &self.diagnostics {
            let e = m.entry(d.line).or_insert(d.severity);
            *e = (*e).max(d.severity);
        }
        m.into_iter().map(|(l, s)| (l, severity_color(s))).collect()
    }
}

/// Colours of the highlighting, for light and dark themes.
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
    fn new(dark: bool) -> Palette {
        if dark {
            Palette {
                plain: Color32::from_rgb(0xD4, 0xD4, 0xD4),
                structure: Color32::from_rgb(0xC5, 0x86, 0xC0),
                keyword: Color32::from_rgb(0x9C, 0xDC, 0xFE),
                keyed: Color32::from_rgb(0x4E, 0xC9, 0xB0),
                node_type: Color32::from_rgb(0xDC, 0xDC, 0xAA),
                number: Color32::from_rgb(0xB5, 0xCE, 0xA8),
                comment: Color32::from_rgb(0x6A, 0x99, 0x55),
            }
        } else {
            Palette {
                plain: Color32::from_rgb(0x20, 0x20, 0x20),
                structure: Color32::from_rgb(0xAF, 0x00, 0xDB),
                keyword: Color32::from_rgb(0x00, 0x10, 0x80),
                keyed: Color32::from_rgb(0x26, 0x7F, 0x99),
                node_type: Color32::from_rgb(0x79, 0x5E, 0x26),
                number: Color32::from_rgb(0x09, 0x86, 0x58),
                comment: Color32::from_rgb(0x00, 0x80, 0x00),
            }
        }
    }
}

/// The text as coloured sections: the first word of a line by what it is
/// (a word after `node` is the node type), numbers, comments.
pub fn highlight(text: &str, font: FontId, dark: bool) -> LayoutJob {
    let p = Palette::new(dark);
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

fn severity_color(s: Severity) -> Color32 {
    match s {
        Severity::Error => Color32::from_rgb(0xE5, 0x48, 0x4D),
        Severity::Warning => Color32::from_rgb(0xE8, 0xA3, 0x3D),
        Severity::Info => Color32::from_rgb(0x5B, 0x9B, 0xD5),
    }
}

fn editor_id() -> egui::Id {
    egui::Id::new("mgv-ascii-editor")
}

/// The editor tab.
pub fn ui(app: &mut Viewer, ui: &mut Ui) {
    let now = ui.input(|i| i.time);
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

    // Toolbar.
    ui.horizontal(|ui| {
        let name = buffer
            .path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "decompiled, not saved".into());
        ui.label(egui::RichText::new(name).strong());
        if buffer.is_dirty() {
            ui.weak("edited");
        }
        if buffer.pending() {
            ui.spinner();
        }
        ui.separator();
        if ui.button("Save").on_hover_text("Ctrl+S").clicked() {
            app.actions.push(Action::Save);
        }
        if ui.button("Compile").on_hover_text("Ctrl+B").clicked() {
            app.actions.push(Action::Compile { view_result: false });
        }
        if ui.button("Compile and View").on_hover_text("Ctrl+Shift+B").clicked() {
            app.actions.push(Action::Compile { view_result: true });
        }
        let (e, w) = buffer.diagnostics.iter().fold((0, 0), |(e, w), d| match d.severity {
            Severity::Error => (e + 1, w),
            Severity::Warning => (e, w + 1),
            Severity::Info => (e, w),
        });
        ui.separator();
        ui.colored_label(severity_color(Severity::Error), format!("{e} errors"));
        ui.colored_label(severity_color(Severity::Warning), format!("{w} warnings"));
    });
    if buffer.disk_changed {
        ui.horizontal(|ui| {
            ui.colored_label(severity_color(Severity::Warning), "The file changed on disk.");
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

    // Diagnostics below the text.
    let mut jump: Option<usize> = app.jump_to_line.take();
    egui::Panel::bottom("diagnostics").resizable(true).default_size(90.0).show(ui, |ui| {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            if buffer.diagnostics.is_empty() {
                ui.weak("No problems found.");
            }
            for d in &buffer.diagnostics {
                let text = format!("{}: {}", d.line + 1, d.message);
                let r = ui.add(
                    egui::Label::new(egui::RichText::new(text).color(severity_color(d.severity)))
                        .sense(egui::Sense::click()),
                );
                if r.clicked() {
                    jump = Some(d.line);
                }
            }
        });
    });

    // The text.
    if let Some(line) = jump {
        buffer.editor.jump = Some(line);
    }
    let font = egui::TextStyle::Monospace.resolve(ui.style());
    let dark = ui.visuals().dark_mode;
    let marks = buffer.marks();
    let line_font = font.clone();
    let highlight_line = move |line: &str| highlight(line, line_font.clone(), dark);
    let out = buffer.editor.show(ui, editor_id(), &font, &highlight_line, &marks);
    if out.changed {
        buffer.edited(now);
        ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64(buffer.delay));
    }
    // The node under the cursor is selected in the view.
    let cursor_line = Some(buffer.editor.cursor.line);
    if cursor_line != buffer.cursor_line {
        buffer.cursor_line = cursor_line;
        if let Some(line) = cursor_line
            && let Some(n) = buffer.outline.node_at(line).filter(|n| n.animation.is_none())
        {
            let name = n.name.clone();
            app.select_node_named(&name);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlighting_keeps_every_character() {
        let text = "node trimesh box # c\n  verts 2\n    0 1 2\n  positionkey 1\nendnode";
        let job = highlight(text, FontId::monospace(12.0), true);
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
