/// A geodetic observation site on a body's surface.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Observer {
    /// Geodetic latitude in degrees (north positive).
    pub latitude_deg: f64,
    /// Geodetic longitude in degrees (east positive).
    pub longitude_deg: f64,
    /// Height above the reference ellipsoid in metres.
    pub height_m: f64,
}

impl Observer {
    pub fn new(latitude_deg: f64, longitude_deg: f64, height_m: f64) -> Self {
        Self {
            latitude_deg,
            longitude_deg,
            height_m,
        }
    }

    pub fn latitude_rad(&self) -> f64 {
        self.latitude_deg.to_radians()
    }

    pub fn longitude_rad(&self) -> f64 {
        self.longitude_deg.to_radians()
    }

    /// Geocentric rectangular position in the body-fixed (ITRS) frame, in AU,
    /// on the WGS84 (Earth) ellipsoid.
    pub fn geocentric_itrs(&self) -> crate::math::DVec3 {
        self.geocentric_fixed(6378.137, 1.0 / 298.257_223_563)
    }

    /// Body-fixed rectangular position (AU) on an arbitrary reference ellipsoid,
    /// given its equatorial radius (km) and flattening. Generalises
    /// `geocentric_itrs` so an observer can stand on any body.
    pub fn geocentric_fixed(
        &self,
        equatorial_radius_km: f64,
        flattening: f64,
    ) -> crate::math::DVec3 {
        let e2 = flattening * (2.0 - flattening);

        let (sin_lat, cos_lat) = self.latitude_rad().sin_cos();
        let (sin_lon, cos_lon) = self.longitude_rad().sin_cos();
        let n = equatorial_radius_km / (1.0 - e2 * sin_lat * sin_lat).sqrt();
        let h = self.height_m / 1000.0;

        let xy = (n + h) * cos_lat;
        let km =
            crate::math::DVec3::new(xy * cos_lon, xy * sin_lon, (n * (1.0 - e2) + h) * sin_lat);
        km / crate::math::AU_KM
    }
}

impl Default for Observer {
    fn default() -> Self {
        Self::new(0.0, 0.0, 0.0)
    }
}
