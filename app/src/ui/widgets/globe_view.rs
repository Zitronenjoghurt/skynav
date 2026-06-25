use crate::gfx::UnifiedCamera;
use crate::gfx::globe::{self, BodyDraw};
use crate::gfx::sky::{self, LineVertex, SkyInstance};
use crate::ui::Selection;
use crate::ui::icons;
use crate::ui::overlay::{label_at, project, project_segment};
use crate::ui::widgets::sky_view::{apparent_look, body_style, star_style};
use crate::ui::widgets::system_view::{OrbitCache, scale, sphere_radius};
use egui::{
    Align2, Color32, FontId, Frame, Pos2, Rect, Response, Sense, Stroke, Widget, pos2, vec2,
};
use glam::{Mat3, Mat4, Quat, Vec3, Vec4};
use serde::{Deserialize, Serialize};
use skynav::math::{DVec3, ecliptic_to_equatorial};
use skynav::{Body, Constellation, Simulation, Star, places};

/// Radius of the background star/body shell. Far enough out that the whole
/// compressed solar system (Neptune reaches a few thousand render units) sorts
/// in front of it; angular sizes are radius-independent, so pushing it out does
/// not change how the stars look.
const BACKGROUND_RADIUS: f32 = 16_000.0;
/// Click/hover pick radius in pixels (squared).
const PICK_DIST_SQ: f32 = 196.0;
/// Pixel radius for clicking a capital marker (squared).
const CAPITAL_PICK_SQ: f32 = 90.0;
/// Range (body radii) the camera eases to when you hop to a new body, so its
/// globe is framed in an orbit view ready to zoom down onto.
const ARRIVAL_RANGE: f32 = 5.0;
/// Night-side floor for far-off planet spheres so a back-lit world stays faintly
/// visible instead of vanishing into black (the body you stand on keeps 0).
const DISTANT_NIGHT_SHADE: f32 = 0.07;
/// Duration of the warp fade when hopping to another body (seconds).
const TRAVEL_SECS: f64 = 0.55;
/// Sky-dome line colours (shared look with the old Sky view). The ecliptic is a
/// warm gold and the horizon a cool teal so the two are easy to tell apart.
const CONSTELLATION_COLOR: [f32; 3] = [0.16, 0.26, 0.42];
const HORIZON_COLOR: [f32; 3] = [0.16, 0.62, 0.50];
const GRID_COLOR: [f32; 3] = [0.11, 0.15, 0.22];
const ECLIPTIC_COLOR: [f32; 3] = [0.70, 0.50, 0.10];

/// In-progress warp to another body, stashed in egui temp memory across frames.
#[derive(Clone)]
struct TravelFade {
    target: Body,
    start: f64,
    swapped: bool,
}

/// How the globe scene is framed for the camera.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GlobeAlignment {
    /// Up is the celestial north pole (inertial J2000) - the body's axis appears
    /// static because the frame is fixed in inertial space.
    Inertial,
    /// Up is the orbital (ecliptic) north and the Sun is locked to a fixed
    /// screen direction, so the body's axis visibly nods toward/away from the Sun
    /// over a year (the seasons).
    Orbital,
}

/// Toggleable Explorer overlays, persisted across sessions. Covers both the
/// globe-scale layers (graticule, axis, capitals) and the sky-dome layers
/// (constellations, horizon, grids) that fade in near a body's surface.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct GlobeLayers {
    pub stars: bool,
    pub labels: bool,
    pub graticule: bool,
    pub axis: bool,
    pub capitals: bool,
    pub view_area: bool,
    /// Marker at the point where the Sun is directly overhead.
    #[serde(default)]
    pub subsolar: bool,
    /// Tropics (max subsolar latitude) and polar circles for the current body.
    #[serde(default)]
    pub special_latitudes: bool,
    pub alignment: GlobeAlignment,
    // Sky-dome layers (shown near the surface, faded out as you rise to orbit).
    pub constellations: bool,
    pub constellation_names: bool,
    pub horizon: bool,
    pub equatorial_grid: bool,
    pub ecliptic: bool,
    pub mag_limit: f32,
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
            subsolar: false,
            special_latitudes: false,
            alignment: GlobeAlignment::Inertial,
            constellations: true,
            constellation_names: false,
            horizon: true,
            equatorial_grid: false,
            ecliptic: true,
            mag_limit: 6.5,
        }
    }
}

/// The Explorer: a continuous 3D view in the equatorial J2000 frame. Up close
/// it is the observer's body as a globe under a star field; zoom out and the
/// surrounding solar system fades in as real lit spheres on their orbits, with
/// the globe shrinking to one body among many. Click the globe to set the
/// observer, click a sky object to select it. Drag to orbit, scroll to zoom.
pub struct GlobeView<'a> {
    sim: &'a mut Simulation,
    camera: &'a mut UnifiedCamera,
    stars: &'a [Star],
    constellations: &'a [Constellation],
    selection: &'a mut Option<Selection>,
    layers: &'a mut GlobeLayers,
    orbits: &'a mut OrbitCache,
}

impl<'a> GlobeView<'a> {
    pub fn new(
        sim: &'a mut Simulation,
        camera: &'a mut UnifiedCamera,
        stars: &'a [Star],
        constellations: &'a [Constellation],
        selection: &'a mut Option<Selection>,
        layers: &'a mut GlobeLayers,
        orbits: &'a mut OrbitCache,
    ) -> Self {
        Self {
            sim,
            camera,
            stars,
            constellations,
            selection,
            layers,
            orbits,
        }
    }
}

