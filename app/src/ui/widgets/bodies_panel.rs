use crate::ui::observed::body_key;
use crate::ui::{Observed, Selection, icons};
use egui::{Align, Color32, Id, Layout, Response, RichText, Widget};
use egui_extras::{Column, TableBuilder};
use skynav::math::{DVec3, equatorial_radec};
use skynav::{Body, Simulation};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SortKey {
    Name,
    Helio,
    Geo,
    Altitude,
}

/// Full-width, sortable table of every body's distances and sky position. Click
/// a row to select it; click a header to sort. The selection is shared with the
/// Info, Sky and System views.
pub struct BodiesPanel<'a> {
    sim: &'a Simulation,
    selection: &'a mut Option<Selection>,
    observed: &'a Observed,
}

impl<'a> BodiesPanel<'a> {
    pub fn new(
        sim: &'a Simulation,
        selection: &'a mut Option<Selection>,
        observed: &'a Observed,
    ) -> Self {
        Self {
            sim,
            selection,
            observed,
        }
    }
}

struct Row {
    body: Body,
    helio: f64,
    /// Geocentric distance / RA-Dec are `None` for Earth itself (the origin of
    /// the geocentric frame, so they would be zero / undefined).
    geo: Option<f64>,
    radec: Option<(f64, f64)>,
    /// Az/Alt are `None` for the body you are standing on (no sky position).
    az: Option<f64>,
    alt: Option<f64>,
}

impl Widget for BodiesPanel<'_> {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        let sort_id = Id::new("bodies_sort");
        let (mut key, mut asc) = ui
            .data(|d| d.get_temp::<(SortKey, bool)>(sort_id))
            .unwrap_or((SortKey::Altitude, false));

        let mut rows: Vec<Row> = Body::ALL
            .iter()
            .map(|&body| {
                let is_earth = body == Body::Earth;
                let is_here = body == self.sim.observer_body;
                let observed = (!is_here).then(|| self.sim.observed_body(body)).flatten();
                Row {
                    body,
                    helio: self.sim.heliocentric(body).length(),
                    geo: (!is_earth).then(|| self.sim.geocentric(body).length()),
                    radec: (!is_earth).then(|| ra_dec(self.sim.geocentric_equatorial(body))),
                    az: observed.map(|h| h.azimuth_deg()),
                    alt: observed.map(|h| h.altitude_deg()),
                }
            })
            .collect();
        sort_rows(&mut rows, key, asc);

        let mut header_clicked: Option<SortKey> = None;
        ui.scope(|ui| {
            TableBuilder::new(ui)
                .striped(true)
                .sense(egui::Sense::click())
                .cell_layout(Layout::left_to_right(Align::Center))
                .column(Column::auto().at_least(76.0))
                .column(Column::remainder().at_least(58.0))
                .column(Column::remainder().at_least(58.0))
                .column(Column::remainder().at_least(78.0))
                .column(Column::remainder().at_least(58.0))
                .column(Column::remainder().at_least(58.0))
                .header(22.0, |mut header| {
                    sort_head(
                        &mut header,
                        "Body",
                        SortKey::Name,
                        key,
                        asc,
                        &mut header_clicked,
                    );
                    sort_head(
                        &mut header,
                        "Helio",
                        SortKey::Helio,
                        key,
                        asc,
                        &mut header_clicked,
                    );
                    sort_head(
                        &mut header,
                        "Geo",
                        SortKey::Geo,
                        key,
                        asc,
                        &mut header_clicked,
                    );
                    plain_head(&mut header, "RA / Dec", "Geocentric equatorial (J2000).");
                    plain_head(&mut header, "Az", "Azimuth from North, eastward.");
                    sort_head(
                        &mut header,
                        "Alt",
                        SortKey::Altitude,
                        key,
                        asc,
                        &mut header_clicked,
                    );
                })
                .body(|mut body| {
                    for r in &rows {
                        let selected = *self.selection == Some(Selection::Body(r.body));
                        body.row(20.0, |mut row| {
                            row.set_selected(selected);
                            row.col(|ui| {
                                ui.label(RichText::new(body_icon(r.body)).color(body_tint(r.body)));
                                ui.label(r.body.name());
                                if self.observed.contains(&body_key(r.body)) {
                                    ui.colored_label(
                                        Color32::from_rgb(120, 215, 150),
                                        icons::CHECK_CIRCLE,
                                    )
                                    .on_hover_text("Observed");
                                }
                            });
                            row.col(|ui| {
                                ui.label(format!("{:.4}", r.helio));
                            });
                            row.col(|ui| match r.geo {
                                Some(g) => {
                                    ui.label(format!("{g:.4}"));
                                }
                                None => {
                                    ui.label("-");
                                }
                            });
                            row.col(|ui| match r.radec {
                                Some((ra_h, dec)) => {
                                    ui.label(format!("{ra_h:.2}h {dec:+.1}°"));
                                }
                                None => {
                                    ui.label("-");
                                }
                            });
                            row.col(|ui| match r.az {
                                Some(az) => {
                                    ui.label(format!("{az:.0}°"));
                                }
                                None => {
                                    ui.label("-");
                                }
                            });
                            row.col(|ui| match r.alt {
                                Some(alt) if alt >= 0.0 => {
                                    ui.colored_label(
                                        Color32::from_rgb(150, 220, 160),
                                        format!("{} {alt:+.1}°", icons::EYE),
                                    );
                                }
                                Some(alt) => {
                                    ui.colored_label(
                                        Color32::from_rgb(130, 138, 152),
                                        format!("{alt:+.1}°"),
                                    );
                                }
                                None => {
                                    ui.label("-");
                                }
                            });
                            if row.response().clicked() {
                                *self.selection = if selected {
                                    None
                                } else {
                                    Some(Selection::Body(r.body))
                                };
                            }
                        });
                    }
                });
        });

        if let Some(clicked) = header_clicked {
            if clicked == key {
                asc = !asc;
            } else {
                key = clicked;
                asc = matches!(clicked, SortKey::Name);
            }
            ui.data_mut(|d| d.insert_temp(sort_id, (key, asc)));
        }

        ui.interact(ui.min_rect(), ui.id().with("bodies"), egui::Sense::hover())
    }
}

