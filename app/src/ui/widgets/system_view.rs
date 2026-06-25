use crate::gfx::OrbitCamera;
use crate::ui::Selection;
use crate::ui::overlay::{label_at, project};
use egui::{
    Align2, Color32, FontId, Frame, Id, Pos2, Rect, Response, Sense, Stroke, Vec2, Widget, pos2,
    vec2,
};
use glam::{DVec3, Mat4, Vec3};
use hifitime::Duration;
use serde::{Deserialize, Serialize};
use skynav::math::AU_KM;
use skynav::{Body, Simulation};

/// Camera distance below which the in-scene Earth-Moon system is drawn.
const MOON_ZOOM_DIST: f32 = 16.0;

/// Faked Moon-orbit radius around Earth in the heliocentric view. The true Moon
/// distance is invisibly small at this scale, so it is exaggerated - but tied to
/// the zoom and clamped well inside the gap to Venus's orbit, so it reads as a
/// tight satellite instead of slicing through neighbouring orbits. The same
/// zoom-relative scheme will extend to the other planets' moons later.
fn moon_orbit_r(camera_distance: f32) -> f32 {
    (camera_distance * 0.04).clamp(0.05, 0.18)
}

/// Toggleable System-view options, persisted across sessions.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SystemLayers {
    pub orbits: bool,
    pub labels: bool,
    pub moon: bool,
    pub inset: bool,
}

impl Default for SystemLayers {
    fn default() -> Self {
        Self {
            orbits: true,
            labels: true,
            moon: true,
            inset: false,
        }
    }
}

/// Planets shown in the heliocentric view (Sun is the origin, Moon omitted).
const PLANETS: [Body; 8] = [
    Body::Mercury,
    Body::Venus,
    Body::Earth,
    Body::Mars,
    Body::Jupiter,
    Body::Saturn,
    Body::Uranus,
    Body::Neptune,
];

/// Points sampled per traced orbit. Dense enough that the straight segments
/// hug the true ellipse.
const ORBIT_SAMPLES: usize = 240;
const MOON_SAMPLES: usize = 96;

// Earth-Moon inset geometry.
const INSET_SIZE: f32 = 188.0;
const INSET_MARGIN: f32 = 10.0;
/// AU mapped to the inset edge (Moon apogee is ~0.0027 AU).
const INSET_MAX_AU: f32 = 0.0029;

/// Cached, render-space orbital paths. The paths are periodic, so a single
/// computation traces each full ellipse regardless of the current epoch. The
/// Moon ring is kept in geocentric ecliptic AU for the Earth-Moon inset.
#[derive(Default)]
pub struct OrbitCache {
    paths: Vec<(Body, Vec<Vec3>)>,
    moon_ring: Vec<Vec2>,
}

impl OrbitCache {
    fn paths(&mut self, sim: &Simulation) -> &[(Body, Vec<Vec3>)] {
        if self.paths.is_empty() {
            self.paths = PLANETS
                .iter()
                .map(|&body| {
                    let points = sim
                        .orbit_path(body, ORBIT_SAMPLES)
                        .iter()
                        .map(|&p| to_render(p))
                        .collect();
                    (body, points)
                })
                .collect();
        }
        &self.paths
    }

    fn moon_ring(&mut self, sim: &Simulation) -> &[Vec2] {
        if self.moon_ring.is_empty() {
            let period = Body::Moon.orbital_period_days();
            self.moon_ring = (0..MOON_SAMPLES)
                .map(|i| {
                    let epoch = sim.clock.epoch
                        + Duration::from_days(i as f64 / MOON_SAMPLES as f64 * period);
                    let g = sim.heliocentric_at(Body::Moon, epoch)
                        - sim.heliocentric_at(Body::Earth, epoch);
                    Vec2::new(g.x as f32, g.y as f32)
                })
                .collect();
        }
        &self.moon_ring
    }
}

/// Heliocentric solar-system view: the Sun at the centre with the planets riding
/// their true orbits. Drag to orbit, scroll to zoom, click a body to select it.
pub struct SystemView<'a> {
    sim: &'a Simulation,
    camera: &'a mut OrbitCamera,
    orbits: &'a mut OrbitCache,
    selection: &'a mut Option<Selection>,
    layers: &'a mut SystemLayers,
}

