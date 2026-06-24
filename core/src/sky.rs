use crate::math::DVec3;
use crate::observer::Observer;
use crate::time::Epoch;
use std::f64::consts::FRAC_PI_2;

/// Observed horizontal coordinates: azimuth (from North, eastward) and altitude
/// above the horizon, both in radians.
#[derive(Debug, Clone, Copy)]
pub struct Horizontal {
    pub azimuth: f64,
    pub altitude: f64,
}

impl Horizontal {
    /// East-North-Up unit direction.
    pub fn enu(&self) -> DVec3 {
        let (sa, ca) = self.altitude.sin_cos();
        let (saz, caz) = self.azimuth.sin_cos();
        DVec3::new(ca * saz, ca * caz, sa)
    }

    pub fn azimuth_deg(&self) -> f64 {
        self.azimuth.to_degrees().rem_euclid(360.0)
    }

    pub fn altitude_deg(&self) -> f64 {
        self.altitude.to_degrees()
    }
}

/// Rigorous ICRS (J2000 RA/Dec, radians) → observed horizontal coordinates via
/// the IAU SOFA chain (precession, nutation, aberration, refraction).
///
/// Diurnal parallax is not applied, so the Moon can be off by up to ~1°; bodies
/// are otherwise sub-arcminute. Standard atmosphere is assumed for refraction.
pub fn observed(ra: f64, dec: f64, epoch: Epoch, observer: &Observer) -> Option<Horizontal> {
    let (y, m, d, h, mi, s, ns) = epoch.to_gregorian_utc();
    let second = s as f64 + ns as f64 / 1.0e9;
    let (utc1, utc2) =
        sofars::ts::dtf2d("UTC", y, m as i32, d as i32, h as i32, mi as i32, second).ok()?;

    let (aob, zob, _hob, _dob, _rob, _eo) = sofars::astro::atco13(
        ra,
        dec,
        0.0,
        0.0,
        0.0,
        0.0,
        utc1,
        utc2,
        0.0,
        observer.longitude_rad(),
        observer.latitude_rad(),
        observer.height_m,
        0.0,
        0.0,
        1013.25,
        15.0,
        0.5,
        0.55,
    )
    .ok()?;

    Some(Horizontal {
        azimuth: aob,
        altitude: FRAC_PI_2 - zob,
    })
}
