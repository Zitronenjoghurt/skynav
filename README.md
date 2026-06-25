# skynav

[![CI](https://github.com/Zitronenjoghurt/skynav/actions/workflows/ci.yml/badge.svg)](https://github.com/Zitronenjoghurt/skynav/actions/workflows/ci.yml)
![Lines of Code](http://tokei.lemon.industries/b1/github/Zitronenjoghurt/skynav?category=code&type=Rust&logo=https://simpleicons.org/icons/rust.svg)

An application for viewing Earth's current position and orientation in space relative to the Sun and other celestial objects.

## Data

The star catalogue and constellation figures are baked into compact blobs under
`core/src/data/` by the `skynav-parse` tool. To regenerate, download the raw
sources into `assets/` and run the parser:

```sh
mkdir -p assets
curl -L -o assets/hyg.csv.gz https://raw.githubusercontent.com/astronexus/HYG-Database/main/hyg/CURRENT/hygdata_v40.csv.gz && gunzip -f assets/hyg.csv.gz
curl -L -o assets/constellations.lines.json https://raw.githubusercontent.com/ofrohn/d3-celestial/master/data/constellations.lines.json
cargo run -p skynav-parse
```

- Stars: [HYG database](https://github.com/astronexus/HYG-Database) (CC BY-SA), limited to magnitude 6.5.
- Constellation lines: [d3-celestial](https://github.com/ofrohn/d3-celestial) (BSD).

The Earth texture is NASA Blue Marble (public domain), embedded via
`include_bytes!`, with two equirectangular downscales of the 21600x10800
topo/bathy source:

- `app/assets/earth_16k.jpg` (16384x8192) - used on native. The app requests the
  GPU's full `max_texture_dimension_2d` (capped at 16384) and builds a mip chain
  with anisotropic filtering; GPUs that cap lower (and WebGPU's usual 8192) get
  the texture downscaled to fit at load time.
- `app/assets/earth.jpg` (8192x4096) - used on the web build, to keep the WASM
  payload reasonable.

```sh
curl -L -o /tmp/earth_src.jpg https://eoimages.gsfc.nasa.gov/images/imagerecords/73000/73909/world.topo.bathy.200412.3x21600x10800.jpg
magick /tmp/earth_src.jpg -resize 16384x8192! -quality 82 app/assets/earth_16k.jpg
sips -s format jpeg -s formatOptions 52 -z 4096 8192 /tmp/earth_src.jpg --out app/assets/earth.jpg
```