impl Widget for GlobeView<'_> {
    fn ui(mut self, ui: &mut egui::Ui) -> Response {
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        ui.painter_at(rect).rect_filled(rect, 0.0, Color32::BLACK);
        self.camera.handle(&response, ui);

        let aspect = rect.width() / rect.height().max(1.0);
        let orientation = self.sim.orientation().as_mat3();
        let sun = self.sim.observer_direction_equatorial(Body::Sun);
        let sun_dir = Vec3::new(sun.x as f32, sun.y as f32, sun.z as f32).normalize_or_zero();
        let (surface, pole) = self.surface_and_pole(orientation);

        // The alignment chooses the orbit framing (Inertial = body axis up;
        // Sun-relative = ecliptic up with the Sun pinned). Everything stays in
        // equatorial coordinates so the camera, occlusion and overlays agree.
        let (orbit_up, orbit_ref) = self.orbit_frame(pole, sun_dir);
        let view = self.camera.view(surface, pole, orbit_up, orbit_ref);
        let proj = self.camera.proj(aspect);
        let view_proj = proj * view;

        // How far the surrounding solar system has faded in (0 close to the
        // body, 1 once it has fully taken over the scene).
        let factor = self.camera.system_factor();

        // One consistent click rule at every zoom: hover identifies, single-click
        // selects an object (or sets your location on bare ground), double-click
        // a body travels there.
        if let Some(p) = response.interact_pointer_pos() {
            if response.double_clicked() {
                if let Some(Selection::Body(body)) = self.pick_object(view_proj, rect, p, factor) {
                    self.start_travel(ui, body);
                }
            } else if response.clicked() {
                self.handle_single_click(view_proj, orientation, p, rect, factor);
            }
        }

        // Star field plus Sun/Moon/planet billboards on the far shell; the
        // billboards fade out as their real spheres fade in. Constellation /
        // horizon / grid lines are sky-dome features, faded out as you rise.
        let billboards = self.body_billboards(1.0 - factor);
        let instances = if self.layers.stars {
            self.with_stars(billboards)
        } else {
            billboards
        };
        let lines = self.sky_lines(factor);
        sky::show(ui, rect, view, proj, instances, lines);

        // The observer's body is the central globe (radius 1, its own slot in
        // the compressed system); the rest of the system grows in around it. The
        // globe keeps its full size and dissolves (opacity) as you touch down, so
        // the descent reads as sinking into a clean planetarium sky rather than a
        // shrinking marble collapsing beneath you.
        let ground = self.camera.ground_factor();
        let mut draws = Vec::new();
        if ground > 0.01 {
            draws.push(globe::draw_body_faded(
                self.sim.observer_body,
                Mat4::from_mat3(orientation),
                sun_dir,
                ground,
            ));
        }
        if factor > 0.0 {
            draws.extend(self.system_draws(factor));
        }
        if !draws.is_empty() {
            globe::show_many(ui, rect, view_proj, draws);
        }

        // Observer-centred orbit lines, traced once and shifted to the observer's
        // moving position each frame (cheap), drawn as a faded overlay.
        let orbit_lines = self.orbit_lines(factor);
        self.draw_overlay(
            ui,
            rect,
            view_proj,
            orientation,
            sun_dir,
            factor,
            &orbit_lines,
        );
        self.options_panel(ui, rect, factor);
        self.control_hints(ui, rect, factor);
        self.zoom_bar(ui, rect);
        // Warp fade covers the instant re-centre when hopping to another body.
        self.animate_travel(ui, rect);
        response
    }
}

