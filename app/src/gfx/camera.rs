use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};
use std::f32::consts::{PI, TAU};

/// Exponential approach rate for smooth camera fly-to (higher = snappier). Kept
/// moderate so arrivals glide in rather than snapping/overshooting.
const EASE: f32 = 6.0;

/// Orbit camera looking at the origin, controlled by drag (rotate) and scroll
/// (zoom). Angles are in radians; distance is in model-radius units. A pending
/// `goal` is eased toward each frame for smooth fly-to.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct OrbitCamera {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    /// Point the camera orbits and looks at (lets it pan to a selected body).
    #[serde(default)]
    pub target: Vec3,
    #[serde(skip)]
    goal: Option<[f32; 3]>,
    #[serde(skip)]
    goal_target: Option<Vec3>,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self {
            yaw: 0.6,
            pitch: 0.4,
            distance: 3.2,
            target: Vec3::ZERO,
            goal: None,
            goal_target: None,
        }
    }
}

impl OrbitCamera {
    pub fn new(yaw: f32, pitch: f32, distance: f32) -> Self {
        Self {
            yaw,
            pitch,
            distance,
            target: Vec3::ZERO,
            goal: None,
            goal_target: None,
        }
    }

    /// Smoothly pan to `target` and zoom to `distance`, keeping the current
    /// orientation (no disorienting swing to top-down).
    pub fn frame(&mut self, target: Vec3, distance: f32) {
        self.goal_target = Some(target);
        self.goal = Some([self.yaw, self.pitch, distance]);
    }

    pub fn handle(&mut self, response: &egui::Response, ui: &egui::Ui) {
        if response.dragged() {
            let delta = response.drag_delta();
            self.yaw -= delta.x * 0.008;
            self.pitch = (self.pitch + delta.y * 0.008).clamp(-1.45, 1.45);
            self.goal = None;
            self.goal_target = None;
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                self.distance = (self.distance * (1.0 - scroll * 0.0015)).clamp(0.3, 600.0);
                self.goal = None;
                self.goal_target = None;
            }
        }
        self.advance(ui);
    }

    fn advance(&mut self, ui: &egui::Ui) {
        if self.goal.is_none() && self.goal_target.is_none() {
            return;
        }
        let dt = ui.input(|i| i.stable_dt).min(0.1);
        let k = 1.0 - (-EASE * dt).exp();
        if let Some(g) = self.goal {
            self.yaw = lerp_angle(self.yaw, g[0], k);
            self.pitch += (g[1] - self.pitch) * k;
            self.distance += (g[2] - self.distance) * k;
            if angle_diff(self.yaw, g[0]).abs() < 0.002 && (self.pitch - g[1]).abs() < 0.002 {
                self.yaw = g[0];
                self.pitch = g[1];
                self.distance = g[2];
                self.goal = None;
            }
        }
        if let Some(gt) = self.goal_target {
            self.target += (gt - self.target) * k;
            if self.target.distance(gt) < 0.01 {
                self.target = gt;
                self.goal_target = None;
            }
        }
        ui.ctx().request_repaint();
    }

    pub fn eye(&self) -> Vec3 {
        let (sp, cp) = self.pitch.sin_cos();
        let (sy, cy) = self.yaw.sin_cos();
        self.target + self.distance * Vec3::new(cp * cy, cp * sy, sp)
    }

    pub fn view(&self) -> Mat4 {
        Mat4::look_at_rh(self.eye(), self.target, Vec3::Z)
    }

    pub fn proj(&self, aspect: f32) -> Mat4 {
        // Far plane reaches the outer planets at the enlarged system scale.
        Mat4::perspective_rh(60f32.to_radians(), aspect.max(0.01), 0.01, 4000.0)
    }

    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        self.proj(aspect) * self.view()
    }
}

/// First-person camera at the observer, looking around the sky. `yaw` is the
/// view azimuth (from North, eastward) and `pitch` the altitude, both radians.
/// Operates in the East-North-Up frame.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LookAroundCamera {
    pub yaw: f32,
    pub pitch: f32,
    pub fov: f32,
    #[serde(skip)]
    goal: Option<[f32; 2]>,
}

impl Default for LookAroundCamera {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.35,
            fov: 75f32.to_radians(),
            goal: None,
        }
    }
}

