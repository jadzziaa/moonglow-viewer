//! Widgets shared by the panels, styled as Moonglow Toolset's.

/// The bold font family (Ubuntu Bold, the bold of egui's own Ubuntu), for
/// section headings.
const BOLD: &str = "bold";

/// Adds the bold font family to `ctx`'s fonts (once a context; it is there
/// from the next frame): Ubuntu Bold, then egui's emoji fonts for the
/// glyphs it lacks.
pub(crate) fn install_fonts(ctx: &egui::Context) {
    let done = egui::Id::new("moonglow-fonts");
    if ctx.data(|d| d.get_temp::<bool>(done)).is_some() {
        return;
    }
    ctx.data_mut(|d| d.insert_temp(done, true));
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "Ubuntu-Bold".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../fonts/Ubuntu-Bold.ttf"
        ))),
    );
    let fallbacks = fonts.families.get(&egui::FontFamily::Proportional).cloned();
    let mut bold = vec!["Ubuntu-Bold".to_string()];
    bold.extend(fallbacks.unwrap_or_default().into_iter().skip(1));
    fonts.families.insert(egui::FontFamily::Name(BOLD.into()), bold);
    ctx.set_fonts(fonts);
}

/// A field's label (the first column of a grid: "Name", "Kind"…): in the
/// strong text colour (white in the dark theme), to stand apart from the
/// values beside it; not bold, which is the headings'.
pub(crate) fn field_label(ui: &mut egui::Ui, text: impl Into<String>) -> egui::Response {
    ui.label(egui::RichText::new(text).strong())
}

/// A section's heading ("PLT colors", "Model"): in Ubuntu Bold (headings
/// alone are bold), the strong text colour, and larger than the field
/// labels under it; under it a rule a pixel thick, three quarters as wide
/// as the pane it is in, then a little room before what follows.
pub(crate) fn section_heading(ui: &mut egui::Ui, text: impl Into<String>) -> egui::Response {
    let bold = egui::FontFamily::Name(BOLD.into());
    let size = egui::TextStyle::Body.resolve(ui.style()).size * HEADING_SCALE;
    let mut text = egui::RichText::new(text).strong().size(size);
    // (Until the fonts are in, the first frame, the strong colour alone.)
    if ui.fonts(|f| f.families().contains(&bold)) {
        text = text.family(bold);
    }
    let heading = ui.label(text);
    // (A pane with no width of its own yet: the heading's.)
    let pane = ui.available_width();
    let width = if pane.is_finite() { pane * HEADING_RULE } else { heading.rect.width() };
    let (rule, _) = ui.allocate_exact_size(egui::vec2(width, 1.0), egui::Sense::hover());
    let stroke = egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color);
    ui.painter().hline(rule.x_range(), rule.center().y, stroke);
    ui.add_space(HEADING_ROOM);
    heading
}

/// A section's heading after other content: room above it (its rule is
/// under it; no separator above).
pub(crate) fn next_section(ui: &mut egui::Ui, text: impl Into<String>) -> egui::Response {
    ui.add_space(SECTION_GAP);
    section_heading(ui, text)
}

/// The room above a section heading that follows other content.
const SECTION_GAP: f32 = 10.0;
/// How much larger than the body text a section heading is.
const HEADING_SCALE: f32 = 1.25;
/// How much of its pane's width a section heading's rule spans.
const HEADING_RULE: f32 = 0.75;
/// The room between a section heading's rule and what follows.
const HEADING_ROOM: f32 = 6.0;
