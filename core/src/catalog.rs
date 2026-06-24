//! Star catalogue, baked from the HYG database by `skynav-parse` into
//! `data/stars.bin` (records limited to magnitude 6.5).

static STAR_DATA: &[u8] = include_bytes!("data/stars.bin");

/// On-disk record baked by `skynav-parse`; shared so the encoder and decoder
/// stay in lockstep.
#[derive(bincode::Encode, bincode::Decode)]
pub struct StarRecord {
    pub ra: f32,
    pub dec: f32,
    pub mag: f32,
    pub ci: f32,
    pub name: String,
}

/// A catalogued star at the J2000.0 epoch.
#[derive(Debug, Clone)]
pub struct Star {
    /// Right ascension at J2000.0 in radians.
    pub ra: f64,
    /// Declination at J2000.0 in radians.
    pub dec: f64,
    pub magnitude: f32,
    /// Approximate RGB colour from the B-V index.
    pub color: [f32; 3],
    /// Display name (proper name, Bayer/Flamsteed designation, or HR number).
    pub name: String,
    /// Unit direction in the equatorial J2000 frame.
    pub unit: [f32; 3],
}

/// Load every catalogued star.
pub fn load_stars() -> Vec<Star> {
    let config = bincode::config::standard();
    let (records, _): (Vec<StarRecord>, usize) =
        bincode::decode_from_slice(STAR_DATA, config).unwrap_or_default();

    records
        .into_iter()
        .map(|r| {
            let (ra, dec) = (r.ra as f64, r.dec as f64);
            let (sin_dec, cos_dec) = dec.sin_cos();
            let (sin_ra, cos_ra) = ra.sin_cos();
            Star {
                ra,
                dec,
                magnitude: r.mag,
                color: bv_to_rgb(r.ci),
                name: r.name,
                unit: [
                    (cos_dec * cos_ra) as f32,
                    (cos_dec * sin_ra) as f32,
                    sin_dec as f32,
                ],
            }
        })
        .collect()
}

/// Approximate star colour from the B-V index via a small gradient.
fn bv_to_rgb(bv: f32) -> [f32; 3] {
    const STOPS: &[(f32, [f32; 3])] = &[
        (-0.4, [0.61, 0.70, 1.00]),
        (0.0, [0.79, 0.86, 1.00]),
        (0.4, [1.00, 1.00, 0.96]),
        (0.8, [1.00, 0.95, 0.82]),
        (1.2, [1.00, 0.86, 0.62]),
        (1.6, [1.00, 0.74, 0.48]),
        (2.0, [1.00, 0.64, 0.42]),
    ];
    let bv = bv.clamp(STOPS[0].0, STOPS[STOPS.len() - 1].0);
    for pair in STOPS.windows(2) {
        let (lo, a) = pair[0];
        let (hi, b) = pair[1];
        if bv <= hi {
            let t = (bv - lo) / (hi - lo);
            return [
                a[0] + (b[0] - a[0]) * t,
                a[1] + (b[1] - a[1]) * t,
                a[2] + (b[2] - a[2]) * t,
            ];
        }
    }
    STOPS[STOPS.len() - 1].1
}
