use crate::ui::observed::{body_key, constellation_key, star_key};
use crate::ui::{Observed, Selection, icons};
use egui::{Button, Color32, Id, Label, ProgressBar, Response, RichText, Sense, TextEdit, Widget};
use skynav::{Body, Constellation, Star};

/// Brightest magnitude of named star included in the checklist.
const STAR_MAG_LIMIT: f32 = 3.0;

const DONE: Color32 = Color32::from_rgb(120, 215, 150);
const TODO: Color32 = Color32::from_rgb(105, 116, 134);
const NAME_DONE: Color32 = Color32::from_rgb(205, 216, 232);

/// One checklist target: a stable key, a display label, the colour of its name
/// and the selection it maps to (constellations are not selectable).
struct Item {
    key: String,
    label: String,
    color: Color32,
    select: Option<Selection>,
}

struct Group {
    title: &'static str,
    items: Vec<Item>,
    default_open: bool,
}

/// An observing checklist: tick off the Solar System bodies, bright named stars
/// and constellations you have actually seen. Marks are shared with the Info,
/// Bodies and Visible panels and persist across sessions.
pub struct ChecklistPanel<'a> {
    stars: &'a [Star],
    constellations: &'a [Constellation],
    observed: &'a mut Observed,
    selection: &'a mut Option<Selection>,
}

impl<'a> ChecklistPanel<'a> {
    pub fn new(
        stars: &'a [Star],
        constellations: &'a [Constellation],
        observed: &'a mut Observed,
        selection: &'a mut Option<Selection>,
    ) -> Self {
        Self {
            stars,
            constellations,
            observed,
            selection,
        }
    }

    /// Build the (owned) checklist catalogue. Returns owned data so the render
    /// pass can freely mutate `observed`/`selection` while iterating it.
    fn groups(&self) -> Vec<Group> {
        let solar = Body::ALL
            .iter()
            .filter(|b| **b != Body::Earth)
            .map(|&body| Item {
                key: body_key(body),
                label: body.name().to_string(),
                color: body_tint(body),
                select: Some(Selection::Body(body)),
            })
            .collect();

        let mut stars: Vec<(usize, &Star)> = self
            .stars
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.name.is_empty() && s.magnitude <= STAR_MAG_LIMIT)
            .collect();
        stars.sort_by(|a, b| a.1.magnitude.total_cmp(&b.1.magnitude));
        let bright_stars = stars
            .into_iter()
            .map(|(i, s)| Item {
                key: star_key(&s.name),
                label: format!("{}  (mag {:.1})", s.name, s.magnitude),
                color: Color32::from_rgb(190, 200, 225),
                select: Some(Selection::Star(i)),
            })
            .collect();

        let mut con: Vec<&Constellation> = self.constellations.iter().collect();
        con.sort_by(|a, b| a.name.cmp(&b.name));
        let constellations = con
            .into_iter()
            .map(|c| Item {
                key: constellation_key(&c.name),
                label: c.name.clone(),
                color: Color32::from_rgb(180, 190, 210),
                select: None,
            })
            .collect();

        vec![
            Group {
                title: "Solar System",
                items: solar,
                default_open: true,
            },
            Group {
                title: "Bright stars",
                items: bright_stars,
                default_open: true,
            },
            Group {
                title: "Constellations",
                items: constellations,
                default_open: false,
            },
        ]
    }
}

impl Widget for ChecklistPanel<'_> {
    fn ui(mut self, ui: &mut egui::Ui) -> Response {
        let groups = self.groups();
        let filter_id = Id::new("checklist_filter");
        let mut filter: String = ui.data(|d| d.get_temp(filter_id)).unwrap_or_default();

        let total: usize = groups.iter().map(|g| g.items.len()).sum();
        let done: usize = groups
            .iter()
            .flat_map(|g| &g.items)
            .filter(|i| self.observed.contains(&i.key))
            .count();

        let response = ui
            .scope(|ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("{} Observed", icons::LIST_CHECKS))
                            .strong()
                            .size(15.0),
                    );
                    ui.label(
                        RichText::new(format!("{done} / {total}"))
                            .color(DONE)
                            .strong(),
                    );
                });
                let frac = if total > 0 {
                    done as f32 / total as f32
                } else {
                    0.0
                };
                ui.add(ProgressBar::new(frac).desired_height(8.0).fill(DONE));
                ui.add_space(4.0);
                ui.add(
                    TextEdit::singleline(&mut filter)
                        .desired_width(f32::INFINITY)
                        .hint_text(format!("{} Filter", icons::MAGNIFYING_GLASS)),
                );
                ui.add_space(4.0);

                let q = filter.trim().to_lowercase();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for group in &groups {
                        self.group_ui(ui, group, &q);
                    }
                });
            })
            .response;

        ui.data_mut(|d| d.insert_temp(filter_id, filter));
        response
    }
}

impl ChecklistPanel<'_> {
    fn group_ui(&mut self, ui: &mut egui::Ui, group: &Group, query: &str) {
        let matching: Vec<&Item> = group
            .items
            .iter()
            .filter(|i| query.is_empty() || i.label.to_lowercase().contains(query))
            .collect();
        if matching.is_empty() {
            return;
        }
        let done = matching
            .iter()
            .filter(|i| self.observed.contains(&i.key))
            .count();

        let header =
            RichText::new(format!("{}   {done} / {}", group.title, matching.len())).strong();
        egui::CollapsingHeader::new(header)
            .id_salt(group.title)
            .default_open(group.default_open)
            // Force every group open while a filter is active.
            .open((!query.is_empty()).then_some(true))
            .show(ui, |ui| {
                for item in matching {
                    self.row(ui, item);
                }
            });
    }

    fn row(&mut self, ui: &mut egui::Ui, item: &Item) {
        let is_obs = self.observed.contains(&item.key);
        ui.horizontal(|ui| {
            let icon = if is_obs {
                icons::CHECK_CIRCLE
            } else {
                icons::CIRCLE
            };
            let color = if is_obs { DONE } else { TODO };
            let tip = if is_obs {
                "Mark as not observed"
            } else {
                "Mark as observed"
            };
            if ui
                .add(Button::new(RichText::new(icon).color(color).size(16.0)).frame(false))
                .on_hover_text(tip)
                .clicked()
            {
                self.observed.toggle(&item.key);
            }

            let name_color = if is_obs { NAME_DONE } else { item.color };
            let text = RichText::new(&item.label).color(name_color);
            match item.select {
                Some(sel) => {
                    let selected = *self.selection == Some(sel);
                    if ui.selectable_label(selected, text).clicked() {
                        *self.selection = if selected { None } else { Some(sel) };
                    }
                }
                None => {
                    if ui
                        .add(Label::new(text).sense(Sense::click()))
                        .on_hover_text("Click to toggle observed")
                        .clicked()
                    {
                        self.observed.toggle(&item.key);
                    }
                }
            }
        });
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
