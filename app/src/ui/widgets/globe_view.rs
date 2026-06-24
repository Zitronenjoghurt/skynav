use crate::gfx::sky::{self, SkyInstance};
use crate::gfx::{OrbitCamera, globe};
use crate::ui::Selection;
use crate::ui::overlay::{label_at, project};
use crate::ui::widgets::sky_view::{body_style, star_style};
use egui::{Align2, Color32, FontId, Frame, Pos2, Rect, Response, Sense, Stroke, Widget, vec2};
use glam::{Mat3, Mat4, Vec3, Vec4};
use serde::{Deserialize, Serialize};
use skynav::{Body, Simulation, Star};

/// Radius of the background star/body shell (well outside the unit globe).
const BACKGROUND_RADIUS: f32 = 300.0;
/// Click/hover pick radius in pixels (squared).
const PICK_DIST_SQ: f32 = 196.0;

/// Toggleable globe-view overlays, persisted across sessions.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct GlobeLayers {
    pub stars: bool,
    pub labels: bool,
}

impl Default for GlobeLayers {
    fn default() -> Self {
        Self {
            stars: true,
            labels: true,
        }
    }
}

/// 3D globe in the equatorial J2000 frame. The real star field, Sun, Moon and
/// planets sit on a far shell behind the globe; click the globe to set the
/// observer, click a sky object to select it. Drag to orbit, scroll to zoom.
pub struct GlobeView<'a> {
    sim: &'a mut Simulation,
    camera: &'a mut OrbitCamera,
    stars: &'a [Star],
    selection: &'a mut Option<Selection>,
    layers: &'a mut GlobeLayers,
}

impl<'a> GlobeView<'a> {
    pub fn new(
        sim: &'a mut Simulation,
        camera: &'a mut OrbitCamera,
        stars: &'a [Star],
        selection: &'a mut Option<Selection>,
        layers: &'a mut GlobeLayers,
    ) -> Self {
        Self {
            sim,
            camera,
            stars,
            selection,
            layers,
        }
    }
}

impl Widget for GlobeView<'_> {
    fn ui(mut self, ui: &mut egui::Ui) -> Response {
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        ui.painter_at(rect).rect_filled(rect, 0.0, Color32::BLACK);
        self.camera.handle(&response, ui);

        let aspect = rect.width() / rect.height().max(1.0);
        let view = self.camera.view();
        let proj = self.camera.proj(aspect);
        let view_proj = proj * view;
        let orientation = self.sim.earth_orientation().as_mat3();
        let sun = self.sim.direction_equatorial(Body::Sun);
        let sun_dir = Vec3::new(sun.x as f32, sun.y as f32, sun.z as f32).normalize_or_zero();

        if response.clicked()
            && let Some(p) = response.interact_pointer_pos()
        {
            self.handle_click(view_proj, orientation, p, rect);
        }

        if self.layers.stars {
            sky::show(ui, rect, view, proj, self.background(), Vec::new());
        }
        globe::show(
            ui,
            rect,
            view_proj,
            Mat4::from_mat3(orientation),
            sun_dir,
            [1.0, 1.0, 1.0],
        );

        self.draw_overlay(ui, rect, view_proj, orientation, sun_dir);
        self.layer_controls(ui, rect);
        response
    }
}