impl LookAroundCamera {
    pub fn handle(&mut self, response: &egui::Response, ui: &egui::Ui) {
        if response.dragged() {
            let delta = response.drag_delta();
            let speed = self.fov * 0.0016;
            self.yaw -= delta.x * speed;
            self.pitch = (self.pitch + delta.y * speed).clamp(-1.5, 1.5);
            self.goal = None;
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                self.fov = (self.fov * (1.0 - scroll * 0.0015))
                    .clamp(8f32.to_radians(), 110f32.to_radians());
            }
        }
        self.advance(ui);
    }

    /// Smoothly turn to face azimuth/altitude (radians).
    pub fn look_at(&mut self, azimuth: f32, altitude: f32) {
        self.goal = Some([azimuth, altitude.clamp(-1.5, 1.5)]);
    }

    fn advance(&mut self, ui: &egui::Ui) {
        let Some(g) = self.goal else { return };
        let dt = ui.input(|i| i.stable_dt).min(0.1);
        let k = 1.0 - (-EASE * dt).exp();
        self.yaw = lerp_angle(self.yaw, g[0], k);
        self.pitch += (g[1] - self.pitch) * k;
        if angle_diff(self.yaw, g[0]).abs() < 0.002 && (self.pitch - g[1]).abs() < 0.002 {
            self.yaw = g[0];
            self.pitch = g[1];
            self.goal = None;
        }
        ui.ctx().request_repaint();
    }

    pub fn direction(&self) -> Vec3 {
        let (sp, cp) = self.pitch.sin_cos();
        let (sy, cy) = self.yaw.sin_cos();
        Vec3::new(cp * sy, cp * cy, sp)
    }

    pub fn view(&self) -> Mat4 {
        Mat4::look_at_rh(Vec3::ZERO, self.direction(), Vec3::Z)
    }

    pub fn proj(&self, aspect: f32) -> Mat4 {
        Mat4::perspective_rh(self.fov, aspect.max(0.01), 0.01, 10.0)
    }
}

/// Continuous surface-to-orbit-to-system camera, always framed on the body the
/// observer stands on (the body sits at the render origin). A single `range`
/// (eye distance from the body centre, in body radii; 1.0 = the surface) drives
/// a smooth transition: near the surface a first-person look-around, then an
/// orbit, then the whole solar system fades in around the shrinking globe.
/// Travelling to another body is done by switching which body the observer
/// stands on (the scene re-centres) and easing `range` back to an orbit view via
/// `frame_orbit`. Operates in the equatorial render frame.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct UnifiedCamera {
    /// Eye distance from the body centre, in body radii (1.0 = on the surface).
    pub range: f32,
    /// Surface look-around azimuth (from north, eastward) and altitude (radians).
    pub look_yaw: f32,
    pub look_pitch: f32,
    /// Orbit heading and elevation (radians) used away from the surface.
    pub orbit_yaw: f32,
    pub orbit_pitch: f32,
    /// Field of view (radians) while standing on the surface: scrolling here
    /// magnifies the sky (telescope) rather than lifting off, only rising once
    /// fully zoomed back out.
    #[serde(default = "default_surface_fov")]
    pub surface_fov: f32,
    /// Pending eased range goal (used when arriving at a freshly chosen body).
    #[serde(skip)]
    goal_range: Option<f32>,
}

fn default_surface_fov() -> f32 {
    SURFACE_FOV_WIDE
}

/// Range below which the camera is a pure surface look-around.
const SURFACE_RANGE: f32 = 1.6;
/// Range above which the camera is a pure orbit.
const ORBIT_RANGE: f32 = 3.6;
/// Range at which the surrounding solar system starts to fade in (still framed
/// on the body), and the range by which it has fully taken over.
const SYSTEM_NEAR: f32 = 8.0;
const SYSTEM_FAR: f32 = 40.0;
/// Furthest the camera can pull back, in body radii: enough to frame the whole
/// compressed solar system even from a small body (large system scale), while
/// staying inside the background star shell.
const MAX_RANGE: f32 = 12_000.0;
/// Surface field-of-view bounds (radians): the widest naked-eye view and the
/// tightest telescope zoom.
const SURFACE_FOV_WIDE: f32 = 1.30; // ~75 degrees
const SURFACE_FOV_NARROW: f32 = 0.21; // ~12 degrees
/// Field of view once fully in orbit (radians, ~57 degrees).
const ORBIT_FOV: f32 = 1.0;
/// Range at/below which scrolling magnifies the sky instead of changing altitude.
const SURFACE_ZOOM_RANGE: f32 = 1.003;

