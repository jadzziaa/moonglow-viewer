//! A text editor for large files: the text is a list of lines, and only the
//! lines in sight are laid out and drawn, so a frame costs the same for a
//! 2,000-line placeable as for the 126,000-line `a_ba` (egui's `TextEdit`
//! lays out the whole text on every change: 0.6 s per keystroke there).
//!
//! It has what editing a model needs: a cursor and selection by keyboard
//! and mouse, word moves, clipboard, undo and redo, indentation kept on new
//! lines, Tab indenting a selection, and jumping to a line.

use std::collections::HashMap;

use egui::text::{CCursor, LayoutJob};
use egui::{Color32, Event, EventFilter, FontId, Key, Modifiers, Rect, Sense, Ui, pos2, vec2};

/// A place in the text: a line and a character within it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Pos {
    pub line: usize,
    /// In characters.
    pub col: usize,
}

impl Pos {
    pub fn new(line: usize, col: usize) -> Pos {
        Pos { line, col }
    }
}

/// One change, for undo: the text between `start` and `end_before` was
/// `removed` and became `inserted`.
#[derive(Debug, Clone, PartialEq)]
struct Edit {
    start: Pos,
    removed: String,
    inserted: String,
    cursor_before: Pos,
    anchor_before: Option<Pos>,
    cursor_after: Pos,
    /// Typing that may merge with the next keystroke.
    typing: bool,
}

/// What a frame did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Output {
    pub changed: bool,
    pub cursor_moved: bool,
}

/// The editor's text and state.
#[derive(Debug, Clone)]
pub struct CodeEditor {
    lines: Vec<String>,
    pub cursor: Pos,
    /// The other end of the selection.
    pub anchor: Option<Pos>,
    /// Where up and down aim (points from the line's start).
    preferred_x: Option<f32>,
    undo: Vec<Edit>,
    redo: Vec<Edit>,
    /// Bumped on every change.
    pub revision: u64,
    /// The longest line, in characters (for the scroll width).
    widest: usize,
    scroll_to_cursor: bool,
    /// Show this line (and put the cursor there) on the next frame.
    pub jump: Option<usize>,
}

fn char_to_byte(s: &str, col: usize) -> usize {
    s.char_indices().nth(col).map_or(s.len(), |(i, _)| i)
}

fn char_len(s: &str) -> usize {
    s.chars().count()
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '.' || c == '-'
}

impl CodeEditor {
    pub fn new(text: &str) -> CodeEditor {
        let mut e = CodeEditor {
            lines: Vec::new(),
            cursor: Pos::default(),
            anchor: None,
            preferred_x: None,
            undo: Vec::new(),
            redo: Vec::new(),
            revision: 0,
            widest: 0,
            scroll_to_cursor: false,
            jump: None,
        };
        e.set_text(text);
        e
    }

    /// Replaces the whole text (no undo), keeping the cursor where it can.
    pub fn set_text(&mut self, text: &str) {
        self.lines = text.split('\n').map(str::to_string).collect();
        self.widest = self.lines.iter().map(|l| char_len(l)).max().unwrap_or(0);
        self.cursor = self.clamp(self.cursor);
        self.anchor = None;
        self.undo.clear();
        self.redo.clear();
        self.revision += 1;
    }

    /// The text, lines joined with `\n`.
    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    fn clamp(&self, p: Pos) -> Pos {
        let line = p.line.min(self.lines.len().saturating_sub(1));
        Pos { line, col: p.col.min(char_len(&self.lines[line])) }
    }

    /// The selection, ordered.
    pub fn selection(&self) -> Option<(Pos, Pos)> {
        let a = self.anchor?;
        (a != self.cursor)
            .then(|| if a < self.cursor { (a, self.cursor) } else { (self.cursor, a) })
    }

    /// The text between two places.
    pub fn slice(&self, a: Pos, b: Pos) -> String {
        if a.line == b.line {
            let l = &self.lines[a.line];
            return l[char_to_byte(l, a.col)..char_to_byte(l, b.col)].to_string();
        }
        let mut out = String::new();
        let first = &self.lines[a.line];
        out.push_str(&first[char_to_byte(first, a.col)..]);
        for l in &self.lines[a.line + 1..b.line] {
            out.push('\n');
            out.push_str(l);
        }
        out.push('\n');
        let last = &self.lines[b.line];
        out.push_str(&last[..char_to_byte(last, b.col)]);
        out
    }

