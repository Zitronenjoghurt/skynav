//! The observer's mapped viewing area: one or more sky patches (an azimuth
//! window with an altitude floor and ceiling) plus a limiting magnitude. When
//! enabled it gates what the Visible and Events views report and is drawn over
//! the Sky view, letting the user match the simulation to what they can really
//! see (obstructions, light pollution).

use serde::{Deserialize, Serialize};

/// A rectangular sky patch in horizontal coordinates (degrees). Azimuth is
/// measured from North, increasing eastward; a patch where `az_min > az_max`
/// wraps across North.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Patch {
    pub az_min_deg: f64,
    pub az_max_deg: f64,
    pub alt_min_deg: f64,
    pub alt_max_deg: f64,
}

impl Patch {
    /// A patch spanning the whole sky above the horizon.
    pub fn full() -> Self {
        Self {
            az_min_deg: 0.0,
            az_max_deg: 360.0,
            alt_min_deg: 0.0,
            alt_max_deg: 90.0,
        }
    }

    pub fn contains(&self, az_deg: f64, alt_deg: f64) -> bool {
        if alt_deg < self.alt_min_deg || alt_deg > self.alt_max_deg {
            return false;
        }
        let az = az_deg.rem_euclid(360.0);
        let (lo, hi) = (
            self.az_min_deg.rem_euclid(360.0),
            self.az_max_deg.rem_euclid(360.0),
        );
        // A full sweep (or any patch a full turn wide) covers every azimuth.
        if (self.az_max_deg - self.az_min_deg).abs() >= 360.0 {
            return true;
        }
        if lo <= hi {
            az >= lo && az <= hi
        } else {
            az >= lo || az <= hi
        }
    }
}

/// The whole mapped viewing area.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewWindow {
    pub enabled: bool,
    /// Faintest star magnitude still visible from this site.
    pub limiting_magnitude: f32,
    pub patches: Vec<Patch>,
}

impl Default for ViewWindow {
    fn default() -> Self {
        Self {
            enabled: false,
            limiting_magnitude: 6.5,
            patches: vec![Patch::full()],
        }
    }
}

impl ViewWindow {
    /// Whether a horizontal direction falls inside the mapped area. When the
    /// window is disabled nothing is gated, so this is always `true`.
    pub fn contains(&self, az_deg: f64, alt_deg: f64) -> bool {
        if !self.enabled {
            return true;
        }
        self.patches.iter().any(|p| p.contains(az_deg, alt_deg))
    }

    /// Whether a star of `magnitude` at this direction would be seen: inside the
    /// area and bright enough for the limiting magnitude.
    pub fn star_visible(&self, magnitude: f32, az_deg: f64, alt_deg: f64) -> bool {
        if !self.enabled {
            return true;
        }
        magnitude <= self.limiting_magnitude && self.contains(az_deg, alt_deg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_wraps_across_north() {
        let p = Patch {
            az_min_deg: 350.0,
            az_max_deg: 20.0,
            alt_min_deg: 0.0,
            alt_max_deg: 90.0,
        };
        assert!(p.contains(0.0, 10.0));
        assert!(p.contains(355.0, 10.0));
        assert!(p.contains(15.0, 10.0));
        assert!(!p.contains(180.0, 10.0));
    }

    #[test]
    fn patch_respects_altitude_band() {
        let p = Patch {
            az_min_deg: 0.0,
            az_max_deg: 360.0,
            alt_min_deg: 20.0,
            alt_max_deg: 60.0,
        };
        assert!(p.contains(123.0, 40.0));
        assert!(!p.contains(123.0, 10.0));
        assert!(!p.contains(123.0, 80.0));
    }

    #[test]
    fn disabled_window_gates_nothing() {
        let w = ViewWindow::default();
        assert!(w.contains(123.0, -45.0));
        assert!(w.star_visible(20.0, 123.0, -45.0));
    }
}