impl<'a> SystemView<'a> {
    pub fn new(
        sim: &'a Simulation,
        camera: &'a mut OrbitCamera,
        orbits: &'a mut OrbitCache,
        selection: &'a mut Option<Selection>,
        layers: &'a mut SystemLayers,
    ) -> Self {
        Self {
            sim,
            camera,
            orbits,
            selection,
            layers,
        }
    }
}

impl Widget for SystemView<'_> {
    fn ui(mut self, ui: &mut egui::Ui) -> Response {
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, Color32::from_rgb(3, 4, 9));
        self.camera.handle(&response, ui);

        // Pan/frame the camera onto a freshly selected body (top-down), so we
        // can zoom into a planet and its moons.
        self.frame_on_select(ui);

        // Keep the focus on the selected body as it moves: advancing time or
        // jumping to an event shifts the planet, so re-aim the orbit centre at
        // its current position (preserving the user's orientation and zoom).
        if let Some(Selection::Body(body)) = *self.selection {
            let pos = self.body_render(body);
            self.camera.track(pos);
        }

        let aspect = rect.width() / rect.height().max(1.0);
        let view_proj = self.camera.view_proj(aspect);
        let hover = response.hover_pos();

        // Screen positions of every clickable body (Sun first).
        let mut screen: Vec<(Body, Pos2)> = Vec::new();
        if let Some(p) = project(view_proj, Vec3::ZERO, rect) {
            screen.push((Body::Sun, p));
        }
        for body in PLANETS {
            if let Some(p) = project(view_proj, to_render(self.sim.heliocentric(body)), rect) {
                screen.push((body, p));
            }
        }
        // In-scene Moon around Earth, only when zoomed in enough to resolve it.
        let moon_scene = self.moon_scene_pos();
        if let Some(mp) = moon_scene
            && let Some(p) = project(view_proj, mp, rect)
        {
            screen.push((Body::Moon, p));
        }

        let nearest = hover.and_then(|h| {
            screen
                .iter()
                .filter(|(_, p)| p.distance(h) < 14.0)
                .min_by(|a, b| a.1.distance(h).total_cmp(&b.1.distance(h)))
                .map(|(b, _)| *b)
        });
        if response.clicked()
            && let Some(pointer) = response.interact_pointer_pos()
        {
            let inset = inset_rect(rect);
            if inset.contains(pointer) {
                if let Some(sel) = inset_pick(self.sim, inset, pointer) {
                    *self.selection = Some(sel);
                }
            } else {
                *self.selection = nearest.map(Selection::Body);
            }
        }

        if self.layers.orbits {
            for (body, points) in self.orbits.paths(self.sim) {
                let selected = *self.selection == Some(Selection::Body(*body));
                let color = if selected {
                    Color32::from_rgb(90, 130, 200)
                } else {
                    Color32::from_rgb(38, 48, 70)
                };
                draw_path(&painter, view_proj, rect, points, color);
            }
        }

        let pulse = if self.selection.is_some() {
            ui.ctx().request_repaint();
            let t = ui.input(|i| i.time);
            0.5 + 0.5 * (t * 3.5).sin() as f32
        } else {
            0.0
        };

        let labels = self.layers.labels;

        // Sun.
        if let Some(p) = project(view_proj, Vec3::ZERO, rect) {
            let sun = Color32::from_rgb(255, 228, 130);
            draw_body(
                &painter,
                p,
                8.0,
                sun,
                "Sun",
                Marks {
                    selected: *self.selection == Some(Selection::Body(Body::Sun)),
                    hovered: nearest == Some(Body::Sun),
                    labels,
                    pulse,
                },
            );
        }

        for body in PLANETS {
            let pos = to_render(self.sim.heliocentric(body));
            if let Some(p) = project(view_proj, pos, rect) {
                let (radius, color) = style(body);
                draw_body(
                    &painter,
                    p,
                    radius,
                    color,
                    body.name(),
                    Marks {
                        selected: *self.selection == Some(Selection::Body(body)),
                        hovered: nearest == Some(body),
                        labels,
                        pulse,
                    },
                );
            }
        }

        if let Some(mp) = moon_scene {
            self.draw_moon_scene(&painter, view_proj, rect, mp, nearest, pulse);
        }

        if self.layers.inset {
            self.draw_moon_inset(ui, rect);
        }
        self.options(ui, rect);
        response
    }
}

