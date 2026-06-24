use crate::ui::cache;
use crate::ui::icons;
use crate::util::now_epoch;
use egui::{Color32, CornerRadius, Response, Sense, Stroke, StrokeKind, Widget, vec2};
use hifitime::Duration;
use skynav::{Epoch, Simulation};

const DAY_SECONDS: f64 = 86_400.0;

/// Media-player-style time bar: play/pause, coarse stepping and a scrubbable
/// day/night track for the current UTC day. Sits in a bottom panel.
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

            // Day-of-date label on the right; the track fills what's left.
            let (y, mo, d, h, mi, _, _) = clock.epoch.to_gregorian_utc();
            let date = format!("{y:04}-{mo:02}-{d:02}  {h:02}:{mi:02} UTC");
            let reserve = 150.0;
            let width = (ui.available_width() - reserve).max(140.0);
            track(ui, &mut clock.epoch, &events, width);
            ui.monospace(date);
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

/// The scrubbable day track: night/twilight/day shading plus a draggable handle.
fn track(ui: &mut egui::Ui, epoch: &mut Epoch, events: &skynav::DayEvents, width: f32) {
    let (rect, response) = ui.allocate_exact_size(vec2(width, 26.0), Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    let start = day_start(*epoch);
    let frac_of = |e: Option<Epoch>| e.map(|e| ((e - start).to_seconds() / DAY_SECONDS) as f32);

    // Shading bands: night base, twilight, daylight.
    painter.rect_filled(rect, 4.0, Color32::from_rgb(10, 13, 24));
    if let (Some(dawn), Some(dusk)) = (
        frac_of(events.astronomical_dawn),
        frac_of(events.astronomical_dusk),
    ) {
        band(&painter, rect, dawn, dusk, Color32::from_rgb(24, 30, 52));
    }
    if let (Some(rise), Some(set)) = (frac_of(events.sunrise), frac_of(events.sunset)) {
        band(&painter, rect, rise, set, Color32::from_rgb(58, 84, 130));
    }
    painter.rect_stroke(
        rect,
        4.0,
        Stroke::new(1.0, Color32::from_rgb(50, 62, 88)),
        StrokeKind::Inside,
    );

    // Current time handle.
    let now = ((*epoch - start).to_seconds() / DAY_SECONDS).clamp(0.0, 1.0) as f32;
    let x = rect.left() + now * rect.width();
    painter.line_segment(
        [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
        Stroke::new(2.0, Color32::from_rgb(255, 236, 150)),
    );
    painter.circle_filled(
        egui::pos2(x, rect.top() + 3.0),
        3.5,
        Color32::from_rgb(255, 236, 150),
    );

    if (response.dragged() || response.clicked())
        && let Some(p) = response.interact_pointer_pos()
    {
        let frac = ((p.x - rect.left()) / rect.width()).clamp(0.0, 1.0) as f64;
        *epoch = start + Duration::from_seconds(frac * DAY_SECONDS);
    }
    response.on_hover_text("Drag to scrub through the day");
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
