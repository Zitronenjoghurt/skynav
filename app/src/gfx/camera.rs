use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};
use std::f32::consts::{PI, TAU};

/// Exponential approach rate for smooth camera fly-to (higher = snappier).
const EASE: f32 = 9.0;

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

    /// Smoothly pan to look top-down at `target` and zoom in to `distance`.
    pub fn frame(&mut self, target: Vec3, distance: f32) {
        self.goal_target = Some(target);
        self.goal = Some([self.yaw, 1.30, distance]);
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
                self.distance = (self.distance * (1.0 - scroll * 0.0015)).clamp(1.3, 60.0);
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
        // Far plane comfortably past the background star shell.
        Mat4::perspective_rh(60f32.to_radians(), aspect.max(0.01), 0.01, 1000.0)
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
