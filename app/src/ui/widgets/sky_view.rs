use crate::gfx::LookAroundCamera;
use crate::gfx::sky::{self, LineVertex, SkyInstance};
use crate::ui::Selection;
use crate::ui::icons;
use crate::ui::overlay::{label_at, project};
use egui::{Align2, Color32, FontId, Frame, Pos2, Rect, Response, Sense, Stroke, Widget};
use glam::{Mat3, Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};
use skynav::math::ecliptic_to_equatorial;
use skynav::sky::Horizontal;
use skynav::{Body, Constellation, Simulation, Star};

const RADIUS: f32 = 1.0;
const CONSTELLATION_COLOR: [f32; 3] = [0.16, 0.26, 0.42];
const HORIZON_COLOR: [f32; 3] = [0.16, 0.62, 0.50];
const GRID_COLOR: [f32; 3] = [0.11, 0.15, 0.22];
const ECLIPTIC_COLOR: [f32; 3] = [0.70, 0.50, 0.10];
/// Click/hover pick radius in pixels (squared).
const PICK_DIST_SQ: f32 = 196.0;

/// Toggleable sky-view overlays, persisted across sessions.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SkyLayers {
    pub constellations: bool,
    pub constellation_names: bool,
    pub labels: bool,
    pub horizon: bool,
    pub equatorial_grid: bool,
    pub ecliptic: bool,
    pub mag_limit: f32,
}

impl Default for SkyLayers {
    fn default() -> Self {
        Self {
            constellations: true,
            constellation_names: false,
            labels: true,
            horizon: true,
            equatorial_grid: false,
            ecliptic: true,
            mag_limit: 6.5,
        }
    }
}

/// Observer's sky dome: star catalogue, constellation figures, optional grids,
/// hover-to-identify and click-to-select. Drag to look around, scroll to zoom.
pub struct SkyView<'a> {
    sim: &'a Simulation,
    camera: &'a mut LookAroundCamera,
    stars: &'a [Star],
    constellations: &'a [Constellation],
    selection: &'a mut Option<Selection>,
    layers: &'a mut SkyLayers,
    follow: &'a mut bool,
}

impl<'a> SkyView<'a> {
    pub fn new(
        sim: &'a Simulation,
        camera: &'a mut LookAroundCamera,
        stars: &'a [Star],
        constellations: &'a [Constellation],
        selection: &'a mut Option<Selection>,
        layers: &'a mut SkyLayers,
        follow: &'a mut bool,
    ) -> Self {
        Self {
            sim,
            camera,
            stars,
            constellations,
            selection,
            layers,
            follow,
        }
    }
}

impl Widget for SkyView<'_> {
    fn ui(mut self, ui: &mut egui::Ui) -> Response {
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        ui.painter()
            .rect_filled(rect, 0.0, Color32::from_rgb(2, 4, 10));
        self.camera.handle(&response, ui);

        let aspect = rect.width() / rect.height().max(1.0);
        let view = self.camera.view();
        let proj = self.camera.proj(aspect);
        let view_proj = proj * view;
        let horizon = self.sim.equatorial_to_horizon().as_mat3();

        let star_dirs: Vec<Vec3> = self
            .stars
            .iter()
            .map(|s| horizon * Vec3::from(s.unit))
            .collect();

        // Follow-cam: keep the selection centred until the user drags.
        if response.dragged() {
            *self.follow = false;
        } else if *self.follow
            && let Some((az, alt)) = self.selected_horizontal(&star_dirs)
        {
            self.camera.look_at(az, alt);
            ui.ctx().request_repaint();
        }

        let instances = self.build_instances(&star_dirs);
        let lines = self.build_lines(horizon);
        sky::show(ui, rect, view, proj, instances, lines);

        if response.clicked()
            && let Some(pointer) = response.interact_pointer_pos()
        {
            *self.selection = self.pick(rect, view_proj, &star_dirs, pointer);
        }

        self.draw_overlay(ui, rect, view_proj, horizon, &star_dirs, &response);
        self.layer_controls(ui, rect);
        response
    }
}

