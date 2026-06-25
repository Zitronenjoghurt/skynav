use crate::gfx::sky::{self, SkyInstance};
use crate::gfx::{OrbitCamera, globe};
use crate::ui::Selection;
use crate::ui::overlay::{label_at, project};
use crate::ui::widgets::sky_view::{body_style, star_style};
use egui::{Align2, Color32, FontId, Frame, Pos2, Rect, Response, Sense, Stroke, Widget, vec2};
use glam::{Mat3, Mat4, Vec3, Vec4};
use serde::{Deserialize, Serialize};
use skynav::{Body, Simulation, Star, places};

/// Radius of the background star/body shell (well outside the unit globe).
const BACKGROUND_RADIUS: f32 = 300.0;
/// Click/hover pick radius in pixels (squared).
const PICK_DIST_SQ: f32 = 196.0;
/// Pixel radius for clicking a capital marker (squared).
const CAPITAL_PICK_SQ: f32 = 90.0;

/// Toggleable globe-view overlays, persisted across sessions.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct GlobeLayers {
    pub stars: bool,
    pub labels: bool,
    pub graticule: bool,
    pub axis: bool,
    pub capitals: bool,
    pub view_area: bool,
}

impl Default for GlobeLayers {
    fn default() -> Self {
        Self {
            stars: true,
            labels: true,
            graticule: false,
            axis: false,
            capitals: false,
            view_area: true,
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
        } else {
            sky::show(ui, rect, view, proj, self.body_billboards(), Vec::new());
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
    fn eye(&self) -> Vec3 {
        self.camera.eye()
    }

    /// Star field + Sun/Moon/planet billboards on the far shell.
    fn background(&self) -> Vec<SkyInstance> {
        let mut instances = self.body_billboards();
        for star in self.stars {
            let (size, brightness) = star_style(star.magnitude);
            instances.push(SkyInstance {
                position: (Vec3::from(star.unit) * BACKGROUND_RADIUS).to_array(),
                size: size * BACKGROUND_RADIUS,
                color: star.color,
                brightness,
            });
        }
        instances
    }

    /// Just the Sun/Moon/planet billboards (always drawn, even with stars off).
    fn body_billboards(&self) -> Vec<SkyInstance> {
        let mut instances = Vec::with_capacity(Body::ALL.len());
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
        // A capital marker takes priority, then bare-globe location picking,
        // then a background sky object.
        if self.layers.capitals
            && let Some((lat, lon)) = self.pick_capital(view_proj, orientation, pointer, rect)
        {
            self.sim.observer.latitude_deg = lat;
            self.sim.observer.longitude_deg = lon;
        } else if let Some((lat, lon)) = pick_globe(view_proj, orientation, pointer, rect) {
            self.sim.observer.latitude_deg = lat;
            self.sim.observer.longitude_deg = lon;
        } else {
            *self.selection = self.pick_object(view_proj, rect, pointer);
        }
    }

    fn pick_object(&self, view_proj: Mat4, rect: Rect, pointer: Pos2) -> Option<Selection> {
        let eye = self.eye();
        let mut best: Option<(f32, Selection)> = None;
        let mut consider = |pos: Vec3, sel: Selection| {
            if occluded_by_globe(eye, pos) {
                return;
            }
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
        if self.layers.stars {
            for (i, star) in self.stars.iter().enumerate() {
                consider(
                    Vec3::from(star.unit) * BACKGROUND_RADIUS,
                    Selection::Star(i),
                );
            }
        }
        best.map(|(_, sel)| sel)
    }

    /// Geodetic lat/lon of the nearest visible capital marker to the cursor.
    fn pick_capital(
        &self,
        view_proj: Mat4,
        orientation: Mat3,
        pointer: Pos2,
        rect: Rect,
    ) -> Option<(f64, f64)> {
        let eye = self.eye();
        let mut best: Option<(f32, (f64, f64))> = None;
        for cap in places::capitals() {
            let world = orientation * itrs_unit(cap.lat, cap.lon).normalize_or_zero();
            if occluded_by_globe(eye, world) {
                continue;
            }
            if let Some(p) = project(view_proj, world, rect) {
                let d2 = p.distance_sq(pointer);
                if d2 < CAPITAL_PICK_SQ && best.as_ref().is_none_or(|b| d2 < b.0) {
                    best = Some((d2, (cap.lat, cap.lon)));
                }
            }
        }
        best.map(|(_, ll)| ll)
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
        let eye = self.eye();

        if self.layers.graticule {
            self.draw_graticule(&painter, rect, view_proj, orientation, eye);
        }
        if self.layers.axis {
            self.draw_axis(&painter, rect, view_proj, orientation, eye);
        }
        if self.layers.capitals {
            self.draw_capitals(ui, &painter, rect, view_proj, orientation, eye);
        }
        if self.layers.view_area && self.sim.view.enabled {
            self.draw_view_patches(&painter, rect, view_proj, eye);
        }

        // Sub-solar point: where the Sun is directly overhead (a surface point).
        if !occluded_by_globe(eye, sun_dir)
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
        if !occluded_by_globe(eye, world)
            && let Some(p) = project(view_proj, world, rect)
        {
            painter.circle_stroke(p, 5.0, Stroke::new(2.0, Color32::YELLOW));
            label_at(&painter, p + vec2(8.0, -7.0), "Observer", Color32::YELLOW);
        }

        if self.layers.labels {
            self.draw_labels(&painter, rect, view_proj, eye);
        }
        self.draw_selection(&painter, rect, view_proj, eye);
        self.identify(ui, &painter, rect, view_proj, eye);

        painter.text(
            rect.left_top() + vec2(8.0, 8.0),
            Align2::LEFT_TOP,
            format!(
                "Lat {:.2}°  Lon {:.2}°   (click globe or a capital to set location)",
                self.sim.observer.latitude_deg, self.sim.observer.longitude_deg
            ),
            FontId::proportional(12.0),
            Color32::from_rgb(170, 185, 205),
        );
    }

    fn draw_graticule(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        view_proj: Mat4,
        orientation: Mat3,
        eye: Vec3,
    ) {
        let faint = Color32::from_rgb(70, 90, 120);
        let equator = Color32::from_rgb(110, 150, 190);
        // Sample a lat/lon arc (t in 0..1 -> degrees) and stroke the segments
        // whose endpoints are both on the visible hemisphere.
        let arc = |f: &dyn Fn(f64) -> (f64, f64), color: Color32| {
            const SEGS: usize = 48;
            let mut prev: Option<Pos2> = None;
            let mut prev_vis = false;
            for i in 0..=SEGS {
                let (lat, lon) = f(i as f64 / SEGS as f64);
                let world = orientation * itrs_unit(lat, lon);
                let vis = !occluded_by_globe(eye, world);
                let p = project(view_proj, world, rect);
                if let (Some(a), Some(b)) = (prev, p)
                    && prev_vis
                    && vis
                {
                    painter.line_segment([a, b], Stroke::new(1.0, color));
                }
                prev = p;
                prev_vis = vis;
            }
        };
        // Meridians (constant longitude).
        for lon in (-180..180).step_by(30) {
            arc(&|t| (t * 160.0 - 80.0, lon as f64), faint);
        }
        // Parallels (constant latitude), equator emphasised.
        for lat in (-60..=60).step_by(30) {
            let color = if lat == 0 { equator } else { faint };
            arc(&|t| (lat as f64, t * 360.0 - 180.0), color);
        }
    }

    fn draw_axis(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        view_proj: Mat4,
        orientation: Mat3,
        eye: Vec3,
    ) {
        let axis = (orientation * Vec3::Z).normalize_or_zero();
        let color = Color32::from_rgb(150, 170, 200);
        for (dir, name) in [(axis, "N"), (-axis, "S")] {
            let base = dir; // north/south pole on the surface
            let tip = dir * 1.4;
            if occluded_by_globe(eye, tip) {
                continue;
            }
            if let (Some(a), Some(b)) = (
                project(view_proj, base, rect),
                project(view_proj, tip, rect),
            ) {
                painter.line_segment([a, b], Stroke::new(1.6, color));
                painter.text(
                    b,
                    Align2::CENTER_CENTER,
                    name,
                    FontId::proportional(13.0),
                    color,
                );
            }
        }
    }

    fn draw_capitals(
        &self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        rect: Rect,
        view_proj: Mat4,
        orientation: Mat3,
        eye: Vec3,
    ) {
        let pointer = ui.input(|i| i.pointer.hover_pos());
        let dot = Color32::from_rgb(255, 170, 90);
        let mut hover: Option<(f32, Pos2, String)> = None;
        for cap in places::capitals() {
            let world = orientation * itrs_unit(cap.lat, cap.lon);
            if occluded_by_globe(eye, world) {
                continue;
            }
            if let Some(p) = project(view_proj, world, rect) {
                painter.circle_filled(p, 2.0, dot);
                if let Some(ptr) = pointer {
                    let d2 = p.distance_sq(ptr);
                    if d2 < CAPITAL_PICK_SQ && hover.as_ref().is_none_or(|h| d2 < h.0) {
                        hover = Some((d2, p, format!("{}, {}", cap.name, cap.country)));
                    }
                }
            }
        }
        if let Some((_, p, name)) = hover {
            painter.circle_stroke(p, 4.0, Stroke::new(1.4, Color32::WHITE));
            label_at(painter, p + vec2(8.0, -6.0), &name, Color32::WHITE);
        }
    }

    fn draw_labels(&self, painter: &egui::Painter, rect: Rect, view_proj: Mat4, eye: Vec3) {
        for body in Body::ALL {
            if let Some(dir) = self.body_dir(body) {
                let pos = dir * BACKGROUND_RADIUS;
                if !occluded_by_globe(eye, pos)
                    && let Some(p) = project(view_proj, pos, rect)
                {
                    label_at(painter, p, body.name(), Color32::from_rgb(200, 210, 230));
                }
            }
        }
        if self.layers.stars {
            for star in self.stars {
                let pos = Vec3::from(star.unit) * BACKGROUND_RADIUS;
                if star.magnitude < 1.8
                    && !star.name.is_empty()
                    && !occluded_by_globe(eye, pos)
                    && let Some(p) = project(view_proj, pos, rect)
                {
                    label_at(painter, p, &star.name, Color32::from_rgb(170, 180, 200));
                }
            }
        }
    }

    fn selection_pos(&self) -> Option<Vec3> {
        match (*self.selection)? {
            Selection::Body(body) => Some(self.body_dir(body)? * BACKGROUND_RADIUS),
            Selection::Star(i) => {
                if !self.layers.stars {
                    return None;
                }
                Some(Vec3::from(self.stars.get(i)?.unit) * BACKGROUND_RADIUS)
            }
        }
    }

    fn draw_selection(&self, painter: &egui::Painter, rect: Rect, view_proj: Mat4, eye: Vec3) {
        let Some(pos) = self.selection_pos() else {
            return;
        };
        if occluded_by_globe(eye, pos) {
            return;
        }
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

    fn identify(
        &self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        rect: Rect,
        view_proj: Mat4,
        eye: Vec3,
    ) {
        let Some(pointer) = ui.input(|i| i.pointer.hover_pos()) else {
            return;
        };
        if !rect.contains(pointer) {
            return;
        }
        let mut best: Option<(f32, String)> = None;
        let mut consider = |pos: Vec3, name: String| {
            if occluded_by_globe(eye, pos) {
                return;
            }
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
        if self.layers.stars {
            for star in self.stars {
                let name = if star.name.is_empty() {
                    format!("mag {:.1} star", star.magnitude)
                } else {
                    star.name.clone()
                };
                consider(Vec3::from(star.unit) * BACKGROUND_RADIUS, name);
            }
        }
        if let Some((_, name)) = best {
            label_at(painter, pointer + vec2(12.0, 8.0), &name, Color32::WHITE);
        }
    }

    /// Outline the observer's mapped viewing patches projected onto the
    /// celestial shell (horizontal coords mapped back into the equatorial frame).
    fn draw_view_patches(&self, painter: &egui::Painter, rect: Rect, view_proj: Mat4, eye: Vec3) {
        let inv = self.sim.equatorial_to_horizon().transpose().as_mat3();
        let accent = Color32::from_rgb(120, 200, 255);
        for patch in &self.sim.view.patches {
            let span = (patch.az_max_deg - patch.az_min_deg).rem_euclid(360.0);
            let span = if span == 0.0 { 360.0 } else { span };
            let (alt0, alt1) = (patch.alt_min_deg, patch.alt_max_deg);
            let mut border: Vec<(f64, f64)> = Vec::new();
            const STEPS: usize = 24;
            for i in 0..=STEPS {
                let t = i as f64 / STEPS as f64;
                border.push((patch.az_min_deg + span * t, alt0));
            }
            for i in 0..=STEPS {
                let t = i as f64 / STEPS as f64;
                border.push((patch.az_max_deg, alt0 + (alt1 - alt0) * t));
            }
            for i in 0..=STEPS {
                let t = i as f64 / STEPS as f64;
                border.push((patch.az_max_deg - span * t, alt1));
            }
            for i in 0..=STEPS {
                let t = i as f64 / STEPS as f64;
                border.push((patch.az_min_deg, alt1 - (alt1 - alt0) * t));
            }
            let mut prev: Option<(Pos2, bool)> = None;
            for (az, alt) in border {
                let world = azalt_to_equatorial(inv, az, alt) * BACKGROUND_RADIUS;
                let vis = !occluded_by_globe(eye, world);
                let p = project(view_proj, world, rect);
                if let (Some((a, av)), Some(b)) = (prev, p)
                    && av
                    && vis
                {
                    painter.line_segment([a, b], Stroke::new(1.5, accent));
                }
                prev = p.map(|q| (q, vis));
            }
        }
    }

    fn layer_controls(&mut self, ui: &mut egui::Ui, rect: Rect) {
        egui::Area::new(ui.id().with("globe_layers"))
            .fixed_pos(rect.right_top() + vec2(-132.0, 8.0))
            .show(ui.ctx(), |ui| {
                Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(116.0);
                    ui.strong("Layers");
                    ui.checkbox(&mut self.layers.stars, "Star field");
                    ui.checkbox(&mut self.layers.labels, "Labels");
                    ui.checkbox(&mut self.layers.graticule, "Lat / lon grid");
                    ui.checkbox(&mut self.layers.axis, "Rotation axis");
                    ui.checkbox(&mut self.layers.capitals, "Capitals");
                    ui.add_enabled(
                        self.sim.view.enabled,
                        egui::Checkbox::new(&mut self.layers.view_area, "Viewing area"),
                    );
                });
            });
    }
}

/// Map a horizontal direction (az/alt degrees) back into the equatorial J2000
/// frame via the inverse of the observer's equatorial-to-horizon rotation.
fn azalt_to_equatorial(horizon_inv: Mat3, az_deg: f64, alt_deg: f64) -> Vec3 {
    let (sa, ca) = (alt_deg.to_radians() as f32).sin_cos();
    let (saz, caz) = (az_deg.to_radians() as f32).sin_cos();
    let enu = Vec3::new(ca * saz, ca * caz, sa);
    (horizon_inv * enu).normalize_or_zero()
}

/// Unit vector on the body-fixed sphere for a geodetic lat/lon (degrees).
fn itrs_unit(lat_deg: f64, lon_deg: f64) -> Vec3 {
    let d = places::itrs_unit(lat_deg, lon_deg);
    Vec3::new(d.x as f32, d.y as f32, d.z as f32)
}

/// Whether the unit globe hides `world_point` from `eye` (ray hits the sphere
/// before reaching the point).
fn occluded_by_globe(eye: Vec3, world_point: Vec3) -> bool {
    let delta = world_point - eye;
    let len = delta.length();
    if len < 1e-6 {
        return false;
    }
    let dir = delta / len;
    let b = 2.0 * eye.dot(dir);
    let c = eye.dot(eye) - 1.0;
    let disc = b * b - 4.0 * c;
    if disc < 0.0 {
        return false;
    }
    let t = (-b - disc.sqrt()) * 0.5;
    t > 1e-3 && t < len - 1e-3
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
