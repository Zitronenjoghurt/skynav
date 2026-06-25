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
    /// IAU three-letter abbreviation (e.g. "Ori"), as baked from the source data.
    pub name: String,
    /// Polylines of equatorial J2000 unit-vector vertices.
    pub lines: Vec<Vec<[f32; 3]>>,
}

impl Constellation {
    /// The full proper name (e.g. "Orion"), resolved from the IAU abbreviation.
    /// Falls back to the abbreviation itself if it is not recognised.
    pub fn full_name(&self) -> &str {
        full_name(&self.name)
    }
}

/// Map an IAU three-letter constellation abbreviation to its full name.
pub fn full_name(abbrev: &str) -> &str {
    match abbrev {
        "And" => "Andromeda",
        "Ant" => "Antlia",
        "Aps" => "Apus",
        "Aqr" => "Aquarius",
        "Aql" => "Aquila",
        "Ara" => "Ara",
        "Ari" => "Aries",
        "Aur" => "Auriga",
        "Boo" => "Boötes",
        "Cae" => "Caelum",
        "Cam" => "Camelopardalis",
        "Cnc" => "Cancer",
        "CVn" => "Canes Venatici",
        "CMa" => "Canis Major",
        "CMi" => "Canis Minor",
        "Cap" => "Capricornus",
        "Car" => "Carina",
        "Cas" => "Cassiopeia",
        "Cen" => "Centaurus",
        "Cep" => "Cepheus",
        "Cet" => "Cetus",
        "Cha" => "Chamaeleon",
        "Cir" => "Circinus",
        "Col" => "Columba",
        "Com" => "Coma Berenices",
        "CrA" => "Corona Australis",
        "CrB" => "Corona Borealis",
        "Crv" => "Corvus",
        "Crt" => "Crater",
        "Cru" => "Crux",
        "Cyg" => "Cygnus",
        "Del" => "Delphinus",
        "Dor" => "Dorado",
        "Dra" => "Draco",
        "Equ" => "Equuleus",
        "Eri" => "Eridanus",
        "For" => "Fornax",
        "Gem" => "Gemini",
        "Gru" => "Grus",
        "Her" => "Hercules",
        "Hor" => "Horologium",
        "Hya" => "Hydra",
        "Hyi" => "Hydrus",
        "Ind" => "Indus",
        "Lac" => "Lacerta",
        "Leo" => "Leo",
        "LMi" => "Leo Minor",
        "Lep" => "Lepus",
        "Lib" => "Libra",
        "Lup" => "Lupus",
        "Lyn" => "Lynx",
        "Lyr" => "Lyra",
        "Men" => "Mensa",
        "Mic" => "Microscopium",
        "Mon" => "Monoceros",
        "Mus" => "Musca",
        "Nor" => "Norma",
        "Oct" => "Octans",
        "Oph" => "Ophiuchus",
        "Ori" => "Orion",
        "Pav" => "Pavo",
        "Peg" => "Pegasus",
        "Per" => "Perseus",
        "Phe" => "Phoenix",
        "Pic" => "Pictor",
        "Psc" => "Pisces",
        "PsA" => "Piscis Austrinus",
        "Pup" => "Puppis",
        "Pyx" => "Pyxis",
        "Ret" => "Reticulum",
        "Sge" => "Sagitta",
        "Sgr" => "Sagittarius",
        "Sco" => "Scorpius",
        "Scl" => "Sculptor",
        "Sct" => "Scutum",
        "Ser" => "Serpens",
        "Sex" => "Sextans",
        "Tau" => "Taurus",
        "Tel" => "Telescopium",
        "Tri" => "Triangulum",
        "TrA" => "Triangulum Australe",
        "Tuc" => "Tucana",
        "UMa" => "Ursa Major",
        "UMi" => "Ursa Minor",
        "Vel" => "Vela",
        "Vir" => "Virgo",
        "Vol" => "Volans",
        "Vul" => "Vulpecula",
        other => other,
    }
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