    /// Replaces the text between `a` and `b` (ordered) with `text`; returns
    /// the end of the inserted text.
    fn splice(&mut self, a: Pos, b: Pos, text: &str) -> Pos {
        let head = self.lines[a.line][..char_to_byte(&self.lines[a.line], a.col)].to_string();
        let tail = self.lines[b.line][char_to_byte(&self.lines[b.line], b.col)..].to_string();
        let mut new: Vec<String> = text.split('\n').map(str::to_string).collect();
        let end_col = char_len(new.last().expect("split gives one"));
        let end_line = a.line + new.len() - 1;
        let end = Pos::new(end_line, if new.len() == 1 { a.col + end_col } else { end_col });
        new[0] = format!("{head}{}", new[0]);
        let last = new.len() - 1;
        new[last].push_str(&tail);
        self.widest = self.widest.max(new.iter().map(|l| char_len(l)).max().unwrap_or(0));
        self.lines.splice(a.line..=b.line, new);
        self.revision += 1;
        end
    }

    /// An undoable replacement of the selection (or nothing) with `text`.
    fn replace_selection(&mut self, text: &str, typing: bool) {
        let (a, b) = self.selection().unwrap_or((self.cursor, self.cursor));
        self.replace(a, b, text, typing);
    }

    fn replace(&mut self, a: Pos, b: Pos, text: &str, typing: bool) {
        let removed = self.slice(a, b);
        let edit_before = (self.cursor, self.anchor);
        let end = self.splice(a, b, text);
        self.cursor = end;
        self.anchor = None;
        self.preferred_x = None;
        self.scroll_to_cursor = true;
        self.redo.clear();
        // Typing goes into the previous step while it continues there.
        if typing
            && removed.is_empty()
            && !text.contains('\n')
            && let Some(last) = self.undo.last_mut()
            && last.typing
            && last.cursor_after == a
        {
            last.inserted.push_str(text);
            last.cursor_after = end;
            return;
        }
        self.undo.push(Edit {
            start: a,
            removed,
            inserted: text.to_string(),
            cursor_before: edit_before.0,
            anchor_before: edit_before.1,
            cursor_after: end,
            typing,
        });
    }

    /// The end of a text inserted at `start`.
    fn end_of(start: Pos, text: &str) -> Pos {
        let parts: Vec<&str> = text.split('\n').collect();
        let last = char_len(parts[parts.len() - 1]);
        if parts.len() == 1 {
            Pos::new(start.line, start.col + last)
        } else {
            Pos::new(start.line + parts.len() - 1, last)
        }
    }

    pub fn undo(&mut self) -> bool {
        let Some(e) = self.undo.pop() else { return false };
        let end = Self::end_of(e.start, &e.inserted);
        self.splice(e.start, end, &e.removed);
        self.cursor = e.cursor_before;
        self.anchor = e.anchor_before;
        self.scroll_to_cursor = true;
        self.redo.push(e);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(e) = self.redo.pop() else { return false };
        let end = Self::end_of(e.start, &e.removed);
        self.splice(e.start, end, &e.inserted);
        self.cursor = e.cursor_after;
        self.anchor = None;
        self.scroll_to_cursor = true;
        let mut e = e;
        e.typing = false;
        self.undo.push(e);
        true
    }

    /// Moves the cursor, extending the selection with `select`.
    fn move_to(&mut self, p: Pos, select: bool) {
        if select {
            self.anchor.get_or_insert(self.cursor);
        } else {
            self.anchor = None;
        }
        self.cursor = self.clamp(p);
        self.scroll_to_cursor = true;
    }

    fn left(&self, p: Pos, word: bool) -> Pos {
        if p.col == 0 {
            return if p.line == 0 {
                p
            } else {
                Pos::new(p.line - 1, char_len(&self.lines[p.line - 1]))
            };
        }
        if !word {
            return Pos::new(p.line, p.col - 1);
        }
        let chars: Vec<char> = self.lines[p.line].chars().collect();
        let mut c = p.col;
        while c > 0 && !is_word(chars[c - 1]) {
            c -= 1;
        }
        while c > 0 && is_word(chars[c - 1]) {
            c -= 1;
        }
        Pos::new(p.line, c)
    }

