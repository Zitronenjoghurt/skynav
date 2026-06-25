use crate::ui::icons;
use crate::util::humanize_until;
use egui::{Align, Color32, ComboBox, Layout, Response, RichText, Widget};
use egui_extras::{Column, TableBuilder};
use serde::{Deserialize, Serialize};
use skynav::{AstroEvent, EventCategory, Simulation};

/// Scan-window presets: (label, days before now, days after now).
const WINDOWS: &[(&str, f64, f64)] = &[
    ("+- 3 days", 3.0, 3.0),
    ("Next 30 days", 1.0, 30.0),
    ("+- 30 days", 30.0, 30.0),
    ("Next year", 1.0, 365.0),
    ("+- 1 year", 365.0, 365.0),
];

/// Persisted state for the Events panel: which categories show, the scan window
/// and whether to restrict to the mapped viewing area.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventsFilter {
    pub sun: bool,
    pub eclipse: bool,
    pub approach: bool,
    pub rise_set: bool,
    pub window: usize,
    pub only_visible: bool,
}

impl Default for EventsFilter {
    fn default() -> Self {
        Self {
            sun: true,
            eclipse: true,
            approach: true,
            rise_set: true,
            window: 1,
            only_visible: false,
        }
    }
}

impl EventsFilter {
    fn shows(&self, category: EventCategory) -> bool {
        match category {
            EventCategory::Sun => self.sun,
            EventCategory::Eclipse => self.eclipse,
            EventCategory::Approach => self.approach,
            EventCategory::RiseSet => self.rise_set,
        }
    }
}

/// Multi-type, past-and-future astronomical events for the observer.
pub struct EventsPanel<'a> {
    sim: &'a mut Simulation,
    filter: &'a mut EventsFilter,
}

impl<'a> EventsPanel<'a> {
    pub fn new(sim: &'a mut Simulation, filter: &'a mut EventsFilter) -> Self {
        Self { sim, filter }
    }
}

impl Widget for EventsPanel<'_> {
    fn ui(mut self, ui: &mut egui::Ui) -> Response {
        let idx = self.filter.window.min(WINDOWS.len() - 1);
        let (_, past, future) = WINDOWS[idx];
        let all = crate::ui::cache::scan_events(ui, self.sim, past, future);
        let now = self.sim.clock.epoch;
        let view_active = self.sim.view.enabled && self.filter.only_visible;

        let events: Vec<&AstroEvent> = all
            .iter()
            .filter(|e| self.filter.shows(e.category))
            .filter(|e| !view_active || self.in_view(e))
            .collect();
        // Highlight the event closest to the current instant (the one you just
        // jumped to stays highlighted, rather than the row flipping to the next).
        let nearest_idx = events
            .iter()
            .enumerate()
            .min_by(|a, b| {
                let da = (a.1.time - now).to_seconds().abs();
                let db = (b.1.time - now).to_seconds().abs();
                da.total_cmp(&db)
            })
            .map(|(i, _)| i);

        let mut jump: Option<skynav::Epoch> = None;
        let response = ui
            .scope(|ui| {
                ui.set_min_width(ui.available_width());
                self.controls(ui);
                ui.add_space(2.0);
                ui.label(RichText::new(format!("{} events", events.len())).weak());
                ui.add_space(2.0);

                if events.is_empty() {
                    ui.weak("No matching events in this window.");
                    return;
                }

                TableBuilder::new(ui)
                    .striped(true)
                    .sense(egui::Sense::click())
                    .cell_layout(Layout::left_to_right(Align::Center))
                    .column(Column::auto())
                    .column(Column::remainder().at_least(150.0))
                    .column(Column::auto().at_least(74.0))
                    .header(20.0, |mut header| {
                        head(&mut header, "", "Event category.");
                        head(&mut header, "Event", "Click a row to jump the clock to it.");
                        head(&mut header, "When", "Time relative to the current instant.");
                    })
                    .body(|mut body| {
                        for (i, e) in events.iter().enumerate() {
                            let current = nearest_idx == Some(i);
                            body.row(22.0, |mut row| {
                                row.set_selected(current);
                                row.col(|ui| {
                                    ui.label(category_icon(e.category));
                                });
                                row.col(|ui| {
                                    let title = RichText::new(&e.title);
                                    let title = if current { title.strong() } else { title };
                                    ui.add(egui::Label::new(title).truncate())
                                        .on_hover_text(detail_text(e));
                                });
                                row.col(|ui| {
                                    let color = if current {
                                        Color32::from_rgb(150, 210, 255)
                                    } else {
                                        Color32::from_rgb(180, 188, 205)
                                    };
                                    ui.colored_label(color, humanize_until(e.time, now));
                                });
                                if row.response().on_hover_text(detail_text(e)).clicked() {
                                    jump = Some(e.time);
                                }
                            });
                        }
                    });
            })
            .response;

        if let Some(t) = jump {
            self.sim.clock.epoch = t;
        }
        response
    }
}

impl EventsPanel<'_> {
    fn in_view(&self, e: &AstroEvent) -> bool {
        match e.body {
            Some(body) if body != skynav::Body::Earth => {
                let (az, alt) = self.sim.horizontal_at(body, e.time);
                self.sim.view.contains(az, alt)
            }
            _ => true,
        }
    }

    fn controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            chip(ui, &mut self.filter.sun, EventCategory::Sun);
            chip(ui, &mut self.filter.eclipse, EventCategory::Eclipse);
            chip(ui, &mut self.filter.approach, EventCategory::Approach);
            chip(ui, &mut self.filter.rise_set, EventCategory::RiseSet);
        });
        ui.horizontal(|ui| {
            let idx = self.filter.window.min(WINDOWS.len() - 1);
            ComboBox::from_id_salt("events_window")
                .selected_text(WINDOWS[idx].0)
                .show_ui(ui, |ui| {
                    for (i, (label, _, _)) in WINDOWS.iter().enumerate() {
                        ui.selectable_value(&mut self.filter.window, i, *label);
                    }
                });
            ui.add_enabled(
                self.sim.view.enabled,
                egui::Checkbox::new(&mut self.filter.only_visible, "Only my viewing area"),
            )
            .on_hover_text("Hide events whose object falls outside your mapped sky (View tab).");
        });
    }
}

fn chip(ui: &mut egui::Ui, on: &mut bool, category: EventCategory) {
    if ui
        .add(egui::Button::selectable(*on, category.label()))
        .clicked()
    {
        *on = !*on;
    }
}

fn detail_text(e: &AstroEvent) -> String {
    let (y, mo, d, h, mi, _, _) = e.time.to_gregorian_utc();
    let stamp = format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02} UTC");
    if e.detail.is_empty() {
        stamp
    } else {
        format!("{}\n{stamp}", e.detail)
    }
}

fn category_icon(category: EventCategory) -> RichText {
    let (icon, color) = match category {
        EventCategory::Sun => (icons::SUN, Color32::from_rgb(255, 214, 130)),
        EventCategory::Eclipse => (icons::MOON, Color32::from_rgb(200, 190, 230)),
        EventCategory::Approach => (icons::ARROWS_IN, Color32::from_rgb(170, 200, 255)),
        EventCategory::RiseSet => (icons::ARROW_LINE_UP, Color32::from_rgb(160, 210, 180)),
    };
    RichText::new(icon).color(color)
}

fn head(header: &mut egui_extras::TableRow, label: &str, tip: &str) {
    header.col(|ui| {
        ui.strong(label).on_hover_text(tip);
    });
}
