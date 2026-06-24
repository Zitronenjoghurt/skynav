use crate::ui::icons;
use crate::util::now_epoch;
use egui::{DragValue, Grid, Response, RichText, Slider, Widget};
use skynav::math::equatorial_radec;
use skynav::{Body, Epoch, Simulation};
use std::f64::consts::{PI, TAU};

/// Clock readout, play/pause and time-rate (fast-forward) controls.
pub struct TimePanel<'a> {
    sim: &'a mut Simulation,
}

impl<'a> TimePanel<'a> {
    pub fn new(sim: &'a mut Simulation) -> Self {
        Self { sim }
    }
}

impl Widget for TimePanel<'_> {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        let (lst, solar) = local_times(self.sim);
        let clock = &mut self.sim.clock;
        ui.scope(|ui| {
            ui.set_min_width(ui.available_width());
            ui.label(RichText::new(format!("{}", clock.epoch)).monospace())
                .on_hover_text("Current simulated instant (UTC).");
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("Solar {solar}")).monospace())
                    .on_hover_text(
                        "Local apparent solar time at the observer (Sun on the meridian = 12:00).",
                    );
                ui.separator();
                ui.label(RichText::new(format!("Sidereal {lst}")).monospace())
                    .on_hover_text(
                        "Local sidereal time: the right ascension currently on the meridian.",
                    );
            });
            ui.separator();

            ui.horizontal(|ui| {
                let label = if clock.playing {
                    format!("{} Pause", icons::PAUSE)
                } else {
                    format!("{} Play", icons::PLAY)
                };
                if ui
                    .button(label)
                    .on_hover_text("Start or stop advancing time.")
                    .clicked()
                {
                    clock.playing = !clock.playing;
                }
                if ui
                    .button(format!("{} Now", icons::CLOCK_COUNTER_CLOCKWISE))
                    .on_hover_text("Jump to the real current time.")
                    .clicked()
                {
                    clock.epoch = now_epoch();
                }
                if ui
                    .button(format!("{} Real time", icons::GAUGE))
                    .on_hover_text("Reset the rate to 1× (wall-clock speed).")
                    .clicked()
                {
                    clock.rate = 1.0;
                }
            });

            ui.add_space(4.0);
            date_editor(ui, &mut clock.epoch);
            ui.separator();
            ui.label("Time rate").on_hover_text(
                "Simulated seconds elapsed per real second. Drag for fast-forward or rewind.",
            );
            ui.add(
                Slider::new(&mut clock.rate, -31_557_600.0..=31_557_600.0)
                    .logarithmic(true)
                    .smallest_positive(1.0)
                    .custom_formatter(|v, _| format_rate(v)),
            );

            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                for (label, rate) in RATE_PRESETS {
                    if ui.small_button(*label).clicked() {
                        clock.rate = *rate;
                    }
                }
            });
        })
        .response
    }
}

/// Local sidereal time and local apparent solar time, formatted HH:MM.
fn local_times(sim: &Simulation) -> (String, String) {
    let lst = (sim.earth_rotation_angle() + sim.observer.longitude_rad()).rem_euclid(TAU);
    let (sun_ra, _) = equatorial_radec(sim.geocentric_equatorial(Body::Sun));
    let solar = (lst - sun_ra + PI).rem_euclid(TAU);
    (clock_string(lst), clock_string(solar))
}

/// Format an angle (radians, 0..2π) as a 24-hour HH:MM clock.
fn clock_string(angle: f64) -> String {
    let hours = angle.to_degrees() / 15.0;
    let h = hours.floor() as i32;
    let m = ((hours - h as f64) * 60.0).floor() as i32;
    format!("{h:02}:{m:02}")
}

fn date_editor(ui: &mut egui::Ui, epoch: &mut Epoch) {
    let (y, mo, d, h, mi, s, _ns) = epoch.to_gregorian_utc();
    let mut year = y;
    let mut month = mo as i32;
    let mut day = d as i32;
    let mut hour = h as i32;
    let mut minute = mi as i32;
    let mut changed = false;

    Grid::new("datetime_grid").num_columns(2).show(ui, |ui| {
        ui.label("Date (UTC)");
        ui.horizontal(|ui| {
            changed |= ui
                .add(DragValue::new(&mut year).range(-4000..=9000))
                .changed();
            changed |= ui
                .add(DragValue::new(&mut month).range(1..=12).prefix("M"))
                .changed();
            changed |= ui
                .add(DragValue::new(&mut day).range(1..=31).prefix("D"))
                .changed();
        });
        ui.end_row();

        ui.label("Time (UTC)");
        ui.horizontal(|ui| {
            changed |= ui
                .add(DragValue::new(&mut hour).range(0..=23).suffix("h"))
                .changed();
            changed |= ui
                .add(DragValue::new(&mut minute).range(0..=59).suffix("m"))
                .changed();
        });
        ui.end_row();
    });

    if changed {
        let day = day.clamp(1, days_in_month(year, month));
        *epoch =
            Epoch::from_gregorian_utc(year, month as u8, day as u8, hour as u8, minute as u8, s, 0);
    }
}

fn days_in_month(year: i32, month: i32) -> i32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        _ => 30,
    }
}

const RATE_PRESETS: &[(&str, f64)] = &[
    ("1×", 1.0),
    ("1 min/s", 60.0),
    ("1 h/s", 3_600.0),
    ("1 d/s", 86_400.0),
    ("30 d/s", 2_592_000.0),
    ("1 yr/s", 31_557_600.0),
];

fn format_rate(v: f64) -> String {
    let a = v.abs();
    if a < 60.0 {
        format!("{v:.0} s/s")
    } else if a < 3_600.0 {
        format!("{:.1} min/s", v / 60.0)
    } else if a < 86_400.0 {
        format!("{:.1} h/s", v / 3_600.0)
    } else if a < 31_557_600.0 {
        format!("{:.1} d/s", v / 86_400.0)
    } else {
        format!("{:.2} yr/s", v / 31_557_600.0)
    }
}