impl GlobeView<'_> {
    fn eye(&self) -> Vec3 {
        let orientation = self.sim.orientation().as_mat3();
        let (surface, pole) = self.surface_and_pole(orientation);
        let sun = self.sim.observer_direction_equatorial(Body::Sun);
        let sun_dir = Vec3::new(sun.x as f32, sun.y as f32, sun.z as f32).normalize_or_zero();
        let (orbit_up, orbit_ref) = self.orbit_frame(pole, sun_dir);
        self.camera.eye(surface, orbit_up, orbit_ref)
    }

    /// Whether the (visible) globe hides a sky point. In the planetarium surface
    /// view the globe is gone, so nothing is occluded - the whole celestial
    /// sphere is visible, pickable and labelled.
    fn sky_occluded(&self, eye: Vec3, pos: Vec3) -> bool {
        self.camera.ground_factor() > 0.4 && occluded_by_globe(eye, pos)
    }

    /// The observer's surface point (unit) and the body's north pole, both in the
    /// equatorial render frame - the anchors the unified camera rises from.
    fn surface_and_pole(&self, orientation: Mat3) -> (Vec3, Vec3) {
        let o = self.sim.observer.geocentric_fixed(
            self.sim.observer_body.equatorial_radius_km(),
            self.sim.observer_body.flattening(),
        );
        let local = Vec3::new(o.x as f32, o.y as f32, o.z as f32);
        let surface = (orientation * local).normalize_or_zero();
        let pole = (orientation * Vec3::Z).normalize_or_zero();
        (surface, pole)
    }

    /// Orbit up-axis and yaw reference (equatorial frame) for the alignment mode.
    /// Inertial: the body's own spin axis is up, referenced to its equator's node
    /// (so the body rotates beneath the camera). Sun-relative: ecliptic north is
    /// up and the Sun is the reference, so it stays pinned as time advances and
    /// the axis visibly nods over a year.
    fn orbit_frame(&self, pole: Vec3, sun_dir: Vec3) -> (Vec3, Vec3) {
        match self.layers.alignment {
            GlobeAlignment::Inertial => {
                let node = Vec3::Z.cross(pole);
                let reference = if node.length_squared() < 1e-6 {
                    Vec3::X
                } else {
                    node.normalize_or_zero()
                };
                (pole, reference)
            }
            GlobeAlignment::Orbital => {
                let reference = if sun_dir == Vec3::ZERO {
                    Vec3::X
                } else {
                    sun_dir
                };
                (ecliptic_north(), reference)
            }
        }
    }

    /// Latitude on the body where the Sun is currently overhead (its declination
    /// in the body frame) - the season indicator.
    fn subsolar_latitude(&self, orientation: Mat3, sun_dir: Vec3) -> f32 {
        let pole = (orientation * Vec3::Z).normalize_or_zero();
        sun_dir.dot(pole).clamp(-1.0, 1.0).asin().to_degrees()
    }

    /// Append the star field to a set of billboards. Stars stay full brightness
    /// at every zoom level; only the body billboards fade as the spheres grow in.
    fn with_stars(&self, mut instances: Vec<SkyInstance>) -> Vec<SkyInstance> {
        for star in self.stars {
            if star.magnitude > self.layers.mag_limit {
                continue;
            }
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

    /// Sun/Moon/planet billboards on the far shell, dimmed by `fade` (1 = full,
    /// 0 = gone) so they hand off to the real spheres as the system fades in.
    fn body_billboards(&self, fade: f32) -> Vec<SkyInstance> {
        let mut instances = Vec::with_capacity(Body::ALL.len());
        if fade <= 0.0 {
            return instances;
        }
        for body in Body::ALL {
            if let Some(dir) = self.body_dir(body) {
                let (base, color) = body_style(body);
                let (size, brightness) = apparent_look(base, self.sim.apparent_magnitude(body));
                let inst = SkyInstance {
                    position: (dir * BACKGROUND_RADIUS).to_array(),
                    size: size * BACKGROUND_RADIUS,
                    color,
                    brightness: brightness * fade,
                };
                if body == Body::Sun {
                    instances.extend(sky::sun_glow(&inst));
                }
                instances.push(inst);
            }
        }
        instances
    }

    /// Sky-dome lines drawn on the far shell in the equatorial frame:
    /// constellation figures, the equatorial grid, the ecliptic and the local
    /// horizon. All fade out (additively dimmed) as the camera rises to orbit,
    /// since they only make sense standing on the surface looking up.
    /// Rotation aligning the equatorial grid to the body the observer stands on,
    /// so its equator and poles match that body rather than always Earth's. It is
    /// ~identity for Earth, whose mean pole defines the J2000 equatorial frame.
    fn equatorial_grid_rot(&self) -> Mat3 {
        let pole = (self.sim.orientation().as_mat3() * Vec3::Z).normalize_or_zero();
        Mat3::from_quat(Quat::from_rotation_arc(Vec3::Z, pole))
    }

    fn sky_lines(&self, factor: f32) -> Vec<LineVertex> {
        let mut lines = Vec::new();
        let seg = |lines: &mut Vec<LineVertex>, a: Vec3, b: Vec3, color: [f32; 3]| {
            lines.push(LineVertex {
                position: (a * BACKGROUND_RADIUS).to_array(),
                color,
            });
            lines.push(LineVertex {
                position: (b * BACKGROUND_RADIUS).to_array(),
                color,
            });
        };

        // Constellation figures are part of the background sky, like the star
        // field, so they stay visible at every zoom (including out in the system)
        // rather than fading as the camera rises.
        if self.layers.constellations {
            for con in self.constellations {
                for poly in &con.lines {
                    for w in poly.windows(2) {
                        seg(
                            &mut lines,
                            Vec3::from(w[0]),
                            Vec3::from(w[1]),
                            CONSTELLATION_COLOR,
                        );
                    }
                }
            }
        }

        // The remaining lines are observer/surface references (grids, the local
        // horizon) that only make sense near a body, so they fade out as you rise.
        let fade = 1.0 - factor;
        if fade <= 0.02 {
            return lines;
        }
        let dim = |c: [f32; 3]| [c[0] * fade, c[1] * fade, c[2] * fade];

        if self.layers.equatorial_grid {
            let color = dim(GRID_COLOR);
            // Align the grid's poles/equator to the body you stand on (not always
            // Earth's), so it reads correctly from Mars, the Moon, etc.
            let rot = self.equatorial_grid_rot();
            for m in 0..12 {
                let ra = m as f32 / 12.0 * std::f32::consts::TAU;
                let mut prev = None;
                // Run each RA line nearly to the poles (±88°).
                for d in -11..=11 {
                    let p = rot * radec_unit(ra, d as f32 * 8f32.to_radians());
                    if let Some(q) = prev {
                        seg(&mut lines, q, p, color);
                    }
                    prev = Some(p);
                }
            }
            // Dec parallels, with a tight ring near each pole (±85°) so the
            // converging RA lines close on a clean cap rather than a void.
            for d in [-88, -85, -60, -30, 0, 30, 60, 85, 88] {
                let dec = (d as f32).to_radians();
                let mut prev = None;
                for r in 0..=36 {
                    let p = rot * radec_unit(r as f32 / 36.0 * std::f32::consts::TAU, dec);
                    if let Some(q) = prev {
                        seg(&mut lines, q, p, color);
                    }
                    prev = Some(p);
                }
            }
        }

        if self.layers.ecliptic {
            let color = dim(ECLIPTIC_COLOR);
            let mut prev = None;
            for i in 0..=72 {
                let lon = i as f64 / 72.0 * std::f64::consts::TAU;
                let eq = ecliptic_to_equatorial(DVec3::new(lon.cos(), lon.sin(), 0.0));
                let p = Vec3::new(eq.x as f32, eq.y as f32, eq.z as f32);
                if let Some(q) = prev {
                    seg(&mut lines, q, p, color);
                }
                prev = Some(p);
            }
        }

        if self.layers.horizon {
            let color = dim(HORIZON_COLOR);
            let inv = self.sim.equatorial_to_horizon().transpose().as_mat3();
            let mut prev = None;
            for i in 0..=72 {
                let p = azalt_to_equatorial(inv, i as f64 / 72.0 * 360.0, 0.0);
                if let Some(q) = prev {
                    seg(&mut lines, q, p, color);
                }
                prev = Some(p);
            }
        }
        lines
    }

    /// Render units per System-view unit, chosen so the observer body's
    /// compressed slot equals the unit globe (radius 1): the central globe and
    /// the surrounding system then share one scale, with no size jump between
    /// them.
    fn system_scale(&self) -> f32 {
        1.0 / sphere_radius(self.sim.observer_body)
    }

    /// A body's position in the compressed System render space (equatorial),
    /// matching the System view's heliocentric layout.
    fn sys_render(&self, body: Body) -> Vec3 {
        let eq = ecliptic_to_equatorial(self.sim.heliocentric(body));
        let v = Vec3::new(eq.x as f32, eq.y as f32, eq.z as f32);
        v.normalize_or_zero() * scale(eq.length())
    }

    /// A body's rendered sphere radius in the Explorer frame (the observer's own
    /// body is the unit globe; the rest share the compressed system scale).
    fn body_render_radius(&self, body: Body) -> f32 {
        if body == self.sim.observer_body {
            1.0
        } else {
            sphere_radius(body) * self.system_scale()
        }
    }

    /// Local offset of a satellite from its parent in the Explorer frame: a tight
    /// faked orbit (true moon distances vanish at the compressed system scale)
    /// along the real bodycentric direction, a few parent-radii out so it reads
    /// as a close companion. Zero for a primary body. Works for any body's moons,
    /// and keeps an Earth-Moon pair separated no matter which one you stand on.
    fn local_offset(&self, body: Body) -> Vec3 {
        let Some(parent) = body.parent() else {
            return Vec3::ZERO;
        };
        let d = self.sim.bodycentric_equatorial(parent, body);
        let dir = Vec3::new(d.x as f32, d.y as f32, d.z as f32).normalize_or_zero();
        dir * self.body_render_radius(parent) * 3.0
    }

    /// A body's absolute Explorer-frame position before centring: its primary
    /// (parent for a moon, else itself) placed by the compressed heliocentric
    /// layout, plus the local moon offset.
    fn group_pos(&self, body: Body) -> Vec3 {
        let primary = body.parent().unwrap_or(body);
        self.sys_render(primary) * self.system_scale() + self.local_offset(body)
    }

    /// A body's position in the Explorer frame, centred on the observer's body.
    fn explorer_pos(&self, body: Body) -> Vec3 {
        self.group_pos(body) - self.group_pos(self.sim.observer_body)
    }

    /// Scene position of any non-observer body (planets at their system slot,
    /// moons on their faked satellite orbit).
    fn scene_sphere_pos(&self, body: Body) -> Option<Vec3> {
        (body != self.sim.observer_body).then(|| self.explorer_pos(body))
    }

    /// The surrounding solar system as real lit spheres, sharing the globe's
    /// depth buffer. Radii grow from zero with `factor` so the planets resolve
    /// into spheres as the camera pulls out (matching the fading billboards).
    fn system_draws(&self, factor: f32) -> Vec<BodyDraw> {
        let sys = self.system_scale();
        let sun_pos = self.explorer_pos(Body::Sun);
        let mut draws = Vec::with_capacity(Body::ALL.len());
        for body in Body::ALL {
            if body == self.sim.observer_body {
                continue;
            }
            let radius = sphere_radius(body) * sys * factor;
            if radius < 1.0e-4 {
                continue;
            }
            let pos = self.explorer_pos(body);
            let model = Mat4::from_translation(pos) * Mat4::from_scale(Vec3::splat(radius));
            draws.push(globe::draw_body_lit(
                body,
                model,
                (sun_pos - pos).normalize_or_zero(),
                DISTANT_NIGHT_SHADE,
            ));
        }
        draws
    }

    /// Orbit ellipses in the Explorer frame: the cached equatorial paths shifted
    /// so the observer's body is at the origin, then scaled to the unit globe.
    fn orbit_lines(&mut self, factor: f32) -> Vec<(Body, Vec<Vec3>)> {
        if factor <= 0.0 {
            return Vec::new();
        }
        let obs = self.sys_render(self.sim.observer_body);
        let sys = self.system_scale();
        self.orbits
            .equatorial_paths(self.sim)
            .iter()
            .map(|(body, points)| {
                let shifted = points.iter().map(|&p| (p - obs) * sys).collect();
                (*body, shifted)
            })
            .collect()
    }

    /// Where a body sits on screen for picking, labels and selection: its real
    /// sphere once the system has faded in, otherwise its billboard on the far
    /// shell. `None` for the observer's own body or a degenerate direction.
    fn body_scene_pos(&self, body: Body, factor: f32) -> Option<Vec3> {
        if body == self.sim.observer_body {
            return None;
        }
        if factor > 0.5 {
            return self.scene_sphere_pos(body);
        }
        Some(self.body_dir(body)? * BACKGROUND_RADIUS)
    }

    /// Equatorial-frame unit direction to a body as seen from the observer's
    /// body, or `None` for the observer's own body / a degenerate direction.
    fn body_dir(&self, body: Body) -> Option<Vec3> {
        if body == self.sim.observer_body {
            return None;
        }
        let d = self.sim.observer_direction_equatorial(body);
        let v = Vec3::new(d.x as f32, d.y as f32, d.z as f32);
        (v != Vec3::ZERO).then_some(v)
    }

    /// A single click selects whatever object is under the cursor; on bare ground
    /// (no object) at the surface it sets the observer's location, a capital
    /// marker snapping to that city. Never changes which body you stand on - that
    /// is a deliberate double-click.
    fn handle_single_click(
        &mut self,
        view_proj: Mat4,
        orientation: Mat3,
        pointer: Pos2,
        rect: Rect,
        factor: f32,
    ) {
        if let Some(sel) = self.pick_object(view_proj, rect, pointer, factor) {
            *self.selection = Some(sel);
        } else if self.camera.ground_factor() > 0.5 && factor < 0.5 {
            // Setting your location only happens while the globe is actually
            // visible AND fills a good part of the view (the orbit regime). On the
            // surface you are looking at the sky, and out in the system the globe
            // is a tiny disc whose limb would snap the observer to a pole - both
            // cases just deselect instead of moving the observer.
            if self.layers.capitals
                && self.sim.observer_body == Body::Earth
                && let Some((lat, lon)) = self.pick_capital(view_proj, orientation, pointer, rect)
            {
                self.sim.observer.latitude_deg = lat;
                self.sim.observer.longitude_deg = lon;
            } else if let Some((lat, lon)) = pick_globe(view_proj, orientation, pointer, rect) {
                self.sim.observer.latitude_deg = lat;
                self.sim.observer.longitude_deg = lon;
            }
        } else {
            *self.selection = None;
        }
    }

    /// Begin travelling to `body`: a brief warp fade hides the instant re-centre
    /// (the destination becomes the scene origin), then the camera settles into
    /// an orbit view of it. No-op if already standing there.
    fn start_travel(&mut self, ui: &egui::Ui, body: Body) {
        *self.selection = Some(Selection::Body(body));
        if body == self.sim.observer_body {
            self.camera.frame_orbit(ARRIVAL_RANGE);
            return;
        }
        let start = ui.input(|i| i.time);
        ui.data_mut(|d| {
            d.insert_temp(
                egui::Id::new("explorer_travel"),
                TravelFade {
                    target: body,
                    start,
                    swapped: false,
                },
            )
        });
    }

    /// Drive the warp fade: dim to black, swap the body you stand on at the peak
    /// (hiding the re-centre), then fade back in on the new world.
    fn animate_travel(&mut self, ui: &egui::Ui, rect: Rect) {
        let id = egui::Id::new("explorer_travel");
        let Some(mut tf) = ui.data(|d| d.get_temp::<TravelFade>(id)) else {
            return;
        };
        let now = ui.input(|i| i.time);
        let t = (((now - tf.start) / TRAVEL_SECS) as f32).clamp(0.0, 1.0);
        // Tent curve: 0 -> 1 at the midpoint -> 0.
        let alpha = 1.0 - (2.0 * t - 1.0).abs();
        if t >= 0.5 && !tf.swapped {
            self.sim.observer_body = tf.target;
            self.camera.frame_orbit(ARRIVAL_RANGE);
            tf.swapped = true;
            ui.data_mut(|d| d.insert_temp(id, tf.clone()));
        }
        ui.painter_at(rect).rect_filled(
            rect,
            0.0,
            Color32::from_black_alpha((alpha * 255.0) as u8),
        );
        if t >= 1.0 {
            ui.data_mut(|d| d.remove::<TravelFade>(id));
        } else {
            ui.ctx().request_repaint();
        }
    }

    /// Which leg of the continuum the camera is in, for the HUD.
    fn regime(&self, factor: f32) -> &'static str {
        if self.camera.orbit_factor() < 0.5 {
            "Surface"
        } else if factor < 0.5 {
            "Orbit"
        } else {
            "Solar System"
        }
    }

    /// Top-left box: the status readout (body, regime, location, season) plus the
    /// view options (standing-on, alignment, layers) behind a collapsing header.
    fn options_panel(&mut self, ui: &mut egui::Ui, rect: Rect, factor: f32) {
        const PANEL_W: f32 = 250.0;
        let regime = self.regime(factor);
        let accent = Color32::from_rgb(150, 185, 230);
        let muted = Color32::from_rgb(150, 162, 184);
        let orientation = self.sim.orientation().as_mat3();
        let sun = self.sim.observer_direction_equatorial(Body::Sun);
        let sun_dir = Vec3::new(sun.x as f32, sun.y as f32, sun.z as f32).normalize_or_zero();
        let subsolar = self.subsolar_latitude(orientation, sun_dir);

        egui::Area::new(ui.id().with("explorer_options"))
            .fixed_pos(rect.left_top() + vec2(12.0, 12.0))
            .constrain_to(rect)
            .show(ui.ctx(), |ui| {
                Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(PANEL_W);
                    ui.spacing_mut().item_spacing.y = 5.0;
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(self.sim.observer_body.name())
                                .size(22.0)
                                .strong(),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                egui::RichText::new(regime)
                                    .size(16.0)
                                    .strong()
                                    .color(accent),
                            );
                        });
                    });
                    egui::Grid::new("explorer_info")
                        .num_columns(2)
                        .spacing([10.0, 3.0])
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new("Location").color(muted));
                            ui.label(
                                egui::RichText::new(format!(
                                    "{:.2}°, {:.2}°",
                                    self.sim.observer.latitude_deg, self.sim.observer.longitude_deg
                                ))
                                .monospace(),
                            );
                            ui.end_row();
                            ui.label(egui::RichText::new("Subsolar lat").color(muted))
                                .on_hover_text("Latitude where the Sun is overhead (the season).");
                            ui.label(egui::RichText::new(format!("{subsolar:+.1}°")).monospace());
                            ui.end_row();
                        });
                    ui.separator();
                    egui::CollapsingHeader::new(
                        egui::RichText::new(format!("{} View options", icons::GEAR)).size(15.0),
                    )
                    .default_open(true)
                    .show(ui, |ui| self.options(ui, PANEL_W));
                });
            });
    }

    /// Game-style control hints, drawn large and centred along the bottom of the
    /// view so it is obvious what you can do at the current zoom.
    fn control_hints(&self, ui: &egui::Ui, rect: Rect, factor: f32) {
        let hint = match self.regime(factor) {
            "Surface" => {
                "Drag to look around    ·    Scroll to rise up    ·    Double-click a body to travel"
            }
            "Orbit" => {
                "Drag to orbit    ·    Scroll to zoom out to the system    ·    Click the globe to stand there    ·    Double-click a body to travel"
            }
            _ => {
                "Drag to orbit    ·    Scroll to zoom    ·    Double-click a planet to go stand on it"
            }
        };
        let painter = ui.painter_at(rect);
        let color = Color32::from_rgb(205, 216, 234);
        let galley = painter.layout_no_wrap(hint.to_owned(), FontId::proportional(14.5), color);
        let center = pos2(rect.center().x, rect.bottom() - 22.0);
        let bg = Rect::from_center_size(center, galley.size() + vec2(26.0, 12.0));
        painter.rect_filled(bg, 8.0, Color32::from_black_alpha(150));
        painter.galley(center - galley.size() * 0.5, galley, color);
    }

    /// Vertical zoom slider on the right edge: a labelled track from Surface up to
    /// the Solar System with a draggable thumb, making the scroll-to-traverse
    /// continuum discoverable and obvious at a glance.
    fn zoom_bar(&mut self, ui: &egui::Ui, rect: Rect) {
        let h = (rect.height() * 0.46).clamp(140.0, 260.0);
        let x = rect.right() - 26.0;
        let top = rect.center().y - h * 0.5;
        let bar = Rect::from_min_size(pos2(x, top), vec2(8.0, h));
        let resp = ui.interact(
            bar.expand2(vec2(12.0, 8.0)),
            ui.id().with("explorer_zoom"),
            Sense::click_and_drag(),
        );

        let (lo, hi) = UnifiedCamera::RANGE_LIMITS;
        let (lmin, lmax) = (lo.ln(), hi.ln());
        let t_of = |range: f32| ((range.ln() - lmin) / (lmax - lmin)).clamp(0.0, 1.0);

        if (resp.dragged() || resp.clicked())
            && let Some(p) = resp.interact_pointer_pos()
        {
            let tt = ((bar.bottom() - p.y) / h).clamp(0.0, 1.0);
            self.camera.set_range((lmin + tt * (lmax - lmin)).exp());
        }

        let painter = ui.painter_at(rect);
        let accent = Color32::from_rgb(150, 185, 230);
        let label_color = Color32::from_rgb(165, 180, 205);

        // Caption above the track so it reads as a zoom control, not a stray dot.
        painter.text(
            pos2(bar.center().x, bar.top() - 8.0),
            Align2::CENTER_BOTTOM,
            "ZOOM",
            FontId::proportional(11.0),
            label_color,
        );
        // The track.
        painter.rect_filled(bar, 4.0, Color32::from_black_alpha(150));
        painter.rect_stroke(
            bar,
            4.0,
            Stroke::new(1.0, Color32::from_rgb(70, 84, 110)),
            egui::StrokeKind::Inside,
        );
        // Region ticks + labels to the left of the track (it sits on the right edge).
        for (label, range) in [("Solar System", 250.0), ("Orbit", 4.0), ("Surface", 1.05)] {
            let y = bar.bottom() - t_of(range) * h;
            painter.line_segment(
                [pos2(bar.left(), y), pos2(bar.right(), y)],
                Stroke::new(1.0, Color32::from_rgb(90, 104, 130)),
            );
            painter.text(
                pos2(bar.left() - 8.0, y),
                Align2::RIGHT_CENTER,
                label,
                FontId::proportional(11.0),
                label_color,
            );
        }
        // A pill-shaped thumb that clearly invites dragging.
        let y = bar.bottom() - t_of(self.camera.range) * h;
        let thumb = Rect::from_center_size(pos2(bar.center().x, y), vec2(18.0, 11.0));
        painter.rect_filled(thumb, 4.0, accent);
        painter.rect_stroke(
            thumb,
            4.0,
            Stroke::new(1.0, Color32::from_rgb(235, 242, 252)),
            egui::StrokeKind::Inside,
        );
    }

    fn pick_object(
        &self,
        view_proj: Mat4,
        rect: Rect,
        pointer: Pos2,
        factor: f32,
    ) -> Option<Selection> {
        let eye = self.eye();
        let occlude = self.camera.ground_factor() > 0.4;
        let mut best: Option<(f32, Selection)> = None;
        let mut consider = |pos: Vec3, sel: Selection| {
            if occlude && occluded_by_globe(eye, pos) {
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
            if let Some(pos) = self.body_scene_pos(body, factor) {
                consider(pos, Selection::Body(body));
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

    #[allow(clippy::too_many_arguments)]
    fn draw_overlay(
        &self,
        ui: &egui::Ui,
        rect: Rect,
        view_proj: Mat4,
        orientation: Mat3,
        sun_dir: Vec3,
        factor: f32,
        orbit_lines: &[(Body, Vec<Vec3>)],
    ) {
        let painter = ui.painter_at(rect);
        let eye = self.eye();
        let ground = self.camera.ground_factor();

        // Globe-surface features (graticule, capitals, axis, observer ring, the
        // sub-solar point) only when the globe itself is visible AND we are still
        // near it (orbit view): they vanish in the planetarium surface view
        // (ground gone) and once the body shrinks to one sphere among many out in
        // the solar system (factor risen).
        if ground > 0.5 && factor < 0.5 {
            if self.layers.graticule {
                self.draw_graticule(&painter, rect, view_proj, orientation, eye);
            }
            if self.layers.axis {
                self.draw_axis(&painter, rect, view_proj, orientation, eye);
            }
            if self.layers.capitals && self.sim.observer_body == Body::Earth {
                self.draw_capitals(ui, &painter, rect, view_proj, orientation, eye);
            }
            if self.layers.special_latitudes {
                self.draw_special_latitudes(&painter, rect, view_proj, orientation, eye);
            }

            // Sub-solar point (where the Sun is overhead): a small sun glyph,
            // optional so it can be hidden.
            if self.layers.subsolar
                && !occluded_by_globe(eye, sun_dir)
                && let Some(p) = project(view_proj, sun_dir, rect)
            {
                painter.text(
                    p,
                    Align2::CENTER_CENTER,
                    icons::SUN,
                    FontId::proportional(17.0),
                    Color32::from_rgb(255, 214, 120),
                );
            }

            // Observer location: a map-pin glyph anchored at the surface point.
            let o = self.sim.observer.geocentric_fixed(
                self.sim.observer_body.equatorial_radius_km(),
                self.sim.observer_body.flattening(),
            );
            let local = Vec3::new(o.x as f32, o.y as f32, o.z as f32).normalize_or_zero();
            let world = orientation * local;
            if !occluded_by_globe(eye, world)
                && let Some(p) = project(view_proj, world, rect)
            {
                painter.text(
                    p,
                    Align2::CENTER_BOTTOM,
                    icons::MAP_PIN,
                    FontId::proportional(19.0),
                    Color32::from_rgb(255, 235, 120),
                );
            }

            if self.layers.alignment == GlobeAlignment::Orbital {
                self.draw_ecliptic_ring(&painter, rect, view_proj, eye);
            }
        }

        // Sky-dome markers (compass, constellation names, the viewing area) near
        // the surface and in orbit, gone once out in the system.
        if factor < 0.5 {
            if self.layers.horizon {
                self.draw_compass(&painter, rect, view_proj, eye);
            }
            if self.layers.constellations && self.layers.constellation_names {
                self.draw_constellation_names(&painter, rect, view_proj, eye);
            }
            if self.layers.view_area && self.sim.view.enabled {
                self.draw_view_patches(&painter, rect, view_proj, eye);
            }
        }

        if factor > 0.0 {
            self.draw_orbits(&painter, rect, view_proj, orbit_lines, factor);
        }

        if self.layers.labels {
            self.draw_labels(&painter, rect, view_proj, eye, factor);
        }
        self.draw_selection(&painter, rect, view_proj, eye, factor);
        self.identify(ui, &painter, rect, view_proj, eye, factor);
    }

    /// Draw the orbital (ecliptic) plane as a ring around the body, to show how
    /// its axis sits relative to the orbit. Only meaningful in orbital alignment.
    fn draw_ecliptic_ring(&self, painter: &egui::Painter, rect: Rect, view_proj: Mat4, eye: Vec3) {
        let obl = skynav::math::J2000_OBLIQUITY as f32;
        let (so, co) = obl.sin_cos();
        let color = Color32::from_rgb(120, 110, 70);
        const SEGS: usize = 96;
        const R: f32 = 1.3;
        let mut prev: Option<(Vec3, bool)> = None;
        for i in 0..=SEGS {
            let t = i as f32 / SEGS as f32 * std::f32::consts::TAU;
            let (st, ct) = t.sin_cos();
            // Ecliptic-plane point rotated into the equatorial J2000 frame.
            let world = Vec3::new(ct, st * co, st * so) * R;
            let vis = !occluded_by_globe(eye, world);
            if let Some((pw, pv)) = prev
                && pv
                && vis
                && let Some(seg) = project_segment(view_proj, pw, world, rect)
            {
                painter.line_segment(seg, Stroke::new(1.2, color));
            }
            prev = Some((world, vis));
        }
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
            let mut prev: Option<(Vec3, bool)> = None;
            for i in 0..=SEGS {
                let (lat, lon) = f(i as f64 / SEGS as f64);
                let world = orientation * itrs_unit(lat, lon);
                let vis = !occluded_by_globe(eye, world);
                if let Some((pw, pv)) = prev
                    && pv
                    && vis
                    && let Some(seg) = project_segment(view_proj, pw, world, rect)
                {
                    painter.line_segment(seg, Stroke::new(1.0, color));
                }
                prev = Some((world, vis));
            }
        };
        // Meridians (constant longitude), running almost all the way to the poles.
        for lon in (-180..180).step_by(30) {
            arc(&|t| (t * 176.0 - 88.0, lon as f64), faint);
        }
        // Parallels (constant latitude), equator emphasised, plus a tight ring
        // near each pole so the converging meridians close on a clean cap instead
        // of trailing off into nothing.
        for lat in [-88, -85, -60, -30, 0, 30, 60, 85, 88] {
            let color = if lat == 0 { equator } else { faint };
            arc(&|t| (lat as f64, t * 360.0 - 180.0), color);
        }
    }

    /// Tropics (the latitudes the Sun can stand directly over) and polar circles
    /// for the body you're on, derived from its axial tilt to the ecliptic.
    fn draw_special_latitudes(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        view_proj: Mat4,
        orientation: Mat3,
        eye: Vec3,
    ) {
        let pole = (orientation * Vec3::Z).normalize_or_zero();
        let tilt = pole
            .dot(ecliptic_north())
            .clamp(-1.0, 1.0)
            .acos()
            .to_degrees() as f64;
        if !(1.0..60.0).contains(&tilt) {
            return;
        }
        let tropic = Color32::from_rgb(200, 150, 70);
        let polar = Color32::from_rgb(90, 150, 200);
        let ring = |lat: f64, color: Color32| {
            const SEGS: usize = 64;
            let mut prev: Option<(Vec3, bool)> = None;
            for i in 0..=SEGS {
                let lon = i as f64 / SEGS as f64 * 360.0 - 180.0;
                let world = orientation * itrs_unit(lat, lon);
                let vis = !occluded_by_globe(eye, world);
                if let Some((pw, pv)) = prev
                    && pv
                    && vis
                    && let Some(seg) = project_segment(view_proj, pw, world, rect)
                {
                    painter.line_segment(seg, Stroke::new(1.2, color));
                }
                prev = Some((world, vis));
            }
        };
        ring(tilt, tropic);
        ring(-tilt, tropic);
        ring(90.0 - tilt, polar);
        ring(-(90.0 - tilt), polar);
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
            if let Some(seg) = project_segment(view_proj, base, tip, rect) {
                painter.line_segment(seg, Stroke::new(1.6, color));
                painter.text(
                    seg[1],
                    Align2::CENTER_CENTER,
                    name,
                    FontId::proportional(13.0),
                    color,
                );
            }
        }
    }

    /// Constellation names at each figure's centroid (equatorial frame).
    fn draw_constellation_names(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        view_proj: Mat4,
        eye: Vec3,
    ) {
        let color = Color32::from_rgb(110, 130, 165);
        for con in self.constellations {
            let mut sum = Vec3::ZERO;
            let mut n = 0.0;
            for poly in &con.lines {
                for v in poly {
                    sum += Vec3::from(*v);
                    n += 1.0;
                }
            }
            if n == 0.0 {
                continue;
            }
            let world = (sum / n).normalize_or_zero() * BACKGROUND_RADIUS;
            if !self.sky_occluded(eye, world)
                && let Some(p) = project(view_proj, world, rect)
            {
                painter.text(
                    p,
                    Align2::CENTER_CENTER,
                    con.full_name(),
                    FontId::proportional(11.0),
                    color,
                );
            }
        }
    }

    /// Cardinal-direction markers (N/E/S/W) on the local horizon.
    fn draw_compass(&self, painter: &egui::Painter, rect: Rect, view_proj: Mat4, eye: Vec3) {
        let inv = self.sim.equatorial_to_horizon().transpose().as_mat3();
        let color = Color32::from_rgb(120, 165, 120);
        for (label, az) in [("N", 0.0), ("E", 90.0), ("S", 180.0), ("W", 270.0)] {
            let world = azalt_to_equatorial(inv, az, 2.0) * BACKGROUND_RADIUS;
            if self.sky_occluded(eye, world) {
                continue;
            }
            if let Some(p) = project(view_proj, world, rect) {
                painter.text(
                    p,
                    Align2::CENTER_CENTER,
                    label,
                    FontId::proportional(15.0),
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

    /// Faded orbit ellipses of the surrounding planets (Explorer frame).
    fn draw_orbits(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        view_proj: Mat4,
        orbit_lines: &[(Body, Vec<Vec3>)],
        factor: f32,
    ) {
        let dim = (factor * 90.0) as u8;
        for (body, points) in orbit_lines {
            let color = if *self.selection == Some(Selection::Body(*body)) {
                Color32::from_rgba_unmultiplied(110, 150, 220, (factor * 200.0) as u8)
            } else {
                Color32::from_rgba_unmultiplied(70, 90, 130, dim)
            };
            let stroke = Stroke::new(1.0, color);
            let mut prev = points.last().copied();
            for &point in points {
                if let Some(p0) = prev
                    && let Some(seg) = project_segment(view_proj, p0, point, rect)
                {
                    painter.line_segment(seg, stroke);
                }
                prev = Some(point);
            }
        }
    }

    fn draw_labels(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        view_proj: Mat4,
        eye: Vec3,
        factor: f32,
    ) {
        for body in Body::ALL {
            if let Some(pos) = self.body_scene_pos(body, factor)
                && !self.sky_occluded(eye, pos)
                && let Some(p) = project(view_proj, pos, rect)
                && !self.label_overlaps_parent(body, p, view_proj, rect, factor)
            {
                label_at(painter, p, body.name(), Color32::from_rgb(200, 210, 230));
            }
        }
        if self.layers.stars {
            for star in self.stars {
                if star.magnitude > self.layers.mag_limit {
                    continue;
                }
                let pos = Vec3::from(star.unit) * BACKGROUND_RADIUS;
                if star.magnitude < 1.8
                    && !star.name.is_empty()
                    && !self.sky_occluded(eye, pos)
                    && let Some(p) = project(view_proj, pos, rect)
                {
                    label_at(painter, p, &star.name, Color32::from_rgb(170, 180, 200));
                }
            }
        }
    }

    /// Whether `body`'s label sits right on top of its parent's on screen (a
    /// moon seen from far away). Used to suppress the moon label so the parent
    /// name shows instead of stacking "Moon" over where "Earth" should read.
    fn label_overlaps_parent(
        &self,
        body: Body,
        screen: Pos2,
        view_proj: Mat4,
        rect: Rect,
        factor: f32,
    ) -> bool {
        const MERGE_SQ: f32 = 400.0;
        let Some(parent) = body.parent() else {
            return false;
        };
        self.body_scene_pos(parent, factor)
            .and_then(|pp| project(view_proj, pp, rect))
            .is_some_and(|parent_p| parent_p.distance_sq(screen) < MERGE_SQ)
    }

    fn selection_pos(&self, factor: f32) -> Option<Vec3> {
        match (*self.selection)? {
            Selection::Body(body) => self.body_scene_pos(body, factor),
            Selection::Star(i) => {
                if !self.layers.stars {
                    return None;
                }
                Some(Vec3::from(self.stars.get(i)?.unit) * BACKGROUND_RADIUS)
            }
        }
    }

    fn draw_selection(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        view_proj: Mat4,
        eye: Vec3,
        factor: f32,
    ) {
        let Some(pos) = self.selection_pos(factor) else {
            return;
        };
        if self.sky_occluded(eye, pos) {
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
        factor: f32,
    ) {
        let Some(pointer) = ui.input(|i| i.pointer.hover_pos()) else {
            return;
        };
        if !rect.contains(pointer) {
            return;
        }
        let occlude = self.camera.ground_factor() > 0.4;
        let mut best: Option<(f32, String)> = None;
        let mut consider = |pos: Vec3, name: String| {
            if occlude && occluded_by_globe(eye, pos) {
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
            if let Some(pos) = self.body_scene_pos(body, factor) {
                consider(pos, body.name().to_string());
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
            let mut prev: Option<(Vec3, bool)> = None;
            for (az, alt) in border {
                let world = azalt_to_equatorial(inv, az, alt) * BACKGROUND_RADIUS;
                let vis = !occluded_by_globe(eye, world);
                if let Some((pw, pv)) = prev
                    && pv
                    && vis
                    && let Some(seg) = project_segment(view_proj, pw, world, rect)
                {
                    painter.line_segment(seg, Stroke::new(1.5, accent));
                }
                prev = Some((world, vis));
            }
        }
    }

    /// The view-options controls (standing-on, alignment, layers), shown inside
    /// the combined control panel's collapsing section.
    fn options(&mut self, ui: &mut egui::Ui, width: f32) {
        ui.strong("Standing on");
        let before = self.sim.observer_body;
        egui::ComboBox::from_id_salt("globe_observer_body")
            .width(width)
            .selected_text(self.sim.observer_body.name())
            .show_ui(ui, |ui| {
                for body in Body::ALL {
                    ui.selectable_value(&mut self.sim.observer_body, body, body.name());
                }
            });
        // Hopping to a new home body eases the camera into an orbit view of it
        // (ready to zoom down to its surface).
        if self.sim.observer_body != before {
            self.camera.frame_orbit(ARRIVAL_RANGE);
            *self.selection = Some(Selection::Body(self.sim.observer_body));
        }
        // Alignment only governs how the globe is framed from orbit; on the
        // surface it has no effect, so it is hidden there to avoid confusion.
        if self.camera.orbit_factor() >= 0.5 {
            ui.separator();
            ui.strong("Alignment");
            egui::ComboBox::from_id_salt("globe_alignment")
                .width(width)
                .selected_text(match self.layers.alignment {
                    GlobeAlignment::Inertial => "Inertial (axis up)",
                    GlobeAlignment::Orbital => "Sun-relative",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.layers.alignment,
                        GlobeAlignment::Inertial,
                        "Inertial (axis up)",
                    )
                    .on_hover_text("The body's own spin axis points up.");
                    ui.selectable_value(
                        &mut self.layers.alignment,
                        GlobeAlignment::Orbital,
                        "Sun-relative",
                    )
                    .on_hover_text("Ecliptic up, Sun pinned: watch the axis nod over a year.");
                });
        }
        ui.separator();
        ui.strong("In the sky")
            .on_hover_text("Stars and reference lines drawn on the celestial sphere.");
        ui.checkbox(&mut self.layers.stars, "Stars");
        ui.checkbox(&mut self.layers.labels, "Object labels");
        ui.checkbox(&mut self.layers.constellations, "Constellation figures");
        ui.add_enabled(
            self.layers.constellations,
            egui::Checkbox::new(&mut self.layers.constellation_names, "Constellation names"),
        );
        ui.checkbox(&mut self.layers.ecliptic, "Ecliptic (Sun's path)");
        ui.checkbox(
            &mut self.layers.equatorial_grid,
            "Equatorial grid (RA / Dec)",
        );
        ui.checkbox(&mut self.layers.horizon, "Horizon & compass");
        ui.add(
            egui::Slider::new(&mut self.layers.mag_limit, 1.0..=6.5)
                .text("Faintest stars")
                .fixed_decimals(1),
        );
        ui.separator();
        ui.strong("On the globe")
            .on_hover_text("Markings drawn on the surface of the body you are at.");
        ui.checkbox(&mut self.layers.graticule, "Latitude / longitude grid");
        ui.checkbox(&mut self.layers.axis, "Rotation axis & poles");
        ui.checkbox(&mut self.layers.subsolar, "Subsolar point (Sun overhead)");
        ui.checkbox(
            &mut self.layers.special_latitudes,
            "Tropics & polar circles",
        )
        .on_hover_text("Latitudes the Sun can reach overhead, and the polar day/night circles.");
        ui.add_enabled(
            self.sim.observer_body == Body::Earth,
            egui::Checkbox::new(&mut self.layers.capitals, "Capital cities"),
        );
        ui.add_enabled(
            self.sim.view.enabled,
            egui::Checkbox::new(&mut self.layers.view_area, "Viewing area"),
        );
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

/// Equatorial-frame unit vector for a right ascension / declination (radians).
fn radec_unit(ra: f32, dec: f32) -> Vec3 {
    let (sd, cd) = dec.sin_cos();
    let (sr, cr) = ra.sin_cos();
    Vec3::new(cd * cr, cd * sr, sd)
}

/// Ecliptic north pole expressed in the equatorial J2000 frame.
fn ecliptic_north() -> Vec3 {
    let obl = skynav::math::J2000_OBLIQUITY as f32;
    Vec3::new(0.0, -obl.sin(), obl.cos())
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

    let hit = origin + dir * t;
    // Reject grazing limb hits: near the silhouette the surface normal is almost
    // perpendicular to the view ray, where a click lands on a wildly imprecise
    // latitude (it would snap the observer to a pole). Only accept reasonably
    // face-on points.
    let facing = hit.normalize().dot((origin - hit).normalize());
    if facing < 0.25 {
        return None;
    }

    let local = (orientation.transpose() * hit).normalize();
    let lat = (local.z as f64).clamp(-1.0, 1.0).asin().to_degrees();
    let lon = (local.y as f64).atan2(local.x as f64).to_degrees();
    Some((lat, lon))
}
