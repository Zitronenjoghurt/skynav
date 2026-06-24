//! Bakes the raw HYG star catalogue and constellation line data (in `assets/`,
//! see the project README) into compact bincode blobs under `core/src/data/`.
//!
//! Run from the workspace root: `cargo run -p skynav-parse`.

use skynav::catalog::StarRecord;
use skynav::constellations::ConstellationRecord;
use std::error::Error;
use std::fs;

const MAG_LIMIT: f32 = 6.5;

fn main() -> Result<(), Box<dyn Error>> {
    fs::create_dir_all("core/src/data")?;
    let cfg = bincode::config::standard();

    let stars = parse_stars()?;
    println!("stars: {}", stars.len());
    fs::write(
        "core/src/data/stars.bin",
        bincode::encode_to_vec(&stars, cfg)?,
    )?;

    let constellations = parse_constellations()?;
    println!("constellations: {}", constellations.len());
    fs::write(
        "core/src/data/constellations.bin",
        bincode::encode_to_vec(&constellations, cfg)?,
    )?;

    Ok(())
}

fn parse_stars() -> Result<Vec<StarRecord>, Box<dyn Error>> {
    let mut rdr = csv::Reader::from_path("assets/hyg.csv")?;
    let headers = rdr.headers()?.clone();
    let col = |name: &str| headers.iter().position(|h| h == name).expect(name);
    let (i_ra, i_dec, i_mag, i_ci) = (col("ra"), col("dec"), col("mag"), col("ci"));
    let (i_proper, i_bayer, i_flam, i_con, i_hr) = (
        col("proper"),
        col("bayer"),
        col("flam"),
        col("con"),
        col("hr"),
    );

    let mut stars = Vec::new();
    for record in rdr.records() {
        let r = record?;
        let mag: f32 = r[i_mag].parse().unwrap_or(99.0);
        if mag > MAG_LIMIT {
            continue;
        }
        let proper = r[i_proper].trim();
        if proper == "Sol" {
            continue;
        }

        let ra_hours: f64 = r[i_ra].parse().unwrap_or(0.0);
        let dec_deg: f64 = r[i_dec].parse().unwrap_or(0.0);
        let ci: f32 = r[i_ci].parse().unwrap_or(0.5);

        stars.push(StarRecord {
            ra: (ra_hours * 15.0).to_radians() as f32,
            dec: dec_deg.to_radians() as f32,
            mag,
            ci,
            name: star_name(
                proper,
                r[i_bayer].trim(),
                r[i_flam].trim(),
                r[i_con].trim(),
                r[i_hr].trim(),
            ),
        });
    }
    Ok(stars)
}

fn star_name(proper: &str, bayer: &str, flam: &str, con: &str, hr: &str) -> String {
    if !proper.is_empty() {
        proper.to_string()
    } else if !bayer.is_empty() {
        format!("{} {con}", greek(bayer))
    } else if !flam.is_empty() {
        format!("{flam} {con}")
    } else if !hr.is_empty() {
        format!("HR {hr}")
    } else {
        String::new()
    }
}

fn greek(bayer: &str) -> String {
    let split = bayer
        .find(|c: char| c.is_ascii_digit())
        .unwrap_or(bayer.len());
    let (letters, suffix) = bayer.split_at(split);
    let symbol = match letters {
        "Alp" => "α",
        "Bet" => "β",
        "Gam" => "γ",
        "Del" => "δ",
        "Eps" => "ε",
        "Zet" => "ζ",
        "Eta" => "η",
        "The" => "θ",
        "Iot" => "ι",
        "Kap" => "κ",
        "Lam" => "λ",
        "Mu" => "μ",
        "Nu" => "ν",
        "Xi" => "ξ",
        "Omi" => "ο",
        "Pi" => "π",
        "Rho" => "ρ",
        "Sig" => "σ",
        "Tau" => "τ",
        "Ups" => "υ",
        "Phi" => "φ",
        "Chi" => "χ",
        "Psi" => "ψ",
        "Ome" => "ω",
        other => other,
    };
    format!("{symbol}{suffix}")
}

fn parse_constellations() -> Result<Vec<ConstellationRecord>, Box<dyn Error>> {
    let json: serde_json::Value =
        serde_json::from_reader(fs::File::open("assets/constellations.lines.json")?)?;
    let mut out = Vec::new();
    for feature in json["features"].as_array().ok_or("no features")? {
        let name = feature["id"].as_str().unwrap_or_default().to_string();
        let mut lines = Vec::new();
        for segment in feature["geometry"]["coordinates"]
            .as_array()
            .ok_or("no coordinates")?
        {
            let mut polyline = Vec::new();
            for point in segment.as_array().ok_or("bad segment")? {
                let ra_deg = point[0].as_f64().unwrap_or(0.0);
                let dec_deg = point[1].as_f64().unwrap_or(0.0);
                polyline.push([ra_deg.to_radians() as f32, dec_deg.to_radians() as f32]);
            }
            lines.push(polyline);
        }
        out.push(ConstellationRecord { name, lines });
    }
    Ok(out)
}
