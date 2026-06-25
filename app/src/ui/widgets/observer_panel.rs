use crate::ui::widgets::panel_ui::{self, field};
use egui::{ComboBox, DragValue, Id, Response, RichText, ScrollArea, TextEdit, Widget};
use skynav::{Body, Capital, Simulation, places};

/// Editor for the observer's geodetic location.
pub struct ObserverPanel<'a> {
    sim: &'a mut Simulation,
}

impl<'a> ObserverPanel<'a> {
    pub fn new(sim: &'a mut Simulation) -> Self {
        Self { sim }
    }
}

impl Widget for ObserverPanel<'_> {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        let sim = self.sim;
        ui.scope(|ui| {
            ui.set_min_width(ui.available_width());
            ui.label(RichText::new("Observer").strong()).on_hover_text(
                "The body and location the Sky, Globe and Events views observe from.",
            );

            panel_ui::section(ui, "Vantage");
            field(ui, "Standing on", |ui| {
                ComboBox::from_id_salt("observer_body")
                    .selected_text(sim.observer_body.name())
                    .show_ui(ui, |ui| {
                        for body in Body::ALL {
                            ui.selectable_value(&mut sim.observer_body, body, body.name());
                        }
                    });
            });

            let on_earth = sim.observer_body == Body::Earth;
            let obs = &mut sim.observer;
            panel_ui::section(ui, "Location");
            field(ui, "Latitude", |ui| {
                ui.add(
                    DragValue::new(&mut obs.latitude_deg)
                        .range(-90.0..=90.0)
                        .speed(0.1)
                        .suffix("°"),
                )
                .on_hover_text("Degrees north (+) or south (-) of the equator.");
            });
            field(ui, "Longitude", |ui| {
                ui.add(
                    DragValue::new(&mut obs.longitude_deg)
                        .range(-180.0..=180.0)
                        .speed(0.1)
                        .suffix("°"),
                )
                .on_hover_text("Degrees east (+) or west (-) of the prime meridian.");
            });
            field(ui, "Height", |ui| {
                ui.add(DragValue::new(&mut obs.height_m).suffix(" m"))
                    .on_hover_text("Elevation above the reference surface, in metres.");
            });

            ui.add_space(6.0);
            if on_earth {
                capital_picker(ui, obs);
                ui.add_space(4.0);
            }
            ui.weak("Tip: click the globe to set the location.");
        })
        .response
    }
}

/// Searchable dropdown of every world capital; picking one moves the observer.
fn capital_picker(ui: &mut egui::Ui, obs: &mut skynav::Observer) {
    let filter_id = Id::new("observer_capital_filter");
    let mut filter: String = ui.data(|d| d.get_temp(filter_id)).unwrap_or_default();

    ComboBox::from_label("Jump to a capital")
        .selected_text("Pick a capital")
        .show_ui(ui, |ui| {
            ui.add(TextEdit::singleline(&mut filter).hint_text("Search city or country"));
            ui.add_space(2.0);
            let q = filter.trim().to_lowercase();
            let mut caps: Vec<&Capital> = places::capitals().iter().collect();
            caps.sort_by_key(|c| c.name);
            ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                for cap in caps {
                    if !q.is_empty()
                        && !cap.name.to_lowercase().contains(&q)
                        && !cap.country.to_lowercase().contains(&q)
                    {
                        continue;
                    }
                    if ui
                        .selectable_label(false, format!("{}, {}", cap.name, cap.country))
                        .clicked()
                    {
                        obs.latitude_deg = cap.lat;
                        obs.longitude_deg = cap.lon;
                        obs.height_m = 0.0;
                    }
                }
            });
        });

    ui.data_mut(|d| d.insert_temp(filter_id, filter));
}
