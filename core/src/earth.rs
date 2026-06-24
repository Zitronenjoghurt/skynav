use crate::math::DMat3;
use crate::time::Epoch;
use std::f64::consts::TAU;

/// IAU 2000 Earth Rotation Angle in radians (UT1 approximated by UTC).
///
/// The angle of the prime meridian measured eastward from the equinox.
pub fn earth_rotation_angle(epoch: Epoch) -> f64 {
    let tu = epoch.to_jde_utc_days() - 2_451_545.0;
    let theta = TAU * (0.779_057_273_264_0 + 1.002_737_811_911_354_6 * tu);
    theta.rem_euclid(TAU)
}

/// Body-fixed (ITRS) → equatorial GCRS rotation: the full IAU 2006/2000A
/// orientation (precession, nutation, Earth rotation) via SOFA `c2t06a`.
///
/// UT1 is approximated by UTC and polar motion is omitted (both sub-arcsecond).
pub fn orientation_matrix(epoch: Epoch) -> DMat3 {
    let tt = epoch.to_jde_tt_days();
    let ut1 = epoch.to_jde_utc_days();
    // c2t06a returns the GCRS→ITRS matrix (row-major); reading it column-major
    // yields its transpose, i.e. the ITRS→GCRS rotation we want for rendering.
    let m = sofars::pnp::c2t06a(tt, 0.0, ut1, 0.0, 0.0, 0.0);
    DMat3::from_cols_array_2d(&m)
}
