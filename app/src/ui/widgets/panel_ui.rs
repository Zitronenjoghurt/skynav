//! Small shared building blocks for the docked side panels, so they read as
//! proper full-width info tables (key flush left, value flush right) with tidy
//! section headers instead of content hugging the left edge.

use egui::{Align, Color32, Layout, Response, RichText, Ui};

const KEY_COLOR: Color32 = Color32::from_rgb(150, 162, 184);
const SECTION_COLOR: Color32 = Color32::from_rgb(150, 190, 255);

/// A section header: a coloured strong label with a little space above and a
/// thin separator below.
pub fn section(ui: &mut Ui, title: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(title).strong().color(SECTION_COLOR));
    ui.separator();
}

/// A full-width key/value row: muted key on the left, value pushed flush to the
/// right edge. Returns the value's `Response` so the caller can add hover text.
pub fn kv(ui: &mut Ui, key: &str, value: &str) -> Response {
    ui.horizontal(|ui| {
        ui.label(RichText::new(key).color(KEY_COLOR));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.monospace(value)
        })
        .inner
    })
    .inner
}

/// As `kv`, but the value is coloured (e.g. a body's tint or a status colour).
pub fn kv_colored(ui: &mut Ui, key: &str, value: &str, color: Color32) -> Response {
    ui.horizontal(|ui| {
        ui.label(RichText::new(key).color(KEY_COLOR));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new(value).monospace().color(color))
        })
        .inner
    })
    .inner
}

/// A full-width row whose right-hand side is built by `add` (for editors such as
/// a `DragValue` or a `ComboBox`), so the control sits flush right and the row
/// fills the panel width.
pub fn field(ui: &mut Ui, key: &str, add: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(key).color(KEY_COLOR));
        ui.with_layout(Layout::right_to_left(Align::Center), add);
    });
}
