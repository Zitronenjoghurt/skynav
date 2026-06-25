# skynav

[![CI](https://github.com/Zitronenjoghurt/skynav/actions/workflows/ci.yml/badge.svg)](https://github.com/Zitronenjoghurt/skynav/actions/workflows/ci.yml)
![Lines of Code](http://tokei.lemon.industries/b1/github/Zitronenjoghurt/skynav?category=code&type=Rust&logo=https://simpleicons.org/icons/rust.svg)

skynav shows where Earth and the other planets actually are right now, and what
the sky looks like from any point on any of them. It computes real ephemerides
and Earth orientation, so the positions, the lit and dark sides, and the sky all
line up with reality instead of an approximation.

## What you can do

- Explore in one continuous view: stand on a planet's surface, rise into orbit,
  zoom out to the whole solar system, and travel to another body.
- Look at the sky from your location, with the full star catalogue,
  constellation figures and live planet positions.
- Pick any date and time, or fast-forward and rewind to watch things move.
- Set your observer location on any body and read off rise/set times, altitude
  and azimuth.
- Keep a checklist of the objects you have observed.

It runs as a native app and in the browser (WebGPU).

## Running

Native:

```sh
cargo run -p skynav-app   # or: make app
```

Web (needs Trunk and the wasm32 target):

```sh
make web
```

A small axum server hosts the built web app, on port 61330 (set `PORT` to
change it):

```sh
make server
```

`make check` runs formatting, clippy and the tests across every crate.

## Layout

- `core` - the simulation: ephemerides, body orientation, coordinate frames and
  events. No UI.
- `app` - the eframe/egui interface and the wgpu renderers.
- `server` - static file server for the web build.
- `parse` - bakes the raw star and constellation data into the blobs the app
  loads at startup.

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

The other bodies use 2048x1024 equirectangular maps from
[Solar System Scope](https://www.solarsystemscope.com/textures/) (CC BY 4.0),
embedded the same way (`app/assets/{mercury,venus,moon,mars,jupiter,saturn,uranus,neptune,sun}.jpg`):

```sh
base=https://www.solarsystemscope.com/textures/download
for b in mercury moon mars jupiter saturn uranus neptune sun; do
  curl -fsSL -o "app/assets/$b.jpg" "$base/2k_$b.jpg"
done
curl -fsSL -o app/assets/venus.jpg "$base/2k_venus_atmosphere.jpg"
```