    fn right(&self, p: Pos, word: bool) -> Pos {
        let len = char_len(&self.lines[p.line]);
        if p.col >= len {
            return if p.line + 1 >= self.lines.len() { p } else { Pos::new(p.line + 1, 0) };
        }
        if !word {
            return Pos::new(p.line, p.col + 1);
        }
        let chars: Vec<char> = self.lines[p.line].chars().collect();
        let mut c = p.col;
        while c < len && is_word(chars[c]) {
            c += 1;
        }
        while c < len && !is_word(chars[c]) {
            c += 1;
        }
        Pos::new(p.line, c)
    }

    fn home(&self, p: Pos) -> Pos {
        // First to the indentation, then to the line's start.
        let indent = self.lines[p.line].chars().take_while(|c| c.is_whitespace()).count();
        Pos::new(p.line, if p.col == indent { 0 } else { indent })
    }

    /// The word around a place.
    fn word_at(&self, p: Pos) -> (Pos, Pos) {
        let chars: Vec<char> = self.lines[p.line].chars().collect();
        let (mut a, mut b) = (p.col.min(chars.len()), p.col.min(chars.len()));
        while a > 0 && is_word(chars[a - 1]) {
            a -= 1;
        }
        while b < chars.len() && is_word(chars[b]) {
            b += 1;
        }
        (Pos::new(p.line, a), Pos::new(p.line, b))
    }

    /// Indents (or outdents) the selected lines by two spaces.
    fn indent(&mut self, outdent: bool) {
        let (a, b) = self.selection().unwrap_or((self.cursor, self.cursor));
        let last = if b.col == 0 && b.line > a.line { b.line - 1 } else { b.line };
        let old = self.slice(Pos::new(a.line, 0), Pos::new(last, char_len(&self.lines[last])));
        let new: Vec<String> = old
            .split('\n')
            .map(|l| {
                if outdent {
                    let strip = l.chars().take(2).take_while(|c| *c == ' ').count();
                    l[strip..].to_string()
                } else {
                    format!("  {l}")
                }
            })
            .collect();
        let new = new.join("\n");
        let end_col = char_len(new.split('\n').next_back().unwrap_or(""));
        self.replace(Pos::new(a.line, 0), Pos::new(last, char_len(&self.lines[last])), &new, false);
        self.anchor = Some(Pos::new(a.line, 0));
        self.cursor = Pos::new(last, end_col);
    }

    /// Types `text` at the cursor (replacing the selection), as a keystroke.
    pub fn insert(&mut self, text: &str) {
        self.replace_selection(text, true);
    }

    /// Puts the cursor at a line's start and shows it.
    pub fn go_to_line(&mut self, line: usize) {
        self.cursor = self.clamp(Pos::new(line, 0));
        self.anchor = None;
        self.scroll_to_cursor = true;
    }

    /// Handles the keyboard and clipboard while focused.
    fn input(&mut self, ui: &Ui, rows_per_page: usize) -> bool {
        let events = ui.input(|i| i.events.clone());
        let mut changed = false;
        for ev in events {
            match ev {
                Event::Text(t) | Event::Paste(t) if !t.is_empty() => {
                    let typing = t.chars().count() == 1;
                    let t = t.replace("\r\n", "\n");
                    self.replace_selection(&t, typing);
                    changed = true;
                }
                Event::Ime(egui::ImeEvent::Commit(t)) if !t.is_empty() => {
                    self.replace_selection(&t, false);
                    changed = true;
                }
                Event::Copy => {
                    if let Some((a, b)) = self.selection() {
                        ui.ctx().copy_text(self.slice(a, b));
                    }
                }
                Event::Cut => {
                    if let Some((a, b)) = self.selection() {
                        ui.ctx().copy_text(self.slice(a, b));
                        self.replace(a, b, "", false);
                        changed = true;
                    }
                }
                Event::Key { key, pressed: true, modifiers, .. } => {
                    changed |= self.key(key, modifiers, rows_per_page);
                }
                _ => {}
            }
        }
        changed
    }