impl SkyView<'_> {
    fn build_instances(&self, star_dirs: &[Vec3]) -> Vec<SkyInstance> {
        let mut instances = Vec::with_capacity(star_dirs.len() + Body::ALL.len());

        for (star, dir) in self.stars.iter().zip(star_dirs) {
            if star.magnitude > self.layers.mag_limit {
                continue;
            }
            if self.sim.view.enabled {
                let h = star_horizontal(*dir);
                if !self
                    .sim
                    .view
                    .star_visible(star.magnitude, h.azimuth_deg(), h.altitude_deg())
                {
                    continue;
                }
            }
            let (size, brightness) = star_style(star.magnitude);
            instances.push(SkyInstance {
                position: (*dir * RADIUS).to_array(),
                size,
                color: star.color,
                brightness,
            });
        }

        for body in Body::ALL {
            if body == self.sim.observer_body {
                continue;
            }
            if let Some(h) = self.sim.observed_body(body) {
                let (base, color) = body_style(body);
                let (size, brightness) = apparent_look(base, self.sim.apparent_magnitude(body));
                let inst = billboard(h, size, color, brightness);
                if body == Body::Sun {
                    instances.extend(sky::sun_glow(&inst));
                }
                instances.push(inst);
            }
        }
        instances
    }

    fn build_lines(&self, horizon: Mat3) -> Vec<LineVertex> {
        let mut lines = Vec::new();

        if self.layers.equatorial_grid {
            self.push_grid(&mut lines, horizon);
        }
        if self.layers.ecliptic {
            push_ecliptic(&mut lines, horizon);
        }
        if self.layers.constellations {
            for constellation in self.constellations {
                for polyline in &constellation.lines {
                    for pair in polyline.windows(2) {
                        let a = horizon * Vec3::from(pair[0]) * RADIUS;
                        let b = horizon * Vec3::from(pair[1]) * RADIUS;
                        lines.push(line_vertex(a, CONSTELLATION_COLOR));
                        lines.push(line_vertex(b, CONSTELLATION_COLOR));
                    }
                }
            }
        }
        if self.layers.horizon {
            let segments = 72;
            for i in 0..segments {
                let a = horizon_point(i, segments);
                let b = horizon_point(i + 1, segments);
                lines.push(line_vertex(a, HORIZON_COLOR));
                lines.push(line_vertex(b, HORIZON_COLOR));
            }
        }
        lines
    }

    fn push_grid(&self, lines: &mut Vec<LineVertex>, horizon: Mat3) {
        // Align the grid's poles/equator to the body you observe from, not always
        // Earth's, so it is correct from Mars, the Moon, etc.
        let rot = self.grid_rot();
        // Meridians (constant RA), running nearly to the poles (±88°).
        for m in 0..12 {
            let ra = m as f32 / 12.0 * std::f32::consts::TAU;
            let mut prev = None;
            for d in -11..=11 {
                let dec = d as f32 * 8f32.to_radians();
                let p = horizon * rot * unit(ra, dec) * RADIUS;
                if let Some(q) = prev {
                    lines.push(line_vertex(q, GRID_COLOR));
                    lines.push(line_vertex(p, GRID_COLOR));
                }
                prev = Some(p);
            }
        }
        // Parallels (constant Dec), with a tight ring near each pole (±85°) so the
        // converging RA lines close on a clean cap.
        for d in [-88, -85, -60, -30, 0, 30, 60, 85, 88] {
            let dec = (d as f32).to_radians();
            let mut prev = None;
            for r in 0..=36 {
                let ra = r as f32 / 36.0 * std::f32::consts::TAU;
                let p = horizon * rot * unit(ra, dec) * RADIUS;
                if let Some(q) = prev {
                    lines.push(line_vertex(q, GRID_COLOR));
                    lines.push(line_vertex(p, GRID_COLOR));
                }
                prev = Some(p);
            }
        }
    }

    /// Rotation aligning the equatorial grid to the body the observer stands on
    /// (~identity for Earth, whose mean pole defines the J2000 equatorial frame).
    fn grid_rot(&self) -> Mat3 {
        let pole = (self.sim.orientation().as_mat3() * Vec3::Z).normalize_or_zero();
        Mat3::from_quat(Quat::from_rotation_arc(Vec3::Z, pole))
    }

    fn draw_overlay(
        &self,
        ui: &egui::Ui,
        rect: Rect,
        view_proj: Mat4,
        horizon: Mat3,
        star_dirs: &[Vec3],
        response: &Response,
    ) {
        let painter = ui.painter_at(rect);

        if self.sim.view.enabled {
            self.draw_view_patches(&painter, rect, view_proj);
        }

        self.draw_selection(&painter, rect, view_proj, star_dirs);

        for (label, az) in [("N", 0.0), ("E", 90.0), ("S", 180.0), ("W", 270.0)] {
            let dir = enu(az_alt(az, 2.0));
            if let Some(p) = project(view_proj, dir, rect) {
                painter.text(
                    p,
                    Align2::CENTER_CENTER,
                    label,
                    FontId::proportional(15.0),
                    Color32::from_rgb(150, 180, 140),
                );
            }
        }

        if self.layers.constellation_names {
            for constellation in self.constellations {
                if let Some(dir) = constellation_centroid(constellation)
                    && let Some(p) = project(view_proj, horizon * dir, rect)
                {
                    painter.text(
                        p,
                        Align2::CENTER_CENTER,
                        constellation.full_name(),
                        FontId::proportional(11.0),
                        Color32::from_rgb(110, 130, 165),
                    );
                }
            }
        }

        if self.layers.labels {
            for body in Body::ALL {
                if body == self.sim.observer_body {
                    continue;
                }
                if let Some(h) = self.sim.observed_body(body)
                    && self.in_view_h(h)
                    && let Some(p) = project(view_proj, enu(h), rect)
                {
                    label_at(&painter, p, body.name(), Color32::from_rgb(200, 210, 230));
                }
            }
            for (star, dir) in self.stars.iter().zip(star_dirs) {
                if star.magnitude < 1.8
                    && !star.name.is_empty()
                    && self.star_in_view(star, *dir)
                    && let Some(p) = project(view_proj, *dir, rect)
                {
                    label_at(&painter, p, &star.name, Color32::from_rgb(170, 180, 200));
                }
            }
        }

        if response.hovered()
            && let Some(pointer) = ui.input(|i| i.pointer.hover_pos())
        {
            self.identify(&painter, rect, view_proj, star_dirs, pointer);
        }
    }

    /// Outline each mapped viewing patch and label the active area.
    fn draw_view_patches(&self, painter: &egui::Painter, rect: Rect, view_proj: Mat4) {
        let accent = Color32::from_rgb(120, 200, 255);
        for patch in &self.sim.view.patches {
            let span = (patch.az_max_deg - patch.az_min_deg).rem_euclid(360.0);
            let span = if span == 0.0 { 360.0 } else { span };
            let mut border: Vec<(f64, f64)> = Vec::new();
            const STEPS: usize = 24;
            let (alt0, alt1) = (patch.alt_min_deg, patch.alt_max_deg);
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
            let mut prev: Option<egui::Pos2> = None;
            for (az, alt) in border {
                let p = project(view_proj, enu(az_alt(az, alt)), rect);
                if let (Some(a), Some(b)) = (prev, p) {
                    painter.line_segment([a, b], Stroke::new(1.5, accent));
                }
                prev = p;
            }
        }
        painter.text(
            rect.left_bottom() + egui::vec2(8.0, -8.0),
            Align2::LEFT_BOTTOM,
            format!(
                "Viewing area active (limit mag {:.1})",
                self.sim.view.limiting_magnitude
            ),
            FontId::proportional(12.0),
            accent,
        );
    }

    fn layer_controls(&mut self, ui: &mut egui::Ui, rect: Rect) {
        const PANEL_W: f32 = 214.0;
        egui::Area::new(ui.id().with("sky_layers"))
            .fixed_pos(rect.left_top() + egui::vec2(12.0, 12.0))
            .constrain_to(rect)
            .show(ui.ctx(), |ui| {
                Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(PANEL_W);
                    egui::CollapsingHeader::new(
                        egui::RichText::new(format!("{} View options", icons::GEAR)).size(15.0),
                    )
                    .default_open(true)
                    .show(ui, |ui| {
                        let l = &mut *self.layers;
                        ui.strong("In the sky");
                        ui.checkbox(&mut l.constellations, "Constellation figures");
                        ui.add_enabled(
                            l.constellations,
                            egui::Checkbox::new(&mut l.constellation_names, "Constellation names"),
                        );
                        ui.checkbox(&mut l.labels, "Object labels");
                        ui.checkbox(&mut l.ecliptic, "Ecliptic (Sun's path)");
                        ui.checkbox(&mut l.equatorial_grid, "Equatorial grid (RA / Dec)");
                        ui.checkbox(&mut l.horizon, "Horizon & compass");
                        ui.add_space(2.0);
                        ui.label("Faintest stars")
                            .on_hover_text("Hide stars dimmer than this.");
                        ui.add(egui::Slider::new(&mut l.mag_limit, 1.0..=6.5).fixed_decimals(1));
                    });
                });
            });
    }

    /// Azimuth/altitude (radians) of the current selection, for follow-cam.
    fn selected_horizontal(&self, star_dirs: &[Vec3]) -> Option<(f32, f32)> {
        match (*self.selection)? {
            Selection::Body(body) => {
                let h = self.sim.observed_body(body)?;
                Some((h.azimuth as f32, h.altitude as f32))
            }
            Selection::Star(i) => {
                let d = star_dirs.get(i)?;
                Some((d.x.atan2(d.y), d.z.clamp(-1.0, 1.0).asin()))
            }
        }
    }

    /// Screen position of a selected object, if it is on screen.
    fn selection_screen(&self, rect: Rect, view_proj: Mat4, star_dirs: &[Vec3]) -> Option<Pos2> {
        match (*self.selection)? {
            Selection::Body(body) => {
                let h = self.sim.observed_body(body)?;
                project(view_proj, enu(h), rect)
            }
            Selection::Star(i) => project(view_proj, *star_dirs.get(i)?, rect),
        }
    }

    fn draw_selection(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        view_proj: Mat4,
        star_dirs: &[Vec3],
    ) {
        let Some(p) = self.selection_screen(rect, view_proj, star_dirs) else {
            return;
        };
        let accent = Color32::from_rgb(120, 180, 255);
        painter.circle_stroke(p, 11.0, Stroke::new(1.6, accent));
        for (dx, dy) in [(0.0, -16.0), (0.0, 16.0), (-16.0, 0.0), (16.0, 0.0)] {
            let from = p + egui::vec2(dx * 0.55, dy * 0.55);
            let to = p + egui::vec2(dx, dy);
            painter.line_segment([from, to], Stroke::new(1.6, accent));
        }
    }

    /// Pick the nearest body or star to `pointer`, or `None` to clear.
    fn pick(
        &self,
        rect: Rect,
        view_proj: Mat4,
        star_dirs: &[Vec3],
        pointer: Pos2,
    ) -> Option<Selection> {
        let mut best: Option<(f32, Selection)> = None;
        let mut consider = |screen: Pos2, sel: Selection| {
            let d2 = screen.distance_sq(pointer);
            if d2 < PICK_DIST_SQ && best.as_ref().is_none_or(|b| d2 < b.0) {
                best = Some((d2, sel));
            }
        };

        for body in Body::ALL {
            if body == self.sim.observer_body {
                continue;
            }
            if let Some(h) = self.sim.observed_body(body)
                && self.in_view_h(h)
                && let Some(p) = project(view_proj, enu(h), rect)
            {
                consider(p, Selection::Body(body));
            }
        }
        for (i, (star, dir)) in self.stars.iter().zip(star_dirs).enumerate() {
            if star.magnitude <= self.layers.mag_limit
                && self.star_in_view(star, *dir)
                && let Some(p) = project(view_proj, *dir, rect)
            {
                consider(p, Selection::Star(i));
            }
        }
        best.map(|(_, sel)| sel)
    }

    /// Whether a body direction is inside the mapped viewing area (always true
    /// when the area is disabled).
    fn in_view_h(&self, h: Horizontal) -> bool {
        !self.sim.view.enabled || self.sim.view.contains(h.azimuth_deg(), h.altitude_deg())
    }

    fn star_in_view(&self, star: &Star, dir: Vec3) -> bool {
        if !self.sim.view.enabled {
            return true;
        }
        let h = star_horizontal(dir);
        self.sim
            .view
            .star_visible(star.magnitude, h.azimuth_deg(), h.altitude_deg())
    }

    fn identify(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        view_proj: Mat4,
        star_dirs: &[Vec3],
        pointer: Pos2,
    ) {
        let mut best: Option<(f32, String, Horizontal)> = None;
        let mut consider = |screen: Pos2, name: String, h: Horizontal| {
            let d2 = screen.distance_sq(pointer);
            if d2 < PICK_DIST_SQ && best.as_ref().is_none_or(|b| d2 < b.0) {
                best = Some((d2, name, h));
            }
        };

        for body in Body::ALL {
            if body == self.sim.observer_body {
                continue;
            }
            if let Some(h) = self.sim.observed_body(body)
                && self.in_view_h(h)
                && let Some(p) = project(view_proj, enu(h), rect)
            {
                consider(p, body.name().to_string(), h);
            }
        }
        for (star, dir) in self.stars.iter().zip(star_dirs) {
            if star.magnitude <= self.layers.mag_limit
                && self.star_in_view(star, *dir)
                && let Some(p) = project(view_proj, *dir, rect)
            {
                let name = if star.name.is_empty() {
                    format!("mag {:.1} star", star.magnitude)
                } else {
                    star.name.clone()
                };
                consider(p, name, star_horizontal(*dir));
            }
        }

        if let Some((_, name, h)) = best {
            let text = format!(
                "{name}\naz {:.1}°  alt {:+.1}°",
                h.azimuth_deg(),
                h.altitude_deg()
            );
            label_at(
                painter,
                pointer + egui::vec2(12.0, 8.0),
                &text,
                Color32::WHITE,
            );
        }
    }
}