fn sort_rows(rows: &mut [Row], key: SortKey, asc: bool) {
    rows.sort_by(|a, b| {
        let ord = match key {
            SortKey::Name => a.body.name().cmp(b.body.name()),
            SortKey::Helio => a.helio.total_cmp(&b.helio),
            SortKey::Geo => a
                .geo
                .unwrap_or(f64::INFINITY)
                .total_cmp(&b.geo.unwrap_or(f64::INFINITY)),
            SortKey::Altitude => a
                .alt
                .unwrap_or(f64::NEG_INFINITY)
                .total_cmp(&b.alt.unwrap_or(f64::NEG_INFINITY)),
        };
        if asc { ord } else { ord.reverse() }
    });
}

fn sort_head(
    header: &mut egui_extras::TableRow,
    label: &str,
    this: SortKey,
    active: SortKey,
    asc: bool,
    clicked: &mut Option<SortKey>,
) {
    header.col(|ui| {
        let arrow = if this == active {
            if asc {
                icons::CARET_UP
            } else {
                icons::CARET_DOWN
            }
        } else {
            ""
        };
        if ui
            .add(egui::Button::new(RichText::new(format!("{label} {arrow}")).strong()).frame(false))
            .on_hover_text("Click to sort by this column.")
            .clicked()
        {
            *clicked = Some(this);
        }
    });
}

fn plain_head(header: &mut egui_extras::TableRow, label: &str, tip: &str) {
    header.col(|ui| {
        ui.strong(label).on_hover_text(tip);
    });
}

fn body_icon(body: Body) -> &'static str {
    match body {
        Body::Sun => icons::SUN,
        Body::Moon => icons::MOON,
        _ => icons::PLANET,
    }
}

fn body_tint(body: Body) -> Color32 {
    match body {
        Body::Sun => Color32::from_rgb(255, 214, 130),
        Body::Moon => Color32::from_rgb(210, 214, 226),
        Body::Mars => Color32::from_rgb(240, 140, 100),
        _ => Color32::from_rgb(170, 200, 255),
    }
}

fn ra_dec(v: DVec3) -> (f64, f64) {
    let (ra, dec) = equatorial_radec(v);
    (ra.to_degrees() / 15.0, dec.to_degrees())
}
