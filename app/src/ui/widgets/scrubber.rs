use crate::ui::cache;
use crate::ui::icons;
use crate::util::now_epoch;
use egui::{
    Align2, Color32, CornerRadius, FontId, Response, Sense, Stroke, StrokeKind, Widget, vec2,
};
use hifitime::Duration;
use skynav::{Body, Epoch, Simulation};

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
        ui.vertical(|ui| {
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
                ui.monospace(readout(clock.epoch))
                    .on_hover_text("Current simulated instant (UTC).");
            });
            ui.add_space(2.0);
            let start = day_start(self.sim.clock.epoch);
            let start_alt = self
                .sim
                .geometric_altitude_at(Body::Sun, start)
                .to_degrees();
            track(ui, &mut self.sim.clock.epoch, &events, start_alt);
        })
        .response
    }
}

const STEPS: &[(&str, f64, &str)] = &[
    ("-1d", -DAY_SECONDS, "Back one day"),
    ("-1h", -3_600.0, "Back one hour"),
    ("+1h", 3_600.0, "Forward one hour"),
    ("+1d", DAY_SECONDS, "Forward one day"),
];

/// "Thu 2026-06-25  14:30 UTC".
fn readout(epoch: Epoch) -> String {
    let (y, mo, d, h, mi, _, _) = epoch.to_gregorian_utc();
    format!(
        "{}  {y:04}-{mo:02}-{d:02}  {h:02}:{mi:02} UTC",
        weekday(epoch)
    )
}

fn weekday(epoch: Epoch) -> &'static str {
    // Days since 2000-01-01 (a Saturday) modulo 7.
    let days = (epoch.to_jde_utc_days() - 2_451_544.5).floor() as i64;
    const NAMES: [&str; 7] = ["Sat", "Sun", "Mon", "Tue", "Wed", "Thu", "Fri"];
    NAMES[days.rem_euclid(7) as usize]
}

/// Full-width day track: layered night/twilight/day shading, hour ticks,
/// sunrise/sunset markers and a draggable handle.
fn track(ui: &mut egui::Ui, epoch: &mut Epoch, events: &skynav::DayEvents, start_alt_deg: f64) {
    let width = ui.available_width().max(160.0);
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
