use crate::body::Body;
use crate::math::DMat3;
use crate::time::Epoch;

/// Body-fixed -> equatorial J2000 (GCRS/ICRF) orientation matrix at `epoch`.
///
/// Earth uses the rigorous IAU 2006/2000A model via SOFA (`earth::orientation_matrix`);
/// every other body is built from its IAU/WGCCRE rotation elements, which place
/// the spin axis and prime meridian in the same equatorial J2000 frame the rest
/// of the renderer uses.
pub fn body_orientation(body: Body, epoch: Epoch) -> DMat3 {
    if body == Body::Earth {
        return crate::earth::orientation_matrix(epoch);
    }

    let e = body.rotation_elements();
    let d = epoch.to_jde_tt_days() - 2_451_545.0;
    let t = d / 36_525.0;
    let ra0 = (e.ra0_deg + e.ra0_rate * t).to_radians();
    let dec0 = (e.dec0_deg + e.dec0_rate * t).to_radians();
    let w = (e.w0_deg + e.w_rate * d).to_radians();

    use std::f64::consts::FRAC_PI_2;
    DMat3::from_rotation_z(ra0 + FRAC_PI_2)
        * DMat3::from_rotation_x(FRAC_PI_2 - dec0)
        * DMat3::from_rotation_z(w)
}
