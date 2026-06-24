use egui::{ComboBox, DragValue, Grid, Response, RichText, Widget};
use skynav::Simulation;

/// (name, latitude, longitude) presets for quick observer placement.
const CITIES: &[(&str, f64, f64)] = &[
    ("London", 51.5074, -0.1278),
    ("New York", 40.7128, -74.0060),
    ("Sao Paulo", -23.5505, -46.6333),
    ("Cairo", 30.0444, 31.2357),
    ("Nairobi", -1.2921, 36.8219),
    ("Tokyo", 35.6762, 139.6503),
    ("Singapore", 1.3521, 103.8198),
    ("Sydney", -33.8688, 151.2093),
    ("Honolulu", 21.3069, -157.8583),
    ("Reykjavik", 64.1466, -21.9426),
];

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
            ComboBox::from_label("Jump to city")
                .selected_text("Pick a city")
                .show_ui(ui, |ui| {
                    for (name, lat, lon) in CITIES {
                        if ui.selectable_label(false, *name).clicked() {
                            obs.latitude_deg = *lat;
                            obs.longitude_deg = *lon;
                            obs.height_m = 0.0;
                        }
                    }
                });
            ui.add_space(4.0);
            ui.weak("Tip: click the globe to set this location.");
        })
        .response
    }
}