impl SystemView<'_> {
    /// Picture-in-picture Earth-Moon system (true scale, top-down ecliptic),
    /// since the Moon is invisibly close to Earth in the heliocentric overview.
    fn draw_moon_inset(&mut self, ui: &egui::Ui, view: Rect) {
        if view.width() < INSET_SIZE * 1.6 || view.height() < INSET_SIZE * 1.6 {
            return;
        }
        let inset = inset_rect(view);
        let p = ui.painter_at(inset);
        p.rect_filled(inset, 6.0, Color32::from_rgb(6, 9, 16));
        p.rect_stroke(
            inset,
            6.0,
            Stroke::new(1.0, Color32::from_rgb(40, 52, 74)),
            egui::StrokeKind::Inside,
        );
        p.text(
            inset.left_top() + vec2(7.0, 5.0),
            Align2::LEFT_TOP,
            "Earth-Moon",
            FontId::proportional(11.0),
            Color32::from_rgb(150, 165, 190),
        );

        let center = inset_center(inset);
        let scale = inset_scale();

        // Sun-direction tick: which way the sunlight (and the lit limb) faces.
        let sun = self.sim.geocentric(Body::Sun);
        let sdir = Vec2::new(sun.x as f32, -(sun.y as f32));
        if sdir.length() > 0.0 {
            let d = sdir.normalized();
            p.line_segment(
                [
                    center + d * (INSET_SIZE * 0.30),
                    center + d * (INSET_SIZE * 0.40),
                ],
                Stroke::new(1.5, Color32::from_rgb(150, 140, 90)),
            );
            label_at(
                &p,
                center + d * (INSET_SIZE * 0.40),
                "Sun",
                Color32::from_rgb(150, 140, 90),
            );
        }

        // Moon orbit ring (geocentric, true scale).
        let ring: Vec<Pos2> = self
            .orbits
            .moon_ring(self.sim)
            .iter()
            .map(|g| center + vec2(g.x, -g.y) * scale)
            .collect();
        for w in ring.windows(2) {
            p.line_segment(
                [w[0], w[1]],
                Stroke::new(1.0, Color32::from_rgb(40, 50, 72)),
            );
        }
        if let (Some(first), Some(last)) = (ring.first(), ring.last()) {
            p.line_segment(
                [*last, *first],
                Stroke::new(1.0, Color32::from_rgb(40, 50, 72)),
            );
        }

        // Earth.
        if *self.selection == Some(Selection::Body(Body::Earth)) {
            p.circle_stroke(
                center,
                9.0,
                Stroke::new(1.5, Color32::from_rgb(120, 170, 255)),
            );
        }
        p.circle_filled(center, 5.0, Color32::from_rgb(90, 150, 240));
        label_at(
            &p,
            center + vec2(7.0, -7.0),
            "Earth",
            Color32::from_rgb(150, 190, 255),
        );

        // Moon: brightness encodes the illuminated fraction.
        let g = self.sim.geocentric(Body::Moon);
        let moon = center + vec2(g.x as f32, -(g.y as f32)) * scale;
        let (frac, _) = self.sim.moon_illumination();
        let shade = ((0.18 + 0.74 * frac as f32).clamp(0.0, 1.0) * 235.0) as u8;
        if *self.selection == Some(Selection::Body(Body::Moon)) {
            p.circle_stroke(
                moon,
                8.0,
                Stroke::new(1.5, Color32::from_rgb(120, 170, 255)),
            );
        }
        p.circle_filled(
            moon,
            3.6,
            Color32::from_rgb(shade, shade, shade.saturating_add(8)),
        );
        label_at(
            &p,
            moon + vec2(6.0, -7.0),
            "Moon",
            Color32::from_rgb(210, 215, 225),
        );

        let km = self.sim.geocentric(Body::Moon).length() * AU_KM;
        p.text(
            inset.left_bottom() + vec2(7.0, -5.0),
            Align2::LEFT_BOTTOM,
            format!("{km:.0} km   {:.0}% lit", frac * 100.0),
            FontId::proportional(10.0),
            Color32::from_rgb(140, 155, 180),
        );
    }

    /// When the selection changes to a body, smoothly frame it top-down.
    fn frame_on_select(&mut self, ui: &egui::Ui) {
        let id = Id::new("system_last_sel");
        let last: Option<Selection> = ui.data(|d| d.get_temp(id)).flatten();
        if last != *self.selection {
            if let Some(Selection::Body(body)) = *self.selection {
                self.camera.frame(self.body_render(body), 6.0);
            }
            ui.data_mut(|d| d.insert_temp(id, *self.selection));
        }
    }

    /// Render-space point a body sits at (the Moon rides its in-scene ring).
    fn body_render(&self, body: Body) -> Vec3 {
        match body {
            Body::Sun => Vec3::ZERO,
            Body::Moon => self
                .moon_scene_pos()
                .unwrap_or_else(|| earth_render(self.sim)),
            other => to_render(self.sim.heliocentric(other)),
        }
    }

    /// Render-space Moon position on its faked ring around Earth, or `None` when
    /// the camera is too far out to bother drawing it.
    fn moon_scene_pos(&self) -> Option<Vec3> {
        if !self.layers.moon || self.camera.distance > MOON_ZOOM_DIST {
            return None;
        }
        let earth = earth_render(self.sim);
        let g = self.sim.geocentric(Body::Moon);
        let ang = (g.y as f32).atan2(g.x as f32);
        let r = moon_orbit_r(self.camera.distance);
        Some(earth + Vec3::new(ang.cos(), ang.sin(), 0.0) * r)
    }

    fn draw_moon_scene(
        &self,
        painter: &egui::Painter,
        view_proj: Mat4,
        rect: Rect,
        moon_pos: Vec3,
        nearest: Option<Body>,
        pulse: f32,
    ) {
        let earth = earth_render(self.sim);
        let r = moon_orbit_r(self.camera.distance);
        let ring: Vec<Vec3> = (0..=64)
            .map(|i| {
                let a = i as f32 / 64.0 * std::f32::consts::TAU;
                earth + Vec3::new(a.cos(), a.sin(), 0.0) * r
            })
            .collect();
        draw_path(
            painter,
            view_proj,
            rect,
            &ring,
            Color32::from_rgb(40, 50, 72),
        );

        if let Some(p) = project(view_proj, moon_pos, rect) {
            let (frac, _) = self.sim.moon_illumination();
            let shade = ((0.2 + 0.7 * frac as f32).clamp(0.0, 1.0) * 235.0) as u8;
            draw_body(
                painter,
                p,
                3.2,
                Color32::from_rgb(shade, shade, shade.saturating_add(8)),
                "Moon",
                Marks {
                    selected: *self.selection == Some(Selection::Body(Body::Moon)),
                    hovered: nearest == Some(Body::Moon),
                    labels: self.layers.labels,
                    pulse,
                },
            );
        }
    }

    fn options(&mut self, ui: &mut egui::Ui, rect: Rect) {
        egui::Area::new(ui.id().with("system_options"))
            .fixed_pos(rect.right_top() + vec2(-126.0, 8.0))
            .show(ui.ctx(), |ui| {
                Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(110.0);
                    ui.strong("Options");
                    ui.checkbox(&mut self.layers.orbits, "Orbits");
                    ui.checkbox(&mut self.layers.labels, "Labels");
                    ui.checkbox(&mut self.layers.moon, "Moon (zoom in)");
                    ui.checkbox(&mut self.layers.inset, "Earth-Moon inset");
                });
            });
    }
}

