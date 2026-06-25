use crate::body::Body;
use crate::catalog::Star;
use crate::ephemeris::{AnalyticEphemeris, Ephemeris};
use crate::math::{DMat3, DVec3, ecliptic_to_equatorial, equatorial_radec};
use crate::observer::Observer;
use crate::sky::{self, Horizontal};
use crate::time::{Epoch, SimClock};
use crate::view::ViewWindow;
use hifitime::Duration;
use std::f64::consts::{PI, TAU};

/// Top-level simulation state the frontend drives: a clock, an ephemeris
/// provider and the active observer. Display code reads positions from here.
pub struct Simulation {
    pub clock: SimClock,
    pub observer: Observer,
    /// The body the observer stands on (Earth by default). Drives the observed
    /// sky, the body-fixed frame and the rendered globe.
    pub observer_body: Body,
    pub view: ViewWindow,
    ephemeris: Box<dyn Ephemeris>,
}

impl Simulation {
    pub fn new(epoch: Epoch) -> Self {
        Self {
            clock: SimClock::new(epoch),
            observer: Observer::default(),
            observer_body: Body::Earth,
            view: ViewWindow::default(),
            ephemeris: Box::new(AnalyticEphemeris),
        }
    }

    pub fn with_ephemeris(mut self, ephemeris: Box<dyn Ephemeris>) -> Self {
        self.ephemeris = ephemeris;
        self
    }

    /// Heliocentric position in the ecliptic J2000 frame (AU).
    pub fn heliocentric(&self, body: Body) -> DVec3 {
        self.ephemeris.position(body, self.clock.epoch)
    }

    /// Heliocentric position (AU, ecliptic J2000) at an arbitrary epoch - used to
    /// trace true orbital paths.
    pub fn heliocentric_at(&self, body: Body, epoch: Epoch) -> DVec3 {
        self.ephemeris.position(body, epoch)
    }

    /// Trace one full orbital period of `body`, returning `samples` heliocentric
    /// points (AU, ecliptic J2000). The path is a closed loop the body rides
    /// exactly, so the orbit line and the body marker never disagree.
    pub fn orbit_path(&self, body: Body, samples: usize) -> Vec<DVec3> {
        let period = body.orbital_period_days();
        (0..samples)
            .map(|i| {
                let frac = i as f64 / samples as f64;
                let epoch = self.clock.epoch + Duration::from_days(frac * period);
                self.ephemeris.position(body, epoch)
            })
            .collect()
    }

    /// Geocentric position in the ecliptic J2000 frame (AU).
    pub fn geocentric(&self, body: Body) -> DVec3 {
        self.heliocentric(body) - self.heliocentric(Body::Earth)
    }

    /// Geocentric position in the equatorial J2000 frame (AU).
    pub fn geocentric_equatorial(&self, body: Body) -> DVec3 {
        ecliptic_to_equatorial(self.geocentric(body))
    }

    /// Unit direction from Earth to `body` in the equatorial J2000 frame.
    pub fn direction_equatorial(&self, body: Body) -> DVec3 {
        self.geocentric_equatorial(body).normalize_or_zero()
    }

    /// Position of `body` relative to `from` in the ecliptic J2000 frame (AU).
    /// Generalises `geocentric` to an arbitrary central body.
    pub fn bodycentric(&self, from: Body, body: Body) -> DVec3 {
        self.heliocentric(body) - self.heliocentric(from)
    }

    /// Position of `body` relative to `from` in the equatorial J2000 frame (AU).
    pub fn bodycentric_equatorial(&self, from: Body, body: Body) -> DVec3 {
        ecliptic_to_equatorial(self.bodycentric(from, body))
    }

    /// Unit direction from the observer's body to `body` (equatorial J2000).
    pub fn observer_direction_equatorial(&self, body: Body) -> DVec3 {
        self.bodycentric_equatorial(self.observer_body, body)
            .normalize_or_zero()
    }

    /// Body-fixed -> equatorial J2000 orientation of the observer's body.
    pub fn orientation(&self) -> DMat3 {
        crate::rotation::body_orientation(self.observer_body, self.clock.epoch)
    }

    /// Position of `body` relative to `from` (equatorial J2000, AU) at an
    /// arbitrary epoch.
    pub fn bodycentric_equatorial_at(&self, from: Body, body: Body, epoch: Epoch) -> DVec3 {
        let p = self.ephemeris.position(body, epoch) - self.ephemeris.position(from, epoch);
        ecliptic_to_equatorial(p)
    }

    /// The observer's local up direction in the equatorial J2000 frame at an
    /// arbitrary epoch (uses the observer body's rotation).
    fn observer_up_at(&self, epoch: Epoch) -> DVec3 {
        let (sin_lat, cos_lat) = self.observer.latitude_rad().sin_cos();
        let (sin_lon, cos_lon) = self.observer.longitude_rad().sin_cos();
        let up_fixed = DVec3::new(cos_lat * cos_lon, cos_lat * sin_lon, sin_lat);
        (crate::rotation::body_orientation(self.observer_body, epoch) * up_fixed)
            .normalize_or_zero()
    }

