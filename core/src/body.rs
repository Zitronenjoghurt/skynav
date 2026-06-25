use serde::{Deserialize, Serialize};

/// IAU/WGCCRE rotation elements defining a body's spin orientation: the north
/// pole direction in the ICRF equatorial (J2000) frame and the prime meridian
/// angle. Pole rates are per Julian century from J2000; the spin rate is per
/// day. Angles and rates are in degrees. Periodic nutation terms are omitted, so
/// these are the secular approximation (good for visualisation).
#[derive(Debug, Clone, Copy)]
pub struct RotationElements {
    pub ra0_deg: f64,
    pub ra0_rate: f64,
    pub dec0_deg: f64,
    pub dec0_rate: f64,
    pub w0_deg: f64,
    pub w_rate: f64,
}

/// A celestial body the simulation can locate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Body {
    Sun,
    Mercury,
    Venus,
    Earth,
    Moon,
    Mars,
    Jupiter,
    Saturn,
    Uranus,
    Neptune,
}

impl Body {
    pub const ALL: [Body; 10] = [
        Body::Sun,
        Body::Mercury,
        Body::Venus,
        Body::Earth,
        Body::Moon,
        Body::Mars,
        Body::Jupiter,
        Body::Saturn,
        Body::Uranus,
        Body::Neptune,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            Body::Sun => "Sun",
            Body::Mercury => "Mercury",
            Body::Venus => "Venus",
            Body::Earth => "Earth",
            Body::Moon => "Moon",
            Body::Mars => "Mars",
            Body::Jupiter => "Jupiter",
            Body::Saturn => "Saturn",
            Body::Uranus => "Uranus",
            Body::Neptune => "Neptune",
        }
    }

    /// Absolute magnitude H(1,0): the apparent magnitude the body would have at
    /// 1 AU from both the Sun and the observer, fully lit. Used to derive a
    /// realistic apparent brightness from any vantage point. Not meaningful for
    /// the Sun (handled separately as a self-luminous source).
    pub fn absolute_magnitude(&self) -> f64 {
        match self {
            Body::Sun => -26.74, // apparent at 1 AU; placeholder, handled specially
            Body::Mercury => -0.6,
            Body::Venus => -4.4,
            Body::Earth => -3.99,
            Body::Moon => 0.25,
            Body::Mars => -1.52,
            Body::Jupiter => -9.4,
            Body::Saturn => -8.9,
            Body::Uranus => -7.2,
            Body::Neptune => -6.9,
        }
    }

    /// Natural satellites of this body that the simulation models. Currently
    /// only Earth's Moon; structured so other moons slot in as they are added.
    pub fn satellites(&self) -> &'static [Body] {
        match self {
            Body::Earth => &[Body::Moon],
            _ => &[],
        }
    }

    /// The body this one orbits as a satellite (its parent), if any.
    pub fn parent(&self) -> Option<Body> {
        match self {
            Body::Moon => Some(Body::Earth),
            _ => None,
        }
    }

    /// Approximate sidereal orbital period in days - heliocentric for the
    /// planets, geocentric for the Moon. Zero for the Sun.
    pub fn orbital_period_days(&self) -> f64 {
        match self {
            Body::Sun => 0.0,
            Body::Mercury => 87.969,
            Body::Venus => 224.701,
            Body::Earth => 365.256,
            Body::Moon => 27.322,
            Body::Mars => 686.980,
            Body::Jupiter => 4_332.589,
            Body::Saturn => 10_759.22,
            Body::Uranus => 30_688.5,
            Body::Neptune => 60_182.0,
        }
    }

    /// Mean (volumetric) radius in kilometres.
    pub fn mean_radius_km(&self) -> f64 {
        match self {
            Body::Sun => 696_000.0,
            Body::Mercury => 2_439.7,
            Body::Venus => 6_051.8,
            Body::Earth => 6_371.0,
            Body::Moon => 1_737.4,
            Body::Mars => 3_389.5,
            Body::Jupiter => 69_911.0,
            Body::Saturn => 58_232.0,
            Body::Uranus => 25_362.0,
            Body::Neptune => 24_622.0,
        }
    }

    /// Equatorial radius in kilometres (the reference radius an observer stands
    /// on, paired with `flattening`).
    pub fn equatorial_radius_km(&self) -> f64 {
        match self {
            Body::Sun => 696_000.0,
            Body::Mercury => 2_440.53,
            Body::Venus => 6_051.8,
            Body::Earth => 6_378.137,
            Body::Moon => 1_737.4,
            Body::Mars => 3_396.19,
            Body::Jupiter => 71_492.0,
            Body::Saturn => 60_268.0,
            Body::Uranus => 25_559.0,
            Body::Neptune => 24_764.0,
        }
    }

    /// Polar flattening (f = (a - b) / a); 0 for a sphere.
    pub fn flattening(&self) -> f64 {
        match self {
            Body::Earth => 1.0 / 298.257_223_563,
            Body::Mars => 0.005_886,
            Body::Jupiter => 0.064_874,
            Body::Saturn => 0.097_962,
            Body::Uranus => 0.022_927,
            Body::Neptune => 0.017_081,
            _ => 0.0,
        }
    }

    /// IAU 2009/2015 WGCCRE rotation elements. Earth's are provided for
    /// completeness but the simulation uses the rigorous SOFA model for Earth;
    /// every other body's orientation is built from these.
    pub fn rotation_elements(&self) -> RotationElements {
        let (ra0_deg, ra0_rate, dec0_deg, dec0_rate, w0_deg, w_rate) = match self {
            Body::Sun => (286.13, 0.0, 63.87, 0.0, 84.176, 14.184_0),
            Body::Mercury => (281.0103, -0.0328, 61.4155, -0.0049, 329.5988, 6.138_510_8),
            Body::Venus => (272.76, 0.0, 67.16, 0.0, 160.20, -1.481_368_8),
            Body::Earth => (0.0, -0.641, 90.0, -0.557, 190.147, 360.985_623_5),
            Body::Moon => (269.9949, 0.0031, 66.5392, 0.0130, 38.3213, 13.176_358_1),
            Body::Mars => (
                317.681_43,
                -0.1061,
                52.886_50,
                -0.0609,
                176.630,
                350.891_982_26,
            ),
            Body::Jupiter => (
                268.056_595,
                -0.006_499,
                64.495_303,
                0.002_413,
                284.95,
                870.536_0,
            ),
            Body::Saturn => (40.589, -0.036, 83.537, -0.004, 38.90, 810.793_902_4),
            Body::Uranus => (257.311, 0.0, -15.175, 0.0, 203.81, -501.160_092_8),
            Body::Neptune => (299.36, 0.0, 43.46, 0.0, 249.978, 541.139_775_7),
        };
        RotationElements {
            ra0_deg,
            ra0_rate,
            dec0_deg,
            dec0_rate,
            w0_deg,
            w_rate,
        }
    }
}