impl Default for UnifiedCamera {
    fn default() -> Self {
        Self {
            range: 3.2,
            look_yaw: 0.0,
            look_pitch: 0.45,
            orbit_yaw: 0.6,
            orbit_pitch: 0.4,
            surface_fov: SURFACE_FOV_WIDE,
            goal_range: None,
        }
    }
}

impl UnifiedCamera {
    /// 0.0 on the surface (look-around), 1.0 in orbit; smooth in between.
    pub fn orbit_factor(&self) -> f32 {
        smoothstep(SURFACE_RANGE, ORBIT_RANGE, self.range)
    }

    /// 0.0 close to the body, 1.0 once the surrounding solar system (Sun +
    /// planets as real spheres) has fully faded in. Drives the scale-morph from a
    /// single globe out to the whole system.
    pub fn system_factor(&self) -> f32 {
        smoothstep(SYSTEM_NEAR, SYSTEM_FAR, self.range)
    }

    /// 1.0 when the body's globe is shown, easing to 0.0 the instant you touch
    /// down so the ground fades away and you get a clean planetarium sky (no
    /// pixelated surface underfoot). Only the last sliver of descent fades.
    pub fn ground_factor(&self) -> f32 {
        smoothstep(1.002, 1.7, self.range)
    }

    /// Smoothly ease the zoom to `range` body radii (keeping orientation). Used
    /// when the observer hops to another body, so the new globe is framed.
    pub fn frame_orbit(&mut self, range: f32) {
        self.goal_range = Some(range.clamp(1.002, MAX_RANGE));
    }

    /// Lowest and highest eye distance (body radii) the camera reaches; used to
    /// place and drive the on-screen zoom indicator.
    pub const RANGE_LIMITS: (f32, f32) = (1.002, MAX_RANGE);

    /// Set the zoom directly (e.g. from the zoom bar), cancelling any pending ease.
    pub fn set_range(&mut self, range: f32) {
        self.range = range.clamp(Self::RANGE_LIMITS.0, Self::RANGE_LIMITS.1);
        self.goal_range = None;
    }