    /// Geocentric equatorial J2000 position (AU) of a body at an arbitrary epoch.
    pub fn geocentric_equatorial_at(&self, body: Body, epoch: Epoch) -> DVec3 {
        ecliptic_to_equatorial(self.geocentric_at(body, epoch))
    }

    /// Geocentric ecliptic J2000 position (AU) of a body at an arbitrary epoch.
    pub fn geocentric_at(&self, body: Body, epoch: Epoch) -> DVec3 {
        self.ephemeris.position(body, epoch) - self.ephemeris.position(Body::Earth, epoch)
    }

    /// Angular separation (radians) between two bodies as seen from Earth at an
    /// arbitrary epoch.
    pub fn separation_at(&self, a: Body, b: Body, epoch: Epoch) -> f64 {
        let u = self.geocentric_equatorial_at(a, epoch).normalize_or_zero();
        let v = self.geocentric_equatorial_at(b, epoch).normalize_or_zero();
        u.dot(v).clamp(-1.0, 1.0).acos()
    }

    /// Angular distance (radians) of a body from the Sun as seen from Earth.
    pub fn elongation_at(&self, body: Body, epoch: Epoch) -> f64 {
        self.separation_at(body, Body::Sun, epoch)
    }

    /// Apparent visual magnitude of `body` as seen from the observer's body
    /// (lower = brighter). Accounts for distance to the Sun, distance to the
    /// observer, and a crude linear phase darkening, so the same body is bright
    /// up close and faint from far away. The Sun is treated as self-luminous.
    pub fn apparent_magnitude(&self, body: Body) -> f64 {
        let from = self.heliocentric(self.observer_body);
        let pos = self.heliocentric(body);
        let delta = (pos - from).length().max(1e-6);
        if body == Body::Sun {
            // Inverse-square dimming of a fixed luminosity relative to 1 AU.
            return -26.74 + 5.0 * delta.log10();
        }
        let r = pos.length().max(1e-6); // body-to-Sun distance (AU)
        // Phase angle at the body, between the directions to the Sun and to the
        // observer: 0 = fully lit facing us, 180 = back-lit.
        let to_sun = (-pos).normalize_or_zero();
        let to_obs = (from - pos).normalize_or_zero();
        let phase_deg = to_sun.dot(to_obs).clamp(-1.0, 1.0).acos().to_degrees();
        body.absolute_magnitude() + 5.0 * (r * delta).log10() + 0.02 * phase_deg
    }

    /// Moon illumination: fraction of the disc lit (0..1) and whether it is
    /// waxing (growing) rather than waning.
    pub fn moon_illumination(&self) -> (f64, bool) {
        let sun = self.geocentric(Body::Sun);
        let moon = self.geocentric(Body::Moon);
        let to_sun = (sun - moon).normalize_or_zero();
        let to_earth = (-moon).normalize_or_zero();
        let fraction = (1.0 + to_sun.dot(to_earth)) / 2.0;
        let elongation = (moon.y.atan2(moon.x) - sun.y.atan2(sun.x)).rem_euclid(TAU);
        (fraction, elongation < PI)
    }

    /// Earth Rotation Angle (radians) at the current epoch.
    pub fn earth_rotation_angle(&self) -> f64 {
        crate::earth::earth_rotation_angle(self.clock.epoch)
    }

    /// Body-fixed → equatorial J2000 orientation matrix at the current epoch.
    pub fn earth_orientation(&self) -> DMat3 {
        crate::earth::orientation_matrix(self.clock.epoch)
    }

    /// Observer's position relative to its body's centre in the equatorial J2000
    /// frame (AU), on that body's reference ellipsoid.
    pub fn observer_equatorial(&self) -> DVec3 {
        let fixed = self.observer.geocentric_fixed(
            self.observer_body.equatorial_radius_km(),
            self.observer_body.flattening(),
        );
        self.orientation() * fixed
    }

    /// Rotation mapping an equatorial J2000 unit vector to the observer's
    /// East-North-Up frame. A fast per-frame transform for the whole star field
    /// (no refraction); use `observed_*` for rigorous per-object placement.
    pub fn equatorial_to_horizon(&self) -> DMat3 {
        self.equatorial_to_horizon_at(self.clock.epoch)
    }

    /// Observed horizontal coordinates of an object at ICRS `ra`/`dec` (radians).
    /// Earth uses the rigorous SOFA chain (refraction, aberration); on any other
    /// body it falls back to a geometric transform (no atmosphere modelled).
    pub fn observed(&self, ra: f64, dec: f64) -> Option<Horizontal> {
        if self.observer_body == Body::Earth {
            sky::observed(ra, dec, self.clock.epoch, &self.observer)
        } else {
            Some(self.geometric_horizontal(ra, dec))
        }
    }