    fn key(&mut self, key: Key, m: Modifiers, page: usize) -> bool {
        let select = m.shift;
        let word = m.command || m.alt;
        match key {
            Key::ArrowLeft => {
                let to = match (self.selection(), select) {
                    (Some((a, _)), false) => a,
                    _ => self.left(self.cursor, word),
                };
                self.move_to(to, select);
                self.preferred_x = None;
            }
            Key::ArrowRight => {
                let to = match (self.selection(), select) {
                    (Some((_, b)), false) => b,
                    _ => self.right(self.cursor, word),
                };
                self.move_to(to, select);
                self.preferred_x = None;
            }
            Key::ArrowUp | Key::ArrowDown | Key::PageUp | Key::PageDown => {
                let rows = if matches!(key, Key::PageUp | Key::PageDown) { page.max(1) } else { 1 };
                let line = if matches!(key, Key::ArrowUp | Key::PageUp) {
                    self.cursor.line.saturating_sub(rows)
                } else {
                    (self.cursor.line + rows).min(self.lines.len() - 1)
                };
                // Keep the column (characters stand for the x position:
                // the editor's font is monospaced).
                let col = self.preferred_x.map_or(self.cursor.col, |x| x as usize);
                self.preferred_x = Some(col as f32);
                self.move_to(Pos::new(line, col), select);
            }
            Key::Home => {
                let to = if m.command { Pos::default() } else { self.home(self.cursor) };
                self.move_to(to, select);
                self.preferred_x = None;
            }
            Key::End => {
                let to = if m.command {
                    let l = self.lines.len() - 1;
                    Pos::new(l, char_len(&self.lines[l]))
                } else {
                    Pos::new(self.cursor.line, char_len(&self.lines[self.cursor.line]))
                };
                self.move_to(to, select);
                self.preferred_x = None;
            }
            Key::A if m.command => {
                let l = self.lines.len() - 1;
                self.anchor = Some(Pos::default());
                self.cursor = Pos::new(l, char_len(&self.lines[l]));
            }
            Key::Z if m.command && !m.shift => return self.undo(),
            Key::Z if m.command && m.shift => return self.redo(),
            Key::Y if m.command => return self.redo(),
            Key::Backspace => {
                if self.selection().is_none() {
                    let from = self.left(self.cursor, word);
                    self.anchor = Some(from);
                }
                if self.selection().is_some() {
                    self.replace_selection("", false);
                    return true;
                }
            }
            Key::Delete => {
                if self.selection().is_none() {
                    let to = self.right(self.cursor, word);
                    self.anchor = Some(to);
                }
                if self.selection().is_some() {
                    self.replace_selection("", false);
                    return true;
                }
            }
            Key::Enter => {
                // A new line keeps the indentation.
                let indent: String = self.lines[self.cursor.line]
                    .chars()
                    .take_while(|c| *c == ' ' || *c == '\t')
                    .collect();
                self.replace_selection(&format!("\n{indent}"), false);
                return true;
            }
            Key::Tab => {
                let multi = self.selection().is_some_and(|(a, b)| a.line != b.line);
                if m.shift || multi {
                    self.indent(m.shift);
                } else {
                    self.replace_selection("  ", true);
                }
                return true;
            }
            _ => {}
        }
        false
    }

