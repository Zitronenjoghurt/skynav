use egui::{ComboBox, DragValue, Grid, Id, Response, RichText, ScrollArea, TextEdit, Widget};
use skynav::{Capital, Simulation, places};

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
        let obs = &mut self.sim.observer;
        ui.scope(|ui| {
            ui.set_min_width(ui.available_width());
            ui.label(RichText::new("Observer location").strong())
                .on_hover_text("Where on Earth the Sky and Events views are computed for.");
            ui.add_space(4.0);
            Grid::new("observer_grid").num_columns(2).show(ui, |ui| {
                ui.label("Latitude")
                    .on_hover_text("Degrees north (+) or south (-) of the equator.");
                ui.add(
                    DragValue::new(&mut obs.latitude_deg)
                        .range(-90.0..=90.0)
                        .speed(0.1)
                        .suffix("°"),
                );
                ui.end_row();
                ui.label("Longitude")
                    .on_hover_text("Degrees east (+) or west (-) of Greenwich.");
                ui.add(
                    DragValue::new(&mut obs.longitude_deg)
                        .range(-180.0..=180.0)
                        .speed(0.1)
                        .suffix("°"),
                );
                ui.end_row();
                ui.label("Height")
                    .on_hover_text("Elevation above sea level, in metres.");
                ui.add(DragValue::new(&mut obs.height_m).suffix(" m"));
                ui.end_row();
            });
            ui.add_space(6.0);
            capital_picker(ui, obs);
            ui.add_space(4.0);
            ui.weak("Tip: click the globe to set this location.");
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