/// Unit vector in the equatorial frame for a right ascension / declination.
fn unit(ra: f32, dec: f32) -> Vec3 {
    let (sd, cd) = dec.sin_cos();
    let (sr, cr) = ra.sin_cos();
    Vec3::new(cd * cr, cd * sr, sd)
}

fn push_ecliptic(lines: &mut Vec<LineVertex>, horizon: Mat3) {
    let mut prev = None;
    for i in 0..=72 {
        let lon = i as f64 / 72.0 * std::f64::consts::TAU;
        let ecl = skynav::math::DVec3::new(lon.cos(), lon.sin(), 0.0);
        let eq = ecliptic_to_equatorial(ecl);
        let p = horizon * Vec3::new(eq.x as f32, eq.y as f32, eq.z as f32) * RADIUS;
        if let Some(q) = prev {
            lines.push(line_vertex(q, ECLIPTIC_COLOR));
            lines.push(line_vertex(p, ECLIPTIC_COLOR));
        }
        prev = Some(p);
    }
}

fn constellation_centroid(constellation: &Constellation) -> Option<Vec3> {
    let mut sum = Vec3::ZERO;
    let mut count = 0.0;
    for polyline in &constellation.lines {
        for v in polyline {
            sum += Vec3::from(*v);
            count += 1.0;
        }
    }
    if count == 0.0 {
        return None;
    }
    Some((sum / count).normalize_or_zero())
}

