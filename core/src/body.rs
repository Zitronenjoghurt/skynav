use serde::{Deserialize, Serialize};

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
}