impl GlobeView<'_> {
    /// Star field + Sun/Moon/planet billboards on a far shell, in the equatorial
    /// J2000 frame the globe is rendered in.
    fn background(&self) -> Vec<SkyInstance> {
        let mut instances = Vec::with_capacity(self.stars.len() + Body::ALL.len());
        for star in self.stars {
            let (size, brightness) = star_style(star.magnitude);
            instances.push(SkyInstance {
                position: (Vec3::from(star.unit) * BACKGROUND_RADIUS).to_array(),
                size: size * BACKGROUND_RADIUS,
                color: star.color,
                brightness,
            });
        }
        for body in Body::ALL {
            if let Some(dir) = self.body_dir(body) {
                let (size, color, brightness) = body_style(body);
                instances.push(SkyInstance {
                    position: (dir * BACKGROUND_RADIUS).to_array(),
                    size: size * BACKGROUND_RADIUS,
                    color,
                    brightness,
                });
            }
        }
        instances
    }

    /// Equatorial-frame unit direction to a body, or `None` for Earth/degenerate.
    fn body_dir(&self, body: Body) -> Option<Vec3> {
        if body == Body::Earth {
            return None;
        }
        let d = self.sim.direction_equatorial(body);
        let v = Vec3::new(d.x as f32, d.y as f32, d.z as f32);
        (v != Vec3::ZERO).then_some(v)
    }

    fn handle_click(&mut self, view_proj: Mat4, orientation: Mat3, pointer: Pos2, rect: Rect) {
        // Clicking the globe itself sets the observer location; otherwise pick a
        // sky object.
        if let Some((lat, lon)) = pick_globe(view_proj, orientation, pointer, rect) {
            self.sim.observer.latitude_deg = lat;
            self.sim.observer.longitude_deg = lon;
        } else {
            *self.selection = self.pick_object(view_proj, rect, pointer);
        }
    }

    fn pick_object(&self, view_proj: Mat4, rect: Rect, pointer: Pos2) -> Option<Selection> {
        let mut best: Option<(f32, Selection)> = None;
        let mut consider = |pos: Vec3, sel: Selection| {
            if let Some(p) = project(view_proj, pos, rect) {
                let d2 = p.distance_sq(pointer);
                if d2 < PICK_DIST_SQ && best.as_ref().is_none_or(|b| d2 < b.0) {
                    best = Some((d2, sel));
                }
            }
        };
        for body in Body::ALL {
            if let Some(dir) = self.body_dir(body) {
                consider(dir * BACKGROUND_RADIUS, Selection::Body(body));
            }
        }
        for (i, star) in self.stars.iter().enumerate() {
            consider(
                Vec3::from(star.unit) * BACKGROUND_RADIUS,
                Selection::Star(i),
            );
        }
        best.map(|(_, sel)| sel)
    }

    fn draw_overlay(
        &self,
        ui: &egui::Ui,
        rect: Rect,
        view_proj: Mat4,
        orientation: Mat3,
        sun_dir: Vec3,
    ) {
        let painter = ui.painter_at(rect);
        let eye = self.camera.eye().normalize_or_zero();

        // Sub-solar point: where the Sun is directly overhead (a surface point).
        if sun_dir.dot(eye) > 0.0
            && let Some(p) = project(view_proj, sun_dir, rect)
        {
            let color = Color32::from_rgb(255, 220, 120);
            painter.circle_stroke(p, 5.0, Stroke::new(1.6, color));
            if self.layers.labels {
                label_at(&painter, p + vec2(7.0, -7.0), "Subsolar", color);
            }
        }

        // Observer location ring on the surface.
        let o = self.sim.observer.geocentric_itrs();
        let local = Vec3::new(o.x as f32, o.y as f32, o.z as f32).normalize_or_zero();
        let world = orientation * local;
        if world.dot(eye) > 0.0
            && let Some(p) = project(view_proj, world, rect)
        {
            painter.circle_stroke(p, 5.0, Stroke::new(2.0, Color32::YELLOW));
            label_at(&painter, p + vec2(8.0, -7.0), "Observer", Color32::YELLOW);
        }

        if self.layers.labels {
            self.draw_labels(&painter, rect, view_proj);
        }
        self.draw_selection(&painter, rect, view_proj);
        self.identify(ui, &painter, rect, view_proj);

        painter.text(
            rect.left_top() + vec2(8.0, 8.0),
            Align2::LEFT_TOP,
            format!(
                "Lat {:.2}°  Lon {:.2}°   (click globe to set location)",
                self.sim.observer.latitude_deg, self.sim.observer.longitude_deg
            ),
            FontId::proportional(12.0),
            Color32::from_rgb(170, 185, 205),
        );
    }

    fn draw_labels(&self, painter: &egui::Painter, rect: Rect, view_proj: Mat4) {
        for body in Body::ALL {
            if let Some(dir) = self.body_dir(body)
                && let Some(p) = project(view_proj, dir * BACKGROUND_RADIUS, rect)
            {
                label_at(painter, p, body.name(), Color32::from_rgb(200, 210, 230));
            }
        }
        for star in self.stars {
            if star.magnitude < 1.8
                && !star.name.is_empty()
                && let Some(p) = project(view_proj, Vec3::from(star.unit) * BACKGROUND_RADIUS, rect)
            {
                label_at(painter, p, &star.name, Color32::from_rgb(170, 180, 200));
            }
        }
    }

    fn selection_pos(&self) -> Option<Vec3> {
        match (*self.selection)? {
            Selection::Body(body) => Some(self.body_dir(body)? * BACKGROUND_RADIUS),
            Selection::Star(i) => Some(Vec3::from(self.stars.get(i)?.unit) * BACKGROUND_RADIUS),
        }
    }

    fn draw_selection(&self, painter: &egui::Painter, rect: Rect, view_proj: Mat4) {
        let Some(pos) = self.selection_pos() else {
            return;
        };
        let Some(p) = project(view_proj, pos, rect) else {
            return;
        };
        let accent = Color32::from_rgb(120, 180, 255);
        painter.circle_stroke(p, 11.0, Stroke::new(1.6, accent));
        for (dx, dy) in [(0.0, -16.0), (0.0, 16.0), (-16.0, 0.0), (16.0, 0.0)] {
            painter.line_segment(
                [p + vec2(dx * 0.55, dy * 0.55), p + vec2(dx, dy)],
                Stroke::new(1.6, accent),
            );
        }
    }

    fn identify(&self, ui: &egui::Ui, painter: &egui::Painter, rect: Rect, view_proj: Mat4) {
        let Some(pointer) = ui.input(|i| i.pointer.hover_pos()) else {
            return;
        };
        if !rect.contains(pointer) {
            return;
        }
        let mut best: Option<(f32, String)> = None;
        let mut consider = |pos: Vec3, name: String| {
            if let Some(p) = project(view_proj, pos, rect) {
                let d2 = p.distance_sq(pointer);
                if d2 < PICK_DIST_SQ && best.as_ref().is_none_or(|b| d2 < b.0) {
                    best = Some((d2, name));
                }
            }
        };
        for body in Body::ALL {
            if let Some(dir) = self.body_dir(body) {
                consider(dir * BACKGROUND_RADIUS, body.name().to_string());
            }
        }
        for star in self.stars {
            let name = if star.name.is_empty() {
                format!("mag {:.1} star", star.magnitude)
            } else {
                star.name.clone()
            };
            consider(Vec3::from(star.unit) * BACKGROUND_RADIUS, name);
        }
        if let Some((_, name)) = best {
            label_at(painter, pointer + vec2(12.0, 8.0), &name, Color32::WHITE);
        }
    }

    fn layer_controls(&mut self, ui: &mut egui::Ui, rect: Rect) {
        egui::Area::new(ui.id().with("globe_layers"))
            .fixed_pos(rect.right_top() + vec2(-128.0, 8.0))
            .show(ui.ctx(), |ui| {
                Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(110.0);
                    ui.strong("Layers");
                    ui.checkbox(&mut self.layers.stars, "Star field");
                    ui.checkbox(&mut self.layers.labels, "Labels");
                });
            });
    }
}

/// Ray-cast a screen click onto the unit globe and return geodetic lat/lon.
fn pick_globe(view_proj: Mat4, orientation: Mat3, screen: Pos2, rect: Rect) -> Option<(f64, f64)> {
    let nx = (screen.x - rect.left()) / rect.width() * 2.0 - 1.0;
    let ny = 1.0 - (screen.y - rect.top()) / rect.height() * 2.0;
    let inv = view_proj.inverse();
    let near = inv * Vec4::new(nx, ny, -1.0, 1.0);
    let far = inv * Vec4::new(nx, ny, 1.0, 1.0);
    let origin = near.truncate() / near.w;
    let dir = (far.truncate() / far.w - origin).normalize();

    let b = 2.0 * origin.dot(dir);
    let c = origin.dot(origin) - 1.0;
    let disc = b * b - 4.0 * c;
    if disc < 0.0 {
        return None;
    }
    let t = (-b - disc.sqrt()) * 0.5;
    if t < 0.0 {
        return None;
    }

    let local = (orientation.transpose() * (origin + dir * t)).normalize();
    let lat = (local.z as f64).clamp(-1.0, 1.0).asin().to_degrees();
    let lon = (local.y as f64).atan2(local.x as f64).to_degrees();
    Some((lat, lon))
}
