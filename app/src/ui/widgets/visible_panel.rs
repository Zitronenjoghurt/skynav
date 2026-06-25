use crate::ui::{Observed, Selection, icons};
use egui::{Align, Color32, Layout, Response, RichText, Widget};
use egui_extras::{Column, TableBuilder};
use glam::Vec3;
use skynav::{Body, Simulation, Star};

/// Brightest magnitude of named star included in the "now visible" list.
const STAR_MAG_LIMIT: f32 = 3.0;

/// Everything currently above the observer's horizon (bodies + bright named
/// stars), sorted by altitude. Click a row to select it.
pub struct VisiblePanel<'a> {
    sim: &'a Simulation,
    stars: &'a [Star],
    selection: &'a mut Option<Selection>,
    observed: &'a Observed,
}

impl<'a> VisiblePanel<'a> {
    pub fn new(
        sim: &'a Simulation,
        stars: &'a [Star],
        selection: &'a mut Option<Selection>,
        observed: &'a Observed,
    ) -> Self {
        Self {
            sim,
            stars,
            selection,
            observed,
        }
    }
}

struct Entry {
    altitude: f32,
    azimuth: f32,
    label: String,
    color: Color32,
    selection: Selection,
}

impl Widget for VisiblePanel<'_> {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        let mut entries = self.collect();
        entries.sort_by(|a, b| b.altitude.total_cmp(&a.altitude));

        ui.scope(|ui| {
            let suffix = if self.sim.view.enabled {
                "  (within your viewing area)"
            } else {
                ""
            };
            ui.label(
                RichText::new(format!("Above the horizon ({}){suffix}", entries.len())).strong(),
            )
            .on_hover_text("Bodies and bright named stars currently up, highest first.");
            ui.add_space(4.0);

            if entries.is_empty() {
                ui.weak("Nothing above the horizon right now.");
                return;
            }

            TableBuilder::new(ui)
                .striped(true)
                .sense(egui::Sense::click())
                .cell_layout(Layout::left_to_right(Align::Center))
                .column(Column::remainder().at_least(90.0))
                .column(Column::auto().at_least(54.0))
                .column(Column::auto().at_least(54.0))
                .header(20.0, |mut header| {
                    head(
                        &mut header,
                        "Object",
                        "Click to inspect and aim the Sky view.",
                    );
                    head(&mut header, "Alt", "Altitude above the horizon.");
                    head(
                        &mut header,
                        "Az",
                        "Azimuth from North, increasing eastward.",
                    );
                })
                .body(|mut body| {
                    for entry in &entries {
                        let selected = *self.selection == Some(entry.selection);
                        body.row(20.0, |mut row| {
                            row.set_selected(selected);
                            row.col(|ui| {
                                if self.observed.is_observed(entry.selection, self.stars) {
                                    ui.colored_label(
                                        Color32::from_rgb(120, 215, 150),
                                        icons::CHECK_CIRCLE,
                                    )
                                    .on_hover_text("You have observed this object.");
                                }
                                ui.colored_label(entry.color, &entry.label);
                            });
                            row.col(|ui| {
                                ui.label(format!("{:+.0}°", entry.altitude));
                            });
                            row.col(|ui| {
                                ui.label(format!("{:.0}°", entry.azimuth));
                            });
                            if row.response().clicked() {
                                *self.selection = if selected {
                                    None
                                } else {
                                    Some(entry.selection)
                                };
                            }
                        });
                    }
                });
        })
        .response
    }
}

impl VisiblePanel<'_> {
    fn collect(&self) -> Vec<Entry> {
        let mut entries = Vec::new();

        for body in Body::ALL {
            if body == Body::Earth {
                continue;
            }
            if let Some(h) = self.sim.observed_body(body)
                && h.altitude >= 0.0
                && self.sim.view.contains(h.azimuth_deg(), h.altitude_deg())
            {
                entries.push(Entry {
                    altitude: h.altitude_deg() as f32,
                    azimuth: h.azimuth_deg() as f32,
                    label: body.name().to_string(),
                    color: body_color(body),
                    selection: Selection::Body(body),
                });
            }
        }

        // Fast approximate altitude for stars via the one horizon matrix.
        let horizon = self.sim.equatorial_to_horizon().as_mat3();
        for (i, star) in self.stars.iter().enumerate() {
            if star.magnitude > STAR_MAG_LIMIT || star.name.is_empty() {
                continue;
            }
            let enu = horizon * Vec3::from(star.unit);
            if enu.z <= 0.0 {
                continue;
            }
            let altitude = enu.z.clamp(-1.0, 1.0).asin().to_degrees();
            let azimuth = enu.x.atan2(enu.y).to_degrees().rem_euclid(360.0);
            if !self
                .sim
                .view
                .star_visible(star.magnitude, azimuth as f64, altitude as f64)
            {
                continue;
            }
            entries.push(Entry {
                altitude,
                azimuth,
                label: star.name.clone(),
                color: Color32::from_rgb(190, 200, 225),
                selection: Selection::Star(i),
            });
        }

        entries
    }
}

fn body_color(body: Body) -> Color32 {
    match body {
        Body::Sun => Color32::from_rgb(255, 224, 130),
        Body::Moon => Color32::from_rgb(220, 224, 235),
        Body::Mars => Color32::from_rgb(240, 140, 100),
        _ => Color32::from_rgb(170, 200, 255),
    }
}

fn head(header: &mut egui_extras::TableRow, label: &str, tip: &str) {
    header.col(|ui| {
        ui.strong(label).on_hover_text(tip);
    });
}
