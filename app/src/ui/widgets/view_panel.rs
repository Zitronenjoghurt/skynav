use crate::gfx::LookAroundCamera;
use crate::ui::icons;
use crate::ui::widgets::panel_ui;
use egui::{DragValue, Grid, Response, RichText, Slider, Widget};
use skynav::{Patch, Simulation};

/// Editor for the observer's mapped viewing area: enable it, set a limiting
/// magnitude and define one or more sky patches (an azimuth window with an
/// altitude floor and ceiling). When enabled it gates the Visible and Events
/// views and is drawn over the Sky view.
pub struct ViewPanel<'a> {
    sim: &'a mut Simulation,
    sky_camera: &'a LookAroundCamera,
}

impl<'a> ViewPanel<'a> {
    pub fn new(sim: &'a mut Simulation, sky_camera: &'a LookAroundCamera) -> Self {
        Self { sim, sky_camera }
    }
}

impl Widget for ViewPanel<'_> {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        let captured = self.captured_patch();
        let view = &mut self.sim.view;
        ui.scope(|ui| {
            ui.set_min_width(ui.available_width());
            // Sliders fill the panel width (leaving room for the value box) so the
            // tab does not hug the left edge and nothing gets clipped on the right.
            ui.spacing_mut().slider_width = (ui.available_width() - 64.0).max(120.0);

            ui.label(RichText::new("Viewing area").strong()).on_hover_text(
                "Restrict what counts as visible to chosen patches of your sky.",
            );
            ui.add_space(4.0);
            ui.checkbox(&mut view.enabled, "Limit to my viewing area")
                .on_hover_text(
                    "When on, the Sky, Visible and Events views only show what falls inside the patches below.",
                );
            ui.add_space(6.0);

            ui.label("Limiting magnitude")
                .on_hover_text("Faintest star you can see from this site (lower = brighter skies / worse light pollution).");
            ui.add(Slider::new(&mut view.limiting_magnitude, 1.0..=6.5).fixed_decimals(1));

            panel_ui::section(ui, "Sky patches");

            ui.horizontal(|ui| {
                if ui
                    .small_button(format!("{} Add", icons::PLUS))
                    .on_hover_text("Add a sky patch covering the whole sky.")
                    .clicked()
                {
                    view.patches.push(Patch::full());
                }
                if ui
                    .small_button(format!("{} Capture view", icons::CROSSHAIR))
                    .on_hover_text("Add a patch matching the current Sky view direction and zoom.")
                    .clicked()
                {
                    view.patches.push(captured);
                }
            });
            ui.add_space(2.0);
            ui.weak("Azimuth 0=N, 90=E, 180=S, 270=W. Altitude 0=horizon, 90=zenith.");
            ui.add_space(4.0);

            let mut remove: Option<usize> = None;
            for (i, patch) in view.patches.iter_mut().enumerate() {
                Grid::new(("view_patch", i)).num_columns(2).show(ui, |ui| {
                    ui.label(format!("Patch {}", i + 1));
                    if ui.small_button(icons::TRASH).on_hover_text("Remove").clicked() {
                        remove = Some(i);
                    }
                    ui.end_row();

                    ui.label("Azimuth");
                    ui.horizontal(|ui| {
                        ui.add(DragValue::new(&mut patch.az_min_deg).range(0.0..=360.0).suffix("°"));
                        ui.label("to");
                        ui.add(DragValue::new(&mut patch.az_max_deg).range(0.0..=360.0).suffix("°"));
                    });
                    ui.end_row();

                    ui.label("Altitude");
                    ui.horizontal(|ui| {
                        ui.add(DragValue::new(&mut patch.alt_min_deg).range(-10.0..=90.0).suffix("°"));
                        ui.label("to");
                        ui.add(DragValue::new(&mut patch.alt_max_deg).range(-10.0..=90.0).suffix("°"));
                    });
                    ui.end_row();
                });
                ui.add_space(4.0);
            }
            if let Some(i) = remove {
                view.patches.remove(i);
            }

            if view.patches.is_empty() {
                ui.weak("No patches: nothing counts as visible while enabled.");
            }
            ui.add_space(4.0);
            if ui
                .button(format!("{} Reset to full sky", icons::ARROW_COUNTER_CLOCKWISE))
                .clicked()
            {
                view.patches = vec![Patch::full()];
            }
        })
        .response
    }
}

impl ViewPanel<'_> {
    fn captured_patch(&self) -> Patch {
        let az = self.sky_camera.yaw.to_degrees().rem_euclid(360.0) as f64;
        let alt = self.sky_camera.pitch.to_degrees() as f64;
        let half = (self.sky_camera.fov.to_degrees() * 0.5) as f64;
        Patch {
            az_min_deg: (az - half).rem_euclid(360.0),
            az_max_deg: (az + half).rem_euclid(360.0),
            alt_min_deg: (alt - half).clamp(-10.0, 90.0),
            alt_max_deg: (alt + half).clamp(-10.0, 90.0),
        }
    }
}
