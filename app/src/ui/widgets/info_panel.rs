use crate::ui::Selection;
use crate::util::humanize_until;
use egui::{Color32, Grid, Response, RichText, Widget};
use skynav::math::AU_KM;
use skynav::{Body, Epoch, Simulation, Star};

/// Detail card for the currently selected object (a Solar System body or a
/// catalogued star). Selection is shared with the Sky, System and Bodies views.
pub struct InfoPanel<'a> {
    sim: &'a Simulation,
    stars: &'a [Star],
    selection: Option<Selection>,
}

impl<'a> InfoPanel<'a> {
    pub fn new(sim: &'a Simulation, stars: &'a [Star], selection: Option<Selection>) -> Self {
        Self {
            sim,
            stars,
            selection,
        }
    }
}

impl Widget for InfoPanel<'_> {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        ui.scope(|ui| {
            ui.set_min_width(ui.available_width());
            match self.selection {
                Some(Selection::Body(body)) => self.body_info(ui, body),
                Some(Selection::Star(i)) if i < self.stars.len() => {
                    self.star_info(ui, &self.stars[i])
                }
                _ => {
                    ui.add_space(8.0);
                    ui.weak("Nothing selected.");
                    ui.add_space(2.0);
                    ui.weak("Click a body or star in the Sky or System view, or a row in the Bodies table.");
                }
            }
        })
        .response
    }
}

impl InfoPanel<'_> {
    fn body_info(&self, ui: &mut egui::Ui, body: Body) {
        heading(ui, body.name());
        let kind = if body == Body::Sun {
            "Star (the Sun)"
        } else if body == Body::Moon {
            "Natural satellite"
        } else {
            "Planet"
        };
        ui.weak(kind);
        ui.add_space(4.0);

        let geo_au = self.sim.geocentric(body).length();
        let helio_au = self.sim.heliocentric(body).length();
        let geo_km = geo_au * AU_KM;
        let radius_km = body.mean_radius_km();

        Grid::new("info_body").num_columns(2).show(ui, |ui| {
            row(
                ui,
                "Mean radius",
                &format!("{radius_km:.0} km"),
                "Mean volumetric radius.",
            );
            if body != Body::Sun {
                row(
                    ui,
                    "From Sun",
                    &format!("{helio_au:.4} AU"),
                    "Heliocentric distance in astronomical units.",
                );
            }
            row(
                ui,
                "From Earth",
                &format!("{geo_au:.4} AU"),
                "Geocentric distance (centre to centre).",
            );
            row(
                ui,
                "",
                &format!("{geo_km:.3e} km"),
                "Same distance in kilometres.",
            );
            if geo_km > 0.0 {
                let diameter = 2.0 * (radius_km / geo_km).atan();
                row(
                    ui,
                    "Angular size",
                    &format_angle(diameter),
                    "Apparent diameter as seen from Earth.",
                );
            }
            let period = body.orbital_period_days();
            if period > 0.0 {
                row(
                    ui,
                    "Orbital period",
                    &format_period(period),
                    "Sidereal period of one orbit.",
                );
            }
            if body == Body::Moon {
                let (frac, waxing) = self.sim.moon_illumination();
                row(
                    ui,
                    "Phase",
                    &format!(
                        "{} ({:.0}% lit)",
                        moon_phase_name(frac, waxing),
                        frac * 100.0
                    ),
                    "Illuminated fraction of the lunar disc.",
                );
            }
            let (ra_h, dec_deg) = ra_dec(self.sim.geocentric_equatorial(body));
            row(
                ui,
                "RA / Dec",
                &format!("{ra_h:.2}h  {dec_deg:+.2}°"),
                "Geocentric right ascension and declination (J2000).",
            );
            self.horizon_rows(ui, self.sim.observed_body(body));
        });

        let rs = crate::ui::cache::body_rise_set(ui, self.sim, body);
        self.rise_set_grid(ui, "info_riseset_body", rs);
    }

    fn rise_set_grid(&self, ui: &mut egui::Ui, id: &str, rs: skynav::RiseSet) {
        let now = self.sim.clock.epoch;
        ui.add_space(6.0);
        ui.label(RichText::new("Today (UTC)").strong())
            .on_hover_text("Rise, set and culmination over the current UTC day.");
        Grid::new(id).num_columns(2).show(ui, |ui| {
            row(
                ui,
                "Rise",
                &format_when(rs.rise, now),
                "When it climbs above the horizon.",
            );
            row(
                ui,
                "Transit",
                &format!(
                    "{} ({:+.1}°)",
                    format_when(rs.transit, now),
                    rs.transit_altitude
                ),
                "Highest point (crossing the meridian) and its altitude.",
            );
            row(
                ui,
                "Set",
                &format_when(rs.set, now),
                "When it drops below the horizon.",
            );
        });
    }

    fn star_info(&self, ui: &mut egui::Ui, star: &Star) {
        let name = if star.name.is_empty() {
            "Unnamed star"
        } else {
            &star.name
        };
        heading(ui, name);
        ui.horizontal(|ui| {
            ui.weak("Star");
            let c = star.color;
            let swatch = Color32::from_rgb(
                (c[0] * 255.0) as u8,
                (c[1] * 255.0) as u8,
                (c[2] * 255.0) as u8,
            );
            let (rect, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 2.0, swatch);
        })
        .response
        .on_hover_text("Approximate colour from the B-V index.");
        ui.add_space(4.0);

        Grid::new("info_star").num_columns(2).show(ui, |ui| {
            row(
                ui,
                "Magnitude",
                &format!("{:.2}", star.magnitude),
                "Apparent visual magnitude (lower is brighter).",
            );
            row(
                ui,
                "RA / Dec",
                &format!(
                    "{:.2}h  {:+.2}°",
                    star.ra.to_degrees() / 15.0,
                    star.dec.to_degrees()
                ),
                "Catalogue right ascension and declination (J2000).",
            );
            self.horizon_rows(ui, self.sim.observed_star(star));
        });

        let day = day_start(self.sim.clock.epoch);
        let rs = skynav::events::star_rise_set(&self.sim.observer, star.ra, star.dec, day);
        if rs.rise.is_none() && rs.set.is_none() {
            ui.add_space(6.0);
            let state = if rs.transit_altitude >= 0.0 {
                "Always above the horizon (circumpolar)."
            } else {
                "Never rises at this latitude."
            };
            ui.weak(state);
        } else {
            self.rise_set_grid(ui, "info_riseset_star", rs);
        }
    }

    fn horizon_rows(&self, ui: &mut egui::Ui, observed: Option<skynav::Horizontal>) {
        if let Some(h) = observed {
            let visible = if h.altitude >= 0.0 {
                "above horizon"
            } else {
                "below horizon"
            };
            row(
                ui,
                "Altitude",
                &format!("{:+.2}°  ({visible})", h.altitude_deg()),
                "Angle above the local horizon, with refraction.",
            );
            row(
                ui,
                "Azimuth",
                &format!("{:.2}°", h.azimuth_deg()),
                "Compass bearing measured from North, increasing eastward.",
            );
        }
    }
}