fn star_horizontal(dir: Vec3) -> Horizontal {
    let dir = dir.normalize_or_zero();
    Horizontal {
        azimuth: (dir.x as f64).atan2(dir.y as f64),
        altitude: (dir.z as f64).clamp(-1.0, 1.0).asin(),
    }
}

fn billboard(h: Horizontal, size: f32, color: [f32; 3], brightness: f32) -> SkyInstance {
    SkyInstance {
        position: (enu(h) * RADIUS).to_array(),
        size,
        color,
        brightness,
    }
}

fn line_vertex(p: Vec3, color: [f32; 3]) -> LineVertex {
    LineVertex {
        position: p.to_array(),
        color,
    }
}

fn enu(h: Horizontal) -> Vec3 {
    let d = h.enu();
    Vec3::new(d.x as f32, d.y as f32, d.z as f32)
}

fn az_alt(azimuth_deg: f64, altitude_deg: f64) -> Horizontal {
    Horizontal {
        azimuth: azimuth_deg.to_radians(),
        altitude: altitude_deg.to_radians(),
    }
}

fn horizon_point(i: usize, segments: usize) -> Vec3 {
    let az = i as f32 / segments as f32 * std::f32::consts::TAU;
    Vec3::new(az.sin(), az.cos(), 0.0) * RADIUS
}

