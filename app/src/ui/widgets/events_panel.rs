use egui::{Align, Layout, Response, RichText, Widget};
use egui_extras::{Column, TableBuilder};
use skynav::{Epoch, Simulation};

/// Sunrise/sunset and twilight times for the observer over the current UTC day.
pub struct EventsPanel<'a> {
    sim: &'a Simulation,
}

impl<'a> EventsPanel<'a> {
    pub fn new(sim: &'a Simulation) -> Self {
        Self { sim }
    }
}

impl Widget for EventsPanel<'_> {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        let ev = crate::ui::cache::sun_day(ui, self.sim);
        ui.scope(|ui| {
            ui.label(RichText::new("Sun events - current UTC day").strong())
                .on_hover_text("Times the Sun crosses each altitude threshold today (UTC).");
            ui.add_space(4.0);

            let rows = [
                (
                    "Astronomical dawn",
                    ev.astronomical_dawn,
                    "Sun reaches -18° - sky begins to brighten.",
                ),
                (
                    "Nautical dawn",
                    ev.nautical_dawn,
                    "Sun reaches -12° - horizon becomes visible at sea.",
                ),
                (
                    "Civil dawn",
                    ev.civil_dawn,
                    "Sun reaches -6° - bright enough for outdoor activity.",
                ),
                (
                    "Sunrise",
                    ev.sunrise,
                    "Sun's upper limb reaches the horizon (-0.833°).",
                ),
                (
                    "Sunset",
                    ev.sunset,
                    "Sun's upper limb drops below the horizon.",
                ),
                ("Civil dusk", ev.civil_dusk, "Sun falls to -6°."),
                ("Nautical dusk", ev.nautical_dusk, "Sun falls to -12°."),
                (
                    "Astronomical dusk",
                    ev.astronomical_dusk,
                    "Sun falls to -18° - full darkness.",
                ),
            ];

            TableBuilder::new(ui)
                .striped(true)
                .cell_layout(Layout::left_to_right(Align::Center))
                .column(Column::remainder().at_least(120.0))
                .column(Column::auto().at_least(72.0))
                .body(|mut body| {
                    for (label, time, tip) in rows {
                        body.row(20.0, |mut row| {
                            row.col(|ui| {
                                ui.label(label).on_hover_text(tip);
                            });
                            row.col(|ui| {
                                ui.label(format_time(time));
                            });
                        });
                    }
                });
        })
        .response
    }
}

fn format_time(time: Option<Epoch>) -> String {
    match time {
        Some(e) => {
            let (_, _, _, h, mi, s, _) = e.to_gregorian_utc();
            format!("{h:02}:{mi:02}:{s:02}")
        }
        None => "-".to_string(),
    }
}