/// Earth's render-space position.
fn earth_render(sim: &Simulation) -> Vec3 {
    to_render(sim.heliocentric(Body::Earth))
}

fn inset_rect(view: Rect) -> Rect {
    Rect::from_min_size(
        pos2(
            view.left() + INSET_MARGIN,
            view.bottom() - INSET_MARGIN - INSET_SIZE,
        ),
        Vec2::splat(INSET_SIZE),
    )
}

fn inset_center(inset: Rect) -> Pos2 {
    inset.center() + vec2(0.0, 6.0)
}

fn inset_scale() -> f32 {
    (INSET_SIZE * 0.5 - 22.0) / INSET_MAX_AU
}

fn inset_pick(sim: &Simulation, inset: Rect, pointer: Pos2) -> Option<Selection> {
    let center = inset_center(inset);
    let g = sim.geocentric(Body::Moon);
    let moon = center + vec2(g.x as f32, -(g.y as f32)) * inset_scale();
    if moon.distance(pointer) < 10.0 {
        Some(Selection::Body(Body::Moon))
    } else if center.distance(pointer) < 9.0 {
        Some(Selection::Body(Body::Earth))
    } else {
        None
    }
}

/// Map a heliocentric point (AU) into render space: same direction, distance
/// log-compressed so inner and outer planets share one view.
fn to_render(helio: DVec3) -> Vec3 {
    let dir = Vec3::new(helio.x as f32, helio.y as f32, helio.z as f32);
    dir.normalize_or_zero() * scale(helio.length())
}

