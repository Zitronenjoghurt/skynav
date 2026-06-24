//! Constellation figure lines, baked from d3-celestial data by `skynav-parse`
//! into `data/constellations.bin`. Vertices are unit directions in the
//! equatorial J2000 frame.

static DATA: &[u8] = include_bytes!("data/constellations.bin");

/// On-disk record baked by `skynav-parse`; shared so the encoder and decoder
/// stay in lockstep.
#[derive(bincode::Encode, bincode::Decode)]
pub struct ConstellationRecord {
    pub name: String,
    pub lines: Vec<Vec<[f32; 2]>>,
}

#[derive(Debug, Clone)]
pub struct Constellation {
    pub name: String,
    /// Polylines of equatorial J2000 unit-vector vertices.
    pub lines: Vec<Vec<[f32; 3]>>,
}

pub fn load() -> Vec<Constellation> {
    let config = bincode::config::standard();
    let (records, _): (Vec<ConstellationRecord>, usize) =
        bincode::decode_from_slice(DATA, config).unwrap_or_default();

    records
        .into_iter()
        .map(|c| Constellation {
            name: c.name,
            lines: c
                .lines
                .into_iter()
                .map(|polyline| polyline.into_iter().map(unit_from_radec).collect())
                .collect(),
        })
        .collect()
}

fn unit_from_radec([ra, dec]: [f32; 2]) -> [f32; 3] {
    let (ra, dec) = (ra as f64, dec as f64);
    let (sin_dec, cos_dec) = dec.sin_cos();
    let (sin_ra, cos_ra) = ra.sin_cos();
    [
        (cos_dec * cos_ra) as f32,
        (cos_dec * sin_ra) as f32,
        sin_dec as f32,
    ]
}