    /// Draws the editor filling the space; `highlight` lays out one line
    /// (without its line end); `marks` colours the gutter by line.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        id: egui::Id,
        font: &FontId,
        highlight: &dyn Fn(&str) -> LayoutJob,
        marks: &HashMap<usize, Color32>,
    ) -> Output {
        let row = ui.fonts_mut(|f| f.row_height(font));
        let char_w = ui.fonts_mut(|f| f.glyph_width(font, 'M'));
        let gutter = (self.lines.len().to_string().len() as f32 + 2.0) * char_w;
        let content =
            vec2(gutter + (self.widest as f32 + 4.0) * char_w, self.lines.len() as f32 * row + row);
        let cursor_before = (self.cursor, self.anchor);
        let mut out = Output::default();
        if let Some(line) = self.jump.take() {
            self.go_to_line(line);
        }
        let mut scroll_target = None;
        let text_color = ui.visuals().text_color();
        let weak = ui.visuals().weak_text_color();
        let select_color = ui.visuals().selection.bg_fill;
        let cursor_color = ui.visuals().text_cursor.stroke.color;
        egui::ScrollArea::both().id_salt(id).auto_shrink([false, false]).show_viewport(
            ui,
            |ui, viewport| {
                let (rect, _) =
                    ui.allocate_exact_size(content.max(viewport.size()), Sense::hover());
                let response = ui.interact(rect, id, Sense::click_and_drag());
                let origin = rect.min;
                let text_x = origin.x + gutter;
                let first = (viewport.top() / row).floor().max(0.0) as usize;
                let last = ((viewport.bottom() / row).ceil() as usize + 1).min(self.lines.len());
                let page = ((viewport.height() / row) as usize).saturating_sub(2);

                // Focus and keyboard.
                if response.clicked() || response.drag_started() {
                    response.request_focus();
                }
                let focused = response.has_focus();
                if focused {
                    ui.memory_mut(|m| {
                        m.set_focus_lock_filter(
                            id,
                            EventFilter {
                                tab: true,
                                horizontal_arrows: true,
                                vertical_arrows: true,
                                escape: false,
                            },
                        )
                    });
                    if self.input(ui, page) {
                        out.changed = true;
                    }
                }

                // The mouse: click places, drag selects, double-click a word.
                let col_at = |editor: &CodeEditor, line: usize, x: f32| -> usize {
                    let g = ui.fonts_mut(|f| {
                        f.layout_no_wrap(editor.lines[line].clone(), font.clone(), text_color)
                    });
                    g.cursor_from_pos(vec2(x - text_x, 0.0)).index.into()
                };
                if let Some(p) = response.interact_pointer_pos() {
                    let line = (((p.y - origin.y) / row).floor().max(0.0) as usize)
                        .min(self.lines.len() - 1);
                    let at = Pos::new(line, col_at(self, line, p.x));
                    if response.double_clicked() {
                        let (a, b) = self.word_at(at);
                        self.anchor = Some(a);
                        self.cursor = b;
                    } else if response.drag_started() || response.clicked() {
                        let shift = ui.input(|i| i.modifiers.shift);
                        self.move_to(at, shift);
                        self.scroll_to_cursor = false;
                    } else if response.dragged() {
                        self.move_to(at, true);
                        self.scroll_to_cursor = false;
                    }
                    self.preferred_x = None;
                }

                let painter = ui.painter_at(ui.clip_rect());
                let sel = self.selection();
                for line in first..last {
                    let y = origin.y + line as f32 * row;
                    // Gutter.
                    if let Some(c) = marks.get(&line) {
                        painter.rect_filled(
                            Rect::from_min_size(pos2(origin.x, y), vec2(3.0, row)),
                            0.0,
                            *c,
                        );
                    }
                    painter.text(
                        pos2(text_x - char_w, y),
                        egui::Align2::RIGHT_TOP,
                        (line + 1).to_string(),
                        font.clone(),
                        weak,
                    );
                    let galley = ui.fonts_mut(|f| f.layout_job(highlight(&self.lines[line])));
                    // Selection behind the text.
                    if let Some((a, b)) = sel
                        && line >= a.line
                        && line <= b.line
                    {
                        let from = if line == a.line { a.col } else { 0 };
                        let to = if line == b.line { b.col } else { char_len(&self.lines[line]) };
                        let x0 = galley.pos_from_cursor(CCursor::new(from)).min.x;
                        let mut x1 = galley.pos_from_cursor(CCursor::new(to)).min.x;
                        if line != b.line {
                            x1 += char_w * 0.6;
                        }
                        painter.rect_filled(
                            Rect::from_min_max(pos2(text_x + x0, y), pos2(text_x + x1, y + row)),
                            0.0,
                            select_color,
                        );
                    }
                    painter.galley(pos2(text_x, y), galley.clone(), text_color);
                    if line == self.cursor.line {
                        let x = galley.pos_from_cursor(CCursor::new(self.cursor.col)).min.x;
                        if focused {
                            painter.line_segment(
                                [pos2(text_x + x, y), pos2(text_x + x, y + row)],
                                egui::Stroke::new(2.0, cursor_color),
                            );
                        }
                    }
                }
                if self.scroll_to_cursor {
                    self.scroll_to_cursor = false;
                    let y = origin.y + self.cursor.line as f32 * row;
                    let x = text_x + self.cursor.col as f32 * char_w;
                    scroll_target = Some(Rect::from_min_size(pos2(x, y), vec2(char_w, row)));
                }
                if focused {
                    ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
                }
                if let Some(r) = scroll_target {
                    // Keep a margin of lines around it.
                    ui.scroll_to_rect(r.expand2(vec2(char_w * 4.0, row * 3.0)), None);
                }
            },
        );
        out.cursor_moved = (self.cursor, self.anchor) != cursor_before;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_merges_and_undoes() {
        let mut e = CodeEditor::new("node dummy a\nendnode");
        e.cursor = Pos::new(0, 12);
        for c in ['b', 'c', 'd'] {
            e.replace_selection(&c.to_string(), true);
        }
        assert_eq!(e.lines()[0], "node dummy abcd");
        assert_eq!(e.undo.len(), 1, "one step for the run of typing");
        e.undo();
        assert_eq!(e.text(), "node dummy a\nendnode");
        e.redo();
        assert_eq!(e.lines()[0], "node dummy abcd");
    }

    #[test]
    fn multi_line_edits() {
        let mut e = CodeEditor::new("one\ntwo\nthree");
        e.anchor = Some(Pos::new(0, 1));
        e.cursor = Pos::new(2, 2);
        assert_eq!(e.slice(Pos::new(0, 1), Pos::new(2, 2)), "ne\ntwo\nth");
        e.replace_selection("X\nY", false);
        assert_eq!(e.text(), "oX\nYree");
        assert_eq!(e.cursor, Pos::new(1, 1));
        e.undo();
        assert_eq!(e.text(), "one\ntwo\nthree");
        assert_eq!((e.cursor, e.anchor), (Pos::new(2, 2), Some(Pos::new(0, 1))));
    }

    #[test]
    fn keys() {
        let mut e = CodeEditor::new("  verts 3\n    0 0 0");
        e.cursor = Pos::new(0, 9);
        e.key(Key::Enter, Modifiers::NONE, 10);
        assert_eq!(e.lines()[1], "  ", "a new line keeps the indentation");
        e.key(Key::Backspace, Modifiers::NONE, 10);
        e.key(Key::Backspace, Modifiers::NONE, 10);
        e.key(Key::Backspace, Modifiers::NONE, 10);
        assert_eq!(e.text(), "  verts 3\n    0 0 0");
        e.cursor = Pos::new(0, 9);
        e.key(Key::ArrowLeft, Modifiers::COMMAND, 10);
        assert_eq!(e.cursor, Pos::new(0, 8));
        e.key(Key::Home, Modifiers::NONE, 10);
        assert_eq!(e.cursor, Pos::new(0, 2), "home goes to the indentation first");
        e.key(Key::Home, Modifiers::NONE, 10);
        assert_eq!(e.cursor, Pos::new(0, 0));
        e.key(Key::ArrowDown, Modifiers::SHIFT, 10);
        assert_eq!(e.selection(), Some((Pos::new(0, 0), Pos::new(1, 0))));
        e.key(Key::Tab, Modifiers::SHIFT, 10);
        assert_eq!(e.text(), "verts 3\n    0 0 0", "shift-tab outdents the selected line");
        e.key(Key::A, Modifiers::COMMAND, 10);
        e.key(Key::Delete, Modifiers::NONE, 10);
        assert_eq!(e.text(), "");
        e.undo();
        assert_eq!(e.text(), "verts 3\n    0 0 0");
    }

    #[test]
    fn words_and_ends() {
        let e = CodeEditor::new("position 0.5 -1.25 3");
        assert_eq!(e.word_at(Pos::new(0, 14)), (Pos::new(0, 13), Pos::new(0, 18)));
        assert_eq!(e.right(Pos::new(0, 0), true), Pos::new(0, 9));
        assert_eq!(e.left(Pos::new(0, 0), false), Pos::new(0, 0));
        assert_eq!(CodeEditor::end_of(Pos::new(3, 4), "ab\ncde"), Pos::new(4, 3));
        assert_eq!(CodeEditor::end_of(Pos::new(3, 4), "ab"), Pos::new(3, 6));
    }
}
