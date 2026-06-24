use crate::ui::Selection;
use egui::{Align, Layout, Response, Widget};
use egui_extras::{Column, TableBuilder};
use skynav::math::{DVec3, equatorial_radec};
use skynav::{Body, Simulation};

/// Full-width table of every body's distances and sky position. Click a row to
/// select it; the selection is shared with the Info, Sky and System views.
pub struct BodiesPanel<'a> {
    sim: &'a Simulation,
    selection: &'a mut Option<Selection>,
}

impl<'a> BodiesPanel<'a> {
    pub fn new(sim: &'a Simulation, selection: &'a mut Option<Selection>) -> Self {
        Self { sim, selection }
    }
}

impl Widget for BodiesPanel<'_> {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        ui.scope(|ui| {
            TableBuilder::new(ui)
                .striped(true)
                .sense(egui::Sense::click())
                .cell_layout(Layout::left_to_right(Align::Center))
                .column(Column::auto().at_least(64.0))
                .columns(Column::remainder().at_least(70.0), 4)
                .header(22.0, |mut header| {
                    head(
                        &mut header,
                        "Body",
                        "Celestial body - click a row to inspect it.",
                    );
                    head(&mut header, "Helio", "Distance from the Sun, in AU.");
                    head(&mut header, "Geo", "Distance from Earth, in AU.");
                    head(
                        &mut header,
                        "RA / Dec",
                        "Geocentric equatorial coordinates (J2000).",
                    );
                    head(
                        &mut header,
                        "Az / Alt",
                        "Observed azimuth and altitude (↓ = below horizon).",
                    );
                })
                .body(|mut body| {
                    for object in Body::ALL {
                        let selected = *self.selection == Some(Selection::Body(object));
                        body.row(20.0, |mut row| {
                            row.set_selected(selected);
                            row.col(|ui| {
                                ui.label(object.name());
                            });
                            row.col(|ui| {
                                ui.label(format!("{:.4}", self.sim.heliocentric(object).length()));
                            });
                            row.col(|ui| {
                                ui.label(format!("{:.4}", self.sim.geocentric(object).length()));
                            });
                            row.col(|ui| {
                                let (ra_h, dec) = ra_dec(self.sim.geocentric_equatorial(object));
                                ui.label(format!("{ra_h:.2}h {dec:+.1}°"));
                            });
                            row.col(|ui| match self.sim.observed_body(object) {
                                Some(h) => {
                                    let below = if h.altitude < 0.0 { " ↓" } else { "" };
                                    ui.label(format!(
                                        "{:.0}° {:+.1}°{below}",
                                        h.azimuth_deg(),
                                        h.altitude_deg()
                                    ));
                                }
                                None => {
                                    ui.label("-");
                                }
                            });
                            if row.response().clicked() {
                                *self.selection = if selected {
                                    None
                                } else {
                                    Some(Selection::Body(object))
                                };
                            }
                        });
                    }
                });
        })
        .response
    }
}

fn head(header: &mut egui_extras::TableRow, label: &str, tip: &str) {
    header.col(|ui| {
        ui.strong(label).on_hover_text(tip);
    });
}

fn ra_dec(v: DVec3) -> (f64, f64) {
    let (ra, dec) = equatorial_radec(v);
    (ra.to_degrees() / 15.0, dec.to_degrees())
}
