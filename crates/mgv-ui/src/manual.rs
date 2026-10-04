//! Help › User Manual: the manual (`docs/manual`), built into the program,
//! a chapter at a time beside the list of chapters; a link to another
//! chapter opens it in place. And Help › About.

use egui::Ui;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};

use crate::Viewer;

/// The chapters: file name and text, the contents first.
pub(crate) const CHAPTERS: [(&str, &str); 7] = [
    ("README.md", include_str!("../../../docs/manual/README.md")),
    ("01-getting-started.md", include_str!("../../../docs/manual/01-getting-started.md")),
    ("02-the-window.md", include_str!("../../../docs/manual/02-the-window.md")),
    ("03-editing-models.md", include_str!("../../../docs/manual/03-editing-models.md")),
    ("04-lighting-and-effects.md", include_str!("../../../docs/manual/04-lighting-and-effects.md")),
    ("05-command-line.md", include_str!("../../../docs/manual/05-command-line.md")),
    ("06-troubleshooting.md", include_str!("../../../docs/manual/06-troubleshooting.md")),
];

/// A chapter without its frontmatter. The manual is an Open Knowledge
/// Format bundle (`docs/index.md`): each chapter opens with a YAML block
/// between `---` lines, which is for tools and not for the reader.
pub(crate) fn body(text: &str) -> &str {
    let mut end = 0;
    for (i, line) in text.split_inclusive('\n').enumerate() {
        end += line.len();
        match (i, line.trim_end() == "---") {
            (0, false) => return text,
            (0, true) => {}
            (_, true) => return text[end..].trim_start(),
            _ => {}
        }
    }
    text
}

/// A chapter's title: its first heading.
pub(crate) fn title(text: &str) -> &str {
    body(text).lines().find_map(|l| l.strip_prefix("# ")).unwrap_or_default()
}

/// The manual's state.
#[derive(Debug, Default)]
pub struct Manual {
    pub open: bool,
    pub about: bool,
    chapter: usize,
    cache: CommonMarkCache,
}

/// The manual and About windows, while open.
pub(crate) fn windows(app: &mut Viewer, ctx: &egui::Context) {
    let m = &mut app.manual;
    if m.open {
        let mut open = true;
        egui::Window::new("User Manual")
            .open(&mut open)
            .default_size([900.0, 640.0])
            .show(ctx, |ui| chapter_ui(m, ui));
        m.open = open;
    }
    if m.about {
        let mut open = true;
        egui::Window::new("About Moonglow Viewer")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("Moonglow Viewer");
                    ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
                    ui.add_space(6.0);
                    ui.label("A model viewer for Neverwinter Nights: Enhanced Edition.");
                });
                ui.add_space(10.0);
                ui.label(
                    "Moonglow Viewer is free software: you can redistribute it and/or modify \
                     it under the terms of the GNU General Public License, version 3. It \
                     comes with no warranty.",
                );
                ui.add_space(6.0);
                ui.label(
                    "It is built on Moonglow Toolset's crates, egui, wgpu and other \
                     open-source libraries; their licenses are in THIRD-PARTY-LICENSES.txt, \
                     beside the program.",
                );
                ui.add_space(6.0);
                ui.label(
                    "Moonglow Viewer is not affiliated with Beamdog or Wizards of the Coast, \
                     and contains no game data: it reads your installation of the game.",
                );
            });
        m.about = open;
    }
}

fn chapter_ui(m: &mut Manual, ui: &mut Ui) {
    let mut go = None;
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(190.0);
            for (i, (_, text)) in CHAPTERS.iter().enumerate() {
                let name = if i == 0 { "Contents" } else { title(text) };
                if ui.selectable_label(m.chapter == i, name).clicked() {
                    go = Some(i);
                }
            }
        });
        ui.separator();
        let (_, text) = CHAPTERS[m.chapter.min(CHAPTERS.len() - 1)];
        for (file, _) in CHAPTERS {
            m.cache.add_link_hook(file);
        }
        egui::ScrollArea::vertical().id_salt(("manual", m.chapter)).auto_shrink(false).show(
            ui,
            |ui| {
                ui.set_max_width(760.0);
                CommonMarkViewer::new().show(ui, &mut m.cache, body(text));
            },
        );
        go = go.or_else(|| {
            CHAPTERS.iter().position(|(file, _)| m.cache.get_link_hook(file) == Some(true))
        });
    });
    if let Some(i) = go {
        m.chapter = i;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_chapter_has_a_title_and_the_contents_links_them() {
        let contents = CHAPTERS[0].1;
        for (file, text) in &CHAPTERS[1..] {
            assert!(!title(text).is_empty(), "{file}");
            assert!(contents.contains(&format!("({file})")), "{file} is in the contents");
        }
    }

    #[test]
    fn a_chapter_is_shown_without_its_frontmatter() {
        assert_eq!(body("---\ntype: Manual Page\n---\n\n# Keys\n"), "# Keys\n");
        assert_eq!(body("---\r\ntitle: a\r\n---\r\n# Keys\r\n"), "# Keys\r\n");
        assert_eq!(body("# Keys\n\n---\n\nMore.\n"), "# Keys\n\n---\n\nMore.\n");
        assert_eq!(body("---\nnever closed\n"), "---\nnever closed\n");
        for (file, text) in CHAPTERS {
            // (A Windows checkout has the chapters with CRLF line ends.)
            assert!(body(text).len() < text.len(), "{file} has frontmatter");
            assert!(body(text).starts_with("# "), "{file} is shown from its heading");
            let crlf = text.replace("\r\n", "\n").replace('\n', "\r\n");
            assert!(body(&crlf).starts_with("# "), "{file} is shown from its heading (CRLF)");
        }
    }
}