fn heading(ui: &mut egui::Ui, text: &str) {
    ui.label(
        RichText::new(text)
            .heading()
            .color(Color32::from_rgb(150, 190, 255)),
    );
}

fn row(ui: &mut egui::Ui, key: &str, value: &str, tip: &str) {
    if key.is_empty() {
        ui.label("");
    } else {
        ui.label(RichText::new(key).weak()).on_hover_text(tip);
    }
    ui.label(value).on_hover_text(tip);
    ui.end_row();
}

fn format_angle(rad: f64) -> String {
    let arcsec = rad.to_degrees() * 3600.0;
    if arcsec >= 60.0 {
        format!("{:.2} arcmin", arcsec / 60.0)
    } else {
        format!("{arcsec:.1} arcsec")
    }
}

fn format_when(time: Option<Epoch>, now: Epoch) -> String {
    match time {
        Some(e) => {
            let (_, _, _, h, mi, _, _) = e.to_gregorian_utc();
            format!("{h:02}:{mi:02} ({})", humanize_until(e, now))
        }
        None => "-".to_string(),
    }
}

fn day_start(epoch: Epoch) -> Epoch {
    let (y, m, d, _, _, _, _) = epoch.to_gregorian_utc();
    Epoch::from_gregorian_utc(y, m, d, 0, 0, 0, 0)
}

fn moon_phase_name(fraction: f64, waxing: bool) -> &'static str {
    if fraction < 0.04 {
        "New moon"
    } else if fraction > 0.96 {
        "Full moon"
    } else if (fraction - 0.5).abs() < 0.05 {
        if waxing {
            "First quarter"
        } else {
            "Last quarter"
        }
    } else if fraction < 0.5 {
        if waxing {
            "Waxing crescent"
        } else {
            "Waning crescent"
        }
    } else if waxing {
        "Waxing gibbous"
    } else {
        "Waning gibbous"
    }
}

fn format_period(days: f64) -> String {
    if days >= 365.25 {
        format!("{:.2} yr", days / 365.25)
    } else {
        format!("{days:.1} d")
    }
}

fn ra_dec(v: skynav::math::DVec3) -> (f64, f64) {
    let (ra, dec) = skynav::math::equatorial_radec(v);
    (ra.to_degrees() / 15.0, dec.to_degrees())
}
