use crate::body::Body;
use crate::math::{AU_KM, DVec3, spherical_to_rect};
use crate::time::Epoch;

/// Source of celestial body positions.
///
/// The analytic provider is the default. A JPL DE provider (via a pure-Rust
/// SPK reader) can be slotted in behind this same trait for higher accuracy.
pub trait Ephemeris {
    /// Heliocentric position of `body` in the ecliptic J2000 frame, in AU.
    fn position(&self, body: Body, epoch: Epoch) -> DVec3;
}

/// Analytic ephemeris: VSOP87A for the planets and Earth, a truncated lunar
/// theory for the Moon. Self-contained and WASM-friendly.
#[derive(Debug, Default, Clone, Copy)]
pub struct AnalyticEphemeris;

impl Ephemeris for AnalyticEphemeris {
    fn position(&self, body: Body, epoch: Epoch) -> DVec3 {
        let jde = epoch.to_jde_tdb_days();
        match body {
            Body::Sun => DVec3::ZERO,
            Body::Moon => earth_position(jde) + geocentric_moon(jde),
            other => planet_position(other, jde),
        }
    }
}

fn earth_position(jde: f64) -> DVec3 {
    let c = vsop87::vsop87a::earth(jde);
    DVec3::new(c.x, c.y, c.z)
}

fn planet_position(body: Body, jde: f64) -> DVec3 {
    use vsop87::vsop87a;
    let c = match body {
        Body::Mercury => vsop87a::mercury(jde),
        Body::Venus => vsop87a::venus(jde),
        Body::Earth => vsop87a::earth(jde),
        Body::Mars => vsop87a::mars(jde),
        Body::Jupiter => vsop87a::jupiter(jde),
        Body::Saturn => vsop87a::saturn(jde),
        Body::Uranus => vsop87a::uranus(jde),
        Body::Neptune => vsop87a::neptune(jde),
        Body::Sun | Body::Moon => unreachable!("handled by Ephemeris::position"),
    };
    DVec3::new(c.x, c.y, c.z)
}

/// Geocentric position of the Moon in the ecliptic frame, in AU.
///
/// Truncated ELP (Meeus) referred to the equinox of date; the frame error
/// versus J2000 is negligible at this accuracy. Upgrade target: ELP/MPP02.
fn geocentric_moon(jde: f64) -> DVec3 {
    let (ecl, distance_km) = astro::lunar::geocent_ecl_pos(jde);
    spherical_to_rect(ecl.long, ecl.lat, distance_km / AU_KM)
}
