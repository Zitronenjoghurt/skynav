pub use glam::{DMat3, DQuat, DVec3};

/// One astronomical unit in kilometres (IAU 2012 definition).
pub const AU_KM: f64 = 149_597_870.7;

/// Mean obliquity of the ecliptic at J2000.0 (84381.406″) in radians.
pub const J2000_OBLIQUITY: f64 = 0.409_092_600_600_582_9;

/// Rotate a unit-agnostic vector from the ecliptic J2000 frame into the
/// equatorial J2000 frame.
pub fn ecliptic_to_equatorial(v: DVec3) -> DVec3 {
    DMat3::from_rotation_x(J2000_OBLIQUITY) * v
}

/// Rotate a vector from the equatorial J2000 frame into the ecliptic J2000 frame.
pub fn equatorial_to_ecliptic(v: DVec3) -> DVec3 {
    DMat3::from_rotation_x(-J2000_OBLIQUITY) * v
}

/// Convert spherical (longitude, latitude, radius) to a rectangular vector.
pub fn spherical_to_rect(lon: f64, lat: f64, radius: f64) -> DVec3 {
    let cl = lat.cos();
    DVec3::new(
        radius * cl * lon.cos(),
        radius * cl * lon.sin(),
        radius * lat.sin(),
    )
}

/// Right ascension and declination (radians) of an equatorial vector.
pub fn equatorial_radec(v: DVec3) -> (f64, f64) {
    let r = v.length();
    let ra = v.y.atan2(v.x).rem_euclid(std::f64::consts::TAU);
    let dec = if r > 0.0 { (v.z / r).asin() } else { 0.0 };
    (ra, dec)
}