    /// Geometric horizontal coordinates of an equatorial-J2000 RA/Dec via the
    /// observer's body-fixed frame (no refraction). Used for non-Earth bodies.
    fn geometric_horizontal(&self, ra: f64, dec: f64) -> Horizontal {
        let (sd, cd) = dec.sin_cos();
        let (sr, cr) = ra.sin_cos();
        let enu = self.equatorial_to_horizon() * DVec3::new(cd * cr, cd * sr, sd);
        Horizontal {
            azimuth: enu.x.atan2(enu.y),
            altitude: enu.z.clamp(-1.0, 1.0).asin(),
        }
    }

    /// Observed horizontal coordinates of a Solar System body, including
    /// topocentric (diurnal) parallax - significant for the Moon. Positions are
    /// relative to the body the observer stands on.
    pub fn observed_body(&self, body: Body) -> Option<Horizontal> {
        let topocentric =
            self.bodycentric_equatorial(self.observer_body, body) - self.observer_equatorial();
        let (ra, dec) = equatorial_radec(topocentric);
        self.observed(ra, dec)
    }

    /// Observed horizontal coordinates of a catalogued star.
    pub fn observed_star(&self, star: &Star) -> Option<Horizontal> {
        self.observed(star.ra, star.dec)
    }

    /// Geometric altitude (radians, no refraction) of a body at an arbitrary
    /// epoch - a fast path for scanning rise/set events. Relative to the observer
    /// body: Earth uses the quick Earth-rotation-angle formula; other bodies use
    /// their IAU rotation (still cheap - no SOFA).
    pub fn geometric_altitude_at(&self, body: Body, epoch: Epoch) -> f64 {
        if self.observer_body == Body::Earth {
            let (ra, dec) = equatorial_radec(self.geocentric_equatorial_at(body, epoch));
            let lst = crate::earth::earth_rotation_angle(epoch) + self.observer.longitude_rad();
            let hour_angle = lst - ra;
            let lat = self.observer.latitude_rad();
            return (dec.sin() * lat.sin() + dec.cos() * lat.cos() * hour_angle.cos())
                .clamp(-1.0, 1.0)
                .asin();
        }
        let dir = self
            .bodycentric_equatorial_at(self.observer_body, body, epoch)
            .normalize_or_zero();
        dir.dot(self.observer_up_at(epoch)).clamp(-1.0, 1.0).asin()
    }

    /// Geometric azimuth and altitude in degrees (no refraction) of a body at an
    /// arbitrary epoch. Azimuth is measured from North, increasing eastward.
    /// Used to test events against the mapped viewing area. Relative to the
    /// observer body (Earth uses the quick rotation-angle formula).
    pub fn horizontal_at(&self, body: Body, epoch: Epoch) -> (f64, f64) {
        if self.observer_body == Body::Earth {
            let (ra, dec) = equatorial_radec(self.geocentric_equatorial_at(body, epoch));
            let lst = crate::earth::earth_rotation_angle(epoch) + self.observer.longitude_rad();
            let (sh, ch) = (lst - ra).sin_cos();
            let (sd, cd) = dec.sin_cos();
            let (sphi, cphi) = self.observer.latitude_rad().sin_cos();
            let alt = (sphi * sd + cphi * cd * ch).clamp(-1.0, 1.0).asin();
            let az = (-cd * sh).atan2(sd * cphi - cd * sphi * ch).rem_euclid(TAU);
            return (az.to_degrees(), alt.to_degrees());
        }
        let dir = self
            .bodycentric_equatorial_at(self.observer_body, body, epoch)
            .normalize_or_zero();
        let enu = self.equatorial_to_horizon_at(epoch) * dir;
        let az = enu.x.atan2(enu.y).rem_euclid(TAU);
        let alt = enu.z.clamp(-1.0, 1.0).asin();
        (az.to_degrees(), alt.to_degrees())
    }

    /// Equatorial J2000 -> observer East-North-Up rotation at an arbitrary epoch.
    fn equatorial_to_horizon_at(&self, epoch: Epoch) -> DMat3 {
        let gcrs_to_itrs = crate::rotation::body_orientation(self.observer_body, epoch).transpose();
        let (sin_lat, cos_lat) = self.observer.latitude_rad().sin_cos();
        let (sin_lon, cos_lon) = self.observer.longitude_rad().sin_cos();
        let east = DVec3::new(-sin_lon, cos_lon, 0.0);
        let north = DVec3::new(-sin_lat * cos_lon, -sin_lat * sin_lon, cos_lat);
        let up = DVec3::new(cos_lat * cos_lon, cos_lat * sin_lon, sin_lat);
        DMat3::from_cols(east, north, up).transpose() * gcrs_to_itrs
    }
}