/// Angular billboard size + brightness for a star of the given magnitude.
/// `pub(crate)` so the globe background can reuse the same look.
pub(crate) fn star_style(magnitude: f32) -> (f32, f32) {
    let t = ((6.5 - magnitude) / 6.5).clamp(0.0, 1.0);
    // Bright stars are a touch larger; faint ones stay near a single pixel. The
    // brightness range is wide so the additive Gaussian core saturates the
    // brightest stars to white while the faintest are barely-there specks.
    let size = 0.0015 + 0.0085 * t * t;
    let brightness = (1.85 - 0.32 * magnitude).clamp(0.08, 3.0);
    (size, brightness)
}

/// Per-body size scale and colour for a billboard. The scale is a relative
/// prominence (the Sun a bit chunkier, planets near 1); the actual angular size
/// is driven by apparent brightness in `apparent_look`, so a body shrinks when
/// seen from far away rather than keeping a fixed disk (which made the Moon read
/// bigger than Earth from Mars).
pub(crate) fn body_style(body: Body) -> (f32, [f32; 3]) {
    match body {
        Body::Sun => (1.7, [1.0, 0.93, 0.75]),
        Body::Moon => (1.0, [0.85, 0.86, 0.92]),
        Body::Mercury => (0.7, [0.80, 0.75, 0.70]),
        Body::Venus => (1.0, [1.0, 0.97, 0.85]),
        Body::Mars => (0.85, [0.95, 0.50, 0.35]),
        Body::Jupiter => (1.25, [0.90, 0.82, 0.70]),
        Body::Saturn => (1.15, [0.90, 0.85, 0.65]),
        Body::Uranus => (0.8, [0.70, 0.85, 0.90]),
        Body::Neptune => (0.8, [0.60, 0.70, 0.95]),
        Body::Earth => (1.0, [0.40, 0.60, 1.0]),
    }
}

/// Billboard size and brightness for a body of apparent magnitude `mag` and
/// per-body `scale`. Both size and glow grow with brightness, so a body that is
/// far away (faint) shrinks to a small dim dot and a near/bright one is a larger
/// glowing disk. Shared by the Sky and the Explorer so a body looks the same in
/// both, and a bright planet always reads larger than a faint one.
pub(crate) fn apparent_look(scale: f32, mag: f64) -> (f32, f32) {
    let m = mag as f32;
    let brightness = (1.6 - 0.28 * m).clamp(0.1, 3.0);
    let size = scale * (0.004 + 0.006 * brightness);
    (size, brightness)
}
