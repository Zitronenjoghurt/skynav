use crate::ui::cache;
use crate::ui::icons;
use crate::util::now_epoch;
use egui::{
    Align2, Color32, CornerRadius, DragValue, FontId, Response, Sense, Slider, Stroke, StrokeKind,
    Widget, vec2,
};
use egui_extras::DatePickerButton;
use hifitime::Duration;
use jiff::civil::Date;
use skynav::math::equatorial_radec;
use skynav::{Body, Epoch, Simulation};
use std::f64::consts::{PI, TAU};

const DAY_SECONDS: f64 = 86_400.0;

// Sun-altitude thresholds (degrees) for each shading band, matching `events`.
const SUNRISE_ALT: f64 = -0.833;
const CIVIL_ALT: f64 = -6.0;
const NAUTICAL_ALT: f64 = -12.0;
const ASTRONOMICAL_ALT: f64 = -18.0;

/// Media-player-style time bar: transport controls on top, a scrubbable
/// day/night track below. Sits in a bottom panel.
pub struct Scrubber<'a> {
    sim: &'a mut Simulation,
}

impl<'a> Scrubber<'a> {
    pub fn new(sim: &'a mut Simulation) -> Self {
        Self { sim }
    }
}

impl Widget for Scrubber<'_> {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        let events = cache::sun_day(ui, self.sim);
        let (lst, solar) = local_times(self.sim);
        ui.vertical(|ui| {
            // One sleek control row, folding in everything the old Time tab did:
            // transport, a real calendar date picker + UTC time editor, the
            // Solar/LST readout, and the time rate (fast-forward / rewind).
            ui.horizontal(|ui| {
                let clock = &mut self.sim.clock;
                let play = if clock.playing {
                    icons::PAUSE
                } else {
                    icons::PLAY
                };
                if ui.button(play).on_hover_text("Play / pause time").clicked() {
                    clock.playing = !clock.playing;
                }
                for (label, secs, tip) in STEPS {
                    if ui.small_button(*label).on_hover_text(*tip).clicked() {
                        clock.epoch += Duration::from_seconds(*secs);
                    }
                }
                if ui
                    .button(format!("{} Now", icons::CLOCK_COUNTER_CLOCKWISE))
                    .on_hover_text("Jump to the real current time")
                    .clicked()
                {
                    clock.epoch = now_epoch();
                }
                ui.separator();
                ui.strong(weekday(clock.epoch));
                date_time_controls(ui, &mut clock.epoch);
                ui.separator();
                ui.monospace(format!("Solar {solar}  LST {lst}"))
                    .on_hover_text(
                        "Local apparent solar time and local sidereal time at the observer.",
                    );
                ui.separator();
                ui.label("Rate").on_hover_text(
                    "Simulated seconds per real second. Drag for fast-forward or rewind.",
                );
                ui.add(
                    Slider::new(&mut clock.rate, -31_557_600.0..=31_557_600.0)
                        .logarithmic(true)
                        .smallest_positive(1.0)
                        .custom_formatter(|v, _| format_rate(v)),
                );
                for (label, rate) in RATE_PRESETS {
                    if ui.small_button(*label).clicked() {
                        clock.rate = *rate;
                    }
                }
            });
            ui.add_space(3.0);
            let start = day_start(self.sim.clock.epoch);
            let start_alt = self
                .sim
                .geometric_altitude_at(Body::Sun, start)
                .to_degrees();
            // The day track spans the full width of the bottom panel.
            let width = ui.available_width();
            track(ui, &mut self.sim.clock.epoch, &events, start_alt, width);
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

/// Format an angle (radians, 0..2pi) as a 24-hour HH:MM clock.
fn clock_string(angle: f64) -> String {
    let hours = angle.to_degrees() / 15.0;
    let h = hours.floor() as i32;
    let m = ((hours - h as f64) * 60.0).floor() as i32;
    format!("{h:02}:{m:02}")
}

const RATE_PRESETS: &[(&str, f64)] = &[
    ("1x", 1.0),
    ("1min/s", 60.0),
    ("1h/s", 3_600.0),
    ("1d/s", 86_400.0),
    ("1yr/s", 31_557_600.0),
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

const STEPS: &[(&str, f64, &str)] = &[
    ("-1d", -DAY_SECONDS, "Back one day"),
    ("-1h", -3_600.0, "Back one hour"),
    ("+1h", 3_600.0, "Forward one hour"),
    ("+1d", DAY_SECONDS, "Forward one day"),
];

fn weekday(epoch: Epoch) -> &'static str {
    // Days since 2000-01-01 (a Saturday) modulo 7.
    let days = (epoch.to_jde_utc_days() - 2_451_544.5).floor() as i64;
    const NAMES: [&str; 7] = ["Sat", "Sun", "Mon", "Tue", "Wed", "Thu", "Fri"];
    NAMES[days.rem_euclid(7) as usize]
}

/// A calendar date picker plus UTC hour/minute editors that together set the
/// clock to any instant - the editing half of the old Time tab, inline.
fn date_time_controls(ui: &mut egui::Ui, epoch: &mut Epoch) {
    let (y, mo, d, h, mi, s, _) = epoch.to_gregorian_utc();
    let mut date =
        Date::new(y as i16, mo as i8, d as i8).unwrap_or_else(|_| Date::new(2000, 1, 1).unwrap());
    let mut hour = h as i32;
    let mut minute = mi as i32;

    let mut changed = ui
        .add(DatePickerButton::new(&mut date).id_salt("scrubber_date"))
        .on_hover_text("Pick the UTC date from a calendar.")
        .changed();
    changed |= ui
        .add(DragValue::new(&mut hour).range(0..=23).suffix("h"))
        .on_hover_text("Hour (UTC).")
        .changed();
    changed |= ui
        .add(DragValue::new(&mut minute).range(0..=59).suffix("m"))
        .on_hover_text("Minute (UTC).")
        .changed();

    if changed {
        *epoch = Epoch::from_gregorian_utc(
            date.year() as i32,
            date.month() as u8,
            date.day() as u8,
            hour as u8,
            minute as u8,
            s,
            0,
        );
    }
}

/// Day track: layered night/twilight/day shading, hour ticks, sunrise/sunset
/// markers and a draggable handle. Rendered at the given `width`.
fn track(
    ui: &mut egui::Ui,
    epoch: &mut Epoch,
    events: &skynav::DayEvents,
    start_alt_deg: f64,
    width: f32,
) {
    let width = width.max(160.0);
    let (rect, response) = ui.allocate_exact_size(vec2(width, 32.0), Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    let start = day_start(*epoch);
    let frac_of = |e: Option<Epoch>| e.map(|e| ((e - start).to_seconds() / DAY_SECONDS) as f32);

    // Layered bands, darkest first: night base then progressively brighter
    // twilight, then full daylight nested inside.
    painter.rect_filled(rect, 5.0, Color32::from_rgb(12, 14, 26));
    let bands = [
        (
            events.astronomical_dawn,
            events.astronomical_dusk,
            ASTRONOMICAL_ALT,
            Color32::from_rgb(22, 27, 48),
        ),
        (
            events.nautical_dawn,
            events.nautical_dusk,
            NAUTICAL_ALT,
            Color32::from_rgb(34, 44, 74),
        ),
        (
            events.civil_dawn,
            events.civil_dusk,
            CIVIL_ALT,
            Color32::from_rgb(52, 70, 110),
        ),
        (
            events.sunrise,
            events.sunset,
            SUNRISE_ALT,
            Color32::from_rgb(92, 134, 196),
        ),
    ];
    for (dawn, dusk, threshold, color) in bands {
        // When the rise crossing falls after the set crossing the lit span wraps
        // across UTC midnight; a missing crossing means the Sun stays on one side
        // of the threshold all day (polar day/night).
        for (a, b) in lit_intervals(frac_of(dawn), frac_of(dusk), start_alt_deg >= threshold) {
            band(&painter, rect, a, b, color);
        }
    }

    // Hour ticks every three hours, labelled at the quarters.
    for hour in (0..=24).step_by(3) {
        let x = rect.left() + (hour as f32 / 24.0) * rect.width();
        painter.line_segment(
            [
                egui::pos2(x, rect.bottom() - 5.0),
                egui::pos2(x, rect.bottom()),
            ],
            Stroke::new(1.0, Color32::from_rgb(90, 100, 120)),
        );
        if hour % 6 == 0 && hour != 0 && hour != 24 {
            painter.text(
                egui::pos2(x, rect.bottom() - 6.0),
                Align2::CENTER_BOTTOM,
                format!("{hour:02}"),
                FontId::proportional(9.0),
                Color32::from_rgb(150, 160, 180),
            );
        }
    }

    // Sunrise / sunset markers.
    for (time, up) in [(events.sunrise, true), (events.sunset, false)] {
        if let Some(f) = frac_of(time) {
            let x = rect.left() + f.clamp(0.0, 1.0) * rect.width();
            let color = if up {
                Color32::from_rgb(255, 214, 130)
            } else {
                Color32::from_rgb(255, 150, 90)
            };
            painter.circle_filled(egui::pos2(x, rect.top() + 6.0), 3.0, color);
        }
    }

    painter.rect_stroke(
        rect,
        5.0,
        Stroke::new(1.0, Color32::from_rgb(60, 72, 98)),
        StrokeKind::Inside,
    );

    // Current-time handle.
    let now = ((*epoch - start).to_seconds() / DAY_SECONDS).clamp(0.0, 1.0) as f32;
    let x = rect.left() + now * rect.width();
    painter.line_segment(
        [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
        Stroke::new(2.0, Color32::from_rgb(255, 236, 150)),
    );
    painter.circle_filled(
        egui::pos2(x, rect.top() + 3.0),
        4.0,
        Color32::from_rgb(255, 236, 150),
    );

    // Hover readout of the time under the cursor.
    if let Some(p) = response.hover_pos() {
        let frac = ((p.x - rect.left()) / rect.width()).clamp(0.0, 1.0) as f64;
        let (_, _, _, h, mi, _, _) =
            (start + Duration::from_seconds(frac * DAY_SECONDS)).to_gregorian_utc();
        painter.text(
            egui::pos2(p.x, rect.top() - 1.0),
            Align2::CENTER_BOTTOM,
            format!("{h:02}:{mi:02}"),
            FontId::monospace(10.0),
            Color32::from_rgb(230, 235, 245),
        );
    }

    if (response.dragged() || response.clicked())
        && let Some(p) = response.interact_pointer_pos()
    {
        let frac = ((p.x - rect.left()) / rect.width()).clamp(0.0, 1.0) as f64;
        *epoch = start + Duration::from_seconds(frac * DAY_SECONDS);
    }
    response.on_hover_text("Drag to scrub through the day");
}

/// Day-fraction spans the Sun spends above a threshold, given the rise/set
/// crossings and whether it starts the UTC day already above it.
fn lit_intervals(dawn: Option<f32>, dusk: Option<f32>, lit_at_start: bool) -> Vec<(f32, f32)> {
    match (dawn, dusk) {
        (Some(a), Some(b)) if a <= b => vec![(a, b)],
        (Some(a), Some(b)) => vec![(0.0, b), (a, 1.0)],
        (Some(a), None) => vec![(a, 1.0)],
        (None, Some(b)) => vec![(0.0, b)],
        (None, None) if lit_at_start => vec![(0.0, 1.0)],
        (None, None) => Vec::new(),
    }
}

fn band(painter: &egui::Painter, rect: egui::Rect, from: f32, to: f32, color: Color32) {
    let x0 = rect.left() + from.clamp(0.0, 1.0) * rect.width();
    let x1 = rect.left() + to.clamp(0.0, 1.0) * rect.width();
    if x1 > x0 {
        let band =
            egui::Rect::from_min_max(egui::pos2(x0, rect.top()), egui::pos2(x1, rect.bottom()));
        painter.rect_filled(band, CornerRadius::ZERO, color);
    }
}

fn day_start(epoch: Epoch) -> Epoch {
    let (y, m, d, _, _, _, _) = epoch.to_gregorian_utc();
    Epoch::from_gregorian_utc(y, m, d, 0, 0, 0, 0)
}