/// Compress AU distances logarithmically so inner and outer planets all fit.
fn scale(au: f64) -> f32 {
    (au as f32 + 1.0).ln() * 3.0
}

/// Highlight state for a drawn body.
struct Marks {
    selected: bool,
    hovered: bool,
    labels: bool,
    pulse: f32,
}

fn draw_body(
    painter: &egui::Painter,
    pos: Pos2,
    radius: f32,
    color: Color32,
    name: &str,
    m: Marks,
) {
    if m.selected {
        let halo = radius + 5.0 + m.pulse * 5.0;
        painter.circle_stroke(
            pos,
            halo,
            Stroke::new(1.5, Color32::from_rgb(120, 170, 255)),
        );
    }
    let r = if m.hovered { radius + 1.5 } else { radius };
    painter.circle_filled(pos, r, color);
    let emphasised = m.selected || m.hovered;
    let label_color = if emphasised { Color32::WHITE } else { color };
    if emphasised {
        label_at(painter, pos + vec2(r + 4.0, -7.0), name, label_color);
    } else if m.labels {
        painter.text(
            pos + vec2(r + 4.0, -7.0),
            egui::Align2::LEFT_TOP,
            name,
            FontId::proportional(12.0),
            label_color,
        );
    }
}

fn draw_path(
    painter: &egui::Painter,
    view_proj: Mat4,
    rect: Rect,
    points: &[Vec3],
    color: Color32,
) {
    let mut prev = points.last().and_then(|&p| project(view_proj, p, rect));
    for &point in points {
        let cur = project(view_proj, point, rect);
        if let (Some(p0), Some(p1)) = (prev, cur) {
            painter.line_segment([p0, p1], Stroke::new(1.0, color));
        }
        prev = cur;
    }
}

fn style(body: Body) -> (f32, Color32) {
    match body {
        Body::Mercury => (3.5, Color32::from_rgb(200, 190, 175)),
        Body::Venus => (5.0, Color32::from_rgb(255, 245, 215)),
        Body::Earth => (5.5, Color32::from_rgb(110, 170, 255)),
        Body::Mars => (4.5, Color32::from_rgb(240, 130, 90)),
        Body::Jupiter => (7.0, Color32::from_rgb(230, 210, 180)),
        Body::Saturn => (6.5, Color32::from_rgb(230, 215, 165)),
        Body::Uranus => (5.0, Color32::from_rgb(180, 220, 230)),
        Body::Neptune => (5.0, Color32::from_rgb(150, 180, 240)),
        Body::Sun | Body::Moon => (3.0, Color32::GRAY),
    }
}
