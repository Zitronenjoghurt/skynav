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
    magnitude: f32,
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
                .column(Column::auto().at_least(44.0))
                .column(Column::auto().at_least(86.0))
                .column(Column::auto().at_least(48.0))
                .header(20.0, |mut header| {
                    head(
                        &mut header,
                        "Object",
                        "Click to inspect and aim the Sky view.",
                    );
                    head(
                        &mut header,
                        "Mag",
                        "Apparent magnitude (lower is brighter; naked-eye limit ~6).",
                    );
                    head(&mut header, "Altitude", "Height above the horizon (0-90°).");
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
                                ui.label(format!("{:.1}", entry.magnitude));
                            });
                            row.col(|ui| {
                                altitude_bar(ui, entry.altitude);
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
            if body == self.sim.observer_body {
                continue;
            }
            if let Some(h) = self.sim.observed_body(body)
                && h.altitude >= 0.0
                && self.sim.view.contains(h.azimuth_deg(), h.altitude_deg())
            {
                entries.push(Entry {
                    altitude: h.altitude_deg() as f32,
                    azimuth: h.azimuth_deg() as f32,
                    magnitude: self.sim.apparent_magnitude(body) as f32,
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
                magnitude: star.magnitude,
                label: star.name.clone(),
                color: Color32::from_rgb(190, 200, 225),
                selection: Selection::Star(i),
            });
        }

        entries
    }
}

/// A small horizontal gauge for an object's altitude (0° horizon -> 90° zenith)
/// with the value overlaid, so the table shows at a glance what is high and what
/// is skimming the horizon.
fn altitude_bar(ui: &mut egui::Ui, altitude: f32) {
    let w = ui.available_width().max(40.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 14.0), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 3.0, Color32::from_rgb(28, 33, 46));
    let frac = (altitude / 90.0).clamp(0.0, 1.0);
    if frac > 0.0 {
        let fill =
            egui::Rect::from_min_size(rect.min, egui::vec2(rect.width() * frac, rect.height()));
        painter.rect_filled(fill, 3.0, altitude_color(altitude));
    }
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        format!("{altitude:+.0}°"),
        egui::FontId::proportional(11.0),
        Color32::from_rgb(235, 240, 248),
    );
}

/// Warm near the horizon (hard to observe) easing to cool blue high up.
fn altitude_color(altitude: f32) -> Color32 {
    let t = (altitude / 90.0).clamp(0.0, 1.0);
    let lerp = |a: f32, b: f32| (a + (b - a) * t) as u8;
    Color32::from_rgb(lerp(150.0, 70.0), lerp(95.0, 130.0), lerp(70.0, 210.0))
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