    pub fn handle(&mut self, response: &egui::Response, ui: &egui::Ui) {
        let s = self.orbit_factor();
        if response.dragged() {
            let delta = response.drag_delta();
            // Near the surface drag looks around the sky (speed scaled to the
            // telescope zoom); far out it orbits.
            if s < 0.5 {
                let speed = self.surface_fov * 0.004;
                self.look_yaw -= delta.x * speed;
                self.look_pitch = (self.look_pitch + delta.y * speed).clamp(-1.5, 1.5);
            } else {
                self.orbit_yaw -= delta.x * 0.008;
                self.orbit_pitch = (self.orbit_pitch + delta.y * 0.008).clamp(-1.45, 1.45);
            }
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                let f = 1.0 - scroll * 0.0015;
                // On the surface, scrolling magnifies the sky (telescope) and only
                // lifts off once zoomed back out to the widest field of view.
                let widen_done = self.surface_fov >= SURFACE_FOV_WIDE - 1.0e-3;
                if self.range <= SURFACE_ZOOM_RANGE && (scroll > 0.0 || !widen_done) {
                    self.surface_fov =
                        (self.surface_fov * f).clamp(SURFACE_FOV_NARROW, SURFACE_FOV_WIDE);
                } else {
                    self.range = (self.range * f).clamp(1.002, MAX_RANGE);
                }
                self.goal_range = None; // manual zoom cancels the arrival ease
            }
        }
        self.advance(ui);
    }

    fn advance(&mut self, ui: &egui::Ui) {
        let Some(goal) = self.goal_range else { return };
        let dt = ui.input(|i| i.stable_dt).min(0.1);
        let k = 1.0 - (-EASE * dt).exp();
        self.range += (goal - self.range) * k;
        if (self.range - goal).abs() < 0.01 {
            self.range = goal;
            self.goal_range = None;
        }
        ui.ctx().request_repaint();
    }

    /// East/North/Up basis at the observer's surface point (equatorial frame).
    fn enu(surface: Vec3, pole: Vec3) -> (Vec3, Vec3, Vec3) {
        let up = surface.normalize_or_zero();
        let east = pole.cross(up).normalize_or_zero();
        let north = up.cross(east);
        (east, north, up)
    }

    /// Orbit direction (centre -> eye) for the current heading/pitch, orbiting
    /// around `up_axis` with `reference` as the yaw = 0 direction. Everything is
    /// in the equatorial render frame, so the camera, occlusion and overlays all
    /// agree (the previous separate "align" matrix broke this for non-Earth).
    fn orbit_dir(&self, up_axis: Vec3, reference: Vec3) -> Vec3 {
        let up = up_axis.normalize_or_zero();
        let mut right = reference - up * reference.dot(up);
        if right.length_squared() < 1e-8 {
            right = up.cross(Vec3::X);
            if right.length_squared() < 1e-8 {
                right = up.cross(Vec3::Y);
            }
        }
        let right = right.normalize_or_zero();
        let fwd = up.cross(right);
        let (sp, cp) = self.orbit_pitch.sin_cos();
        let (sy, cy) = self.orbit_yaw.sin_cos();
        (right * (cp * cy) + fwd * (cp * sy) + up * sp).normalize_or_zero()
    }

    /// Eye position in the equatorial render frame. `surface` is the observer's
    /// surface point (unit); `orbit_up`/`orbit_ref` define the orbit framing.
    pub fn eye(&self, surface: Vec3, orbit_up: Vec3, orbit_ref: Vec3) -> Vec3 {
        let s = self.orbit_factor();
        let up = surface.normalize_or_zero();
        let orbit_dir = self.orbit_dir(orbit_up, orbit_ref);
        // As we descend the eye direction swings to sit above the observer, so
        // the bottom of the zoom seamlessly becomes "standing on your spot".
        slerp_dir(up, orbit_dir, s) * self.range
    }

    pub fn view(&self, surface: Vec3, pole: Vec3, orbit_up: Vec3, orbit_ref: Vec3) -> Mat4 {
        let s = self.orbit_factor();
        let (east, north, up) = Self::enu(surface, pole);
        let eye = self.eye(surface, orbit_up, orbit_ref);

        let (sp, cp) = self.look_pitch.sin_cos();
        let (sy, cy) = self.look_yaw.sin_cos();
        let forward_surface = (north * cp * cy + east * cp * sy + up * sp).normalize_or_zero();
        let forward_orbit = (-eye).normalize_or_zero();
        let forward = slerp_dir(forward_surface, forward_orbit, s);

        let up_view = slerp_dir(up, orbit_up.normalize_or_zero(), s);
        Mat4::look_at_rh(eye, eye + forward, up_view)
    }

    pub fn proj(&self, aspect: f32) -> Mat4 {
        let s = self.orbit_factor();
        // Surface uses the (telescope-zoomable) field of view, easing to a fixed
        // orbit FOV as you rise.
        let fov = self.surface_fov + (ORBIT_FOV - self.surface_fov) * s;
        // Near tracks the eye distance so depth precision holds across the huge
        // surface-to-system range, while staying under the ~0.002 surface gap
        // when seated on the globe. Far reaches past the star shell so the whole
        // solar system sorts correctly against the background.
        let near = (self.range * 0.001).clamp(0.001, 50.0);
        Mat4::perspective_rh(fov, aspect.max(0.01), near, 40_000.0)
    }
}

/// Smooth Hermite interpolation, returning 0 below `lo` and 1 above `hi`.
fn smoothstep(lo: f32, hi: f32, x: f32) -> f32 {
    let t = ((x - lo) / (hi - lo)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Shortest-arc interpolation between two unit directions.
fn slerp_dir(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    let a = a.normalize_or_zero();
    let b = b.normalize_or_zero();
    let d = a.dot(b).clamp(-1.0, 1.0);
    if d > 0.9995 {
        return a.lerp(b, t).normalize_or_zero();
    }
    let theta = d.acos() * t;
    let rel = (b - a * d).normalize_or_zero();
    (a * theta.cos() + rel * theta.sin()).normalize_or_zero()
}

/// Signed shortest angular difference `b - a`, wrapped to (-π, π].
fn angle_diff(a: f32, b: f32) -> f32 {
    let mut d = (b - a) % TAU;
    if d > PI {
        d -= TAU;
    } else if d < -PI {
        d += TAU;
    }
    d
}

fn lerp_angle(a: f32, b: f32, t: f32) -> f32 {
    a + angle_diff(a, b) * t
}
