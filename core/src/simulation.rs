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
    pub view: ViewWindow,
    ephemeris: Box<dyn Ephemeris>,
}

impl Simulation {
    pub fn new(epoch: Epoch) -> Self {
        Self {
            clock: SimClock::new(epoch),
            observer: Observer::default(),
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

    /// Observer's geocentric position in the equatorial J2000 frame (AU).
    pub fn observer_equatorial(&self) -> DVec3 {
        self.earth_orientation() * self.observer.geocentric_itrs()
    }

    /// Rotation mapping an equatorial J2000 unit vector to the observer's
    /// East-North-Up frame. A fast per-frame transform for the whole star field
    /// (no refraction); use `observed_*` for rigorous per-object placement.
    pub fn equatorial_to_horizon(&self) -> DMat3 {
        let gcrs_to_itrs = self.earth_orientation().transpose();
        let (sin_lat, cos_lat) = self.observer.latitude_rad().sin_cos();
        let (sin_lon, cos_lon) = self.observer.longitude_rad().sin_cos();
        let east = DVec3::new(-sin_lon, cos_lon, 0.0);
        let north = DVec3::new(-sin_lat * cos_lon, -sin_lat * sin_lon, cos_lat);
        let up = DVec3::new(cos_lat * cos_lon, cos_lat * sin_lon, sin_lat);
        DMat3::from_cols(east, north, up).transpose() * gcrs_to_itrs
    }

    /// Observed horizontal coordinates of an object at ICRS `ra`/`dec` (radians).
    pub fn observed(&self, ra: f64, dec: f64) -> Option<Horizontal> {
        sky::observed(ra, dec, self.clock.epoch, &self.observer)
    }

    /// Observed horizontal coordinates of a Solar System body, including
    /// topocentric (diurnal) parallax - significant for the Moon.
    pub fn observed_body(&self, body: Body) -> Option<Horizontal> {
        let topocentric = self.geocentric_equatorial(body) - self.observer_equatorial();
        let (ra, dec) = equatorial_radec(topocentric);
        self.observed(ra, dec)
    }

    /// Observed horizontal coordinates of a catalogued star.
    pub fn observed_star(&self, star: &Star) -> Option<Horizontal> {
        self.observed(star.ra, star.dec)
    }

    /// Geometric altitude (radians, no refraction) of a body at an arbitrary
    /// epoch - a fast path for scanning rise/set events.
    pub fn geometric_altitude_at(&self, body: Body, epoch: Epoch) -> f64 {
        let (ra, dec) = equatorial_radec(self.geocentric_equatorial_at(body, epoch));
        let lst = crate::earth::earth_rotation_angle(epoch) + self.observer.longitude_rad();
        let hour_angle = lst - ra;
        let lat = self.observer.latitude_rad();
        (dec.sin() * lat.sin() + dec.cos() * lat.cos() * hour_angle.cos())
            .clamp(-1.0, 1.0)
            .asin()
    }

    /// Geometric azimuth and altitude in degrees (no refraction) of a body at an
    /// arbitrary epoch. Azimuth is measured from North, increasing eastward.
    /// Used to test events against the mapped viewing area.
    pub fn horizontal_at(&self, body: Body, epoch: Epoch) -> (f64, f64) {
        let (ra, dec) = equatorial_radec(self.geocentric_equatorial_at(body, epoch));
        let lst = crate::earth::earth_rotation_angle(epoch) + self.observer.longitude_rad();
        let (sh, ch) = (lst - ra).sin_cos();
        let (sd, cd) = dec.sin_cos();
        let (sphi, cphi) = self.observer.latitude_rad().sin_cos();
        let alt = (sphi * sd + cphi * cd * ch).clamp(-1.0, 1.0).asin();
        let az = (-cd * sh).atan2(sd * cphi - cd * sphi * ch).rem_euclid(TAU);
        (az.to_degrees(), alt.to_degrees())
    }
}
