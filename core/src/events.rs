use crate::body::Body;
use crate::simulation::Simulation;
use crate::time::Epoch;
use hifitime::Duration;

const SUNRISE_ALT: f64 = -0.833;
const CIVIL_ALT: f64 = -6.0;
const NAUTICAL_ALT: f64 = -12.0;
const ASTRONOMICAL_ALT: f64 = -18.0;

/// Sun rise/set and twilight times for the observer over the current UTC day.
/// A value is `None` when that crossing does not occur (polar day/night).
#[derive(Debug, Clone, Copy, Default)]
pub struct DayEvents {
    pub sunrise: Option<Epoch>,
    pub sunset: Option<Epoch>,
    pub civil_dawn: Option<Epoch>,
    pub civil_dusk: Option<Epoch>,
    pub nautical_dawn: Option<Epoch>,
    pub nautical_dusk: Option<Epoch>,
    pub astronomical_dawn: Option<Epoch>,
    pub astronomical_dusk: Option<Epoch>,
}

pub fn sun_day(sim: &Simulation) -> DayEvents {
    sun_day_at(sim, day_start(sim.clock.epoch))
}

fn sun_day_at(sim: &Simulation, start: Epoch) -> DayEvents {
    let sun = |alt, rising| cross(sim, Body::Sun, start, alt, rising);
    DayEvents {
        sunrise: sun(SUNRISE_ALT, true),
        sunset: sun(SUNRISE_ALT, false),
        civil_dawn: sun(CIVIL_ALT, true),
        civil_dusk: sun(CIVIL_ALT, false),
        nautical_dawn: sun(NAUTICAL_ALT, true),
        nautical_dusk: sun(NAUTICAL_ALT, false),
        astronomical_dawn: sun(ASTRONOMICAL_ALT, true),
        astronomical_dusk: sun(ASTRONOMICAL_ALT, false),
    }
}

/// Rise, set and transit (highest point) of any body for the observer over the
/// current UTC day. `rise`/`set` are `None` when no crossing occurs that day
/// (circumpolar or never up).
#[derive(Debug, Clone, Copy, Default)]
pub struct RiseSet {
    pub rise: Option<Epoch>,
    pub set: Option<Epoch>,
    pub transit: Option<Epoch>,
    /// Altitude (degrees) at transit - the body's best elevation today.
    pub transit_altitude: f64,
}

pub fn body_rise_set(sim: &Simulation, body: Body) -> RiseSet {
    body_rise_set_at(sim, body, day_start(sim.clock.epoch))
}

fn body_rise_set_at(sim: &Simulation, body: Body, start: Epoch) -> RiseSet {
    let horizon = if body == Body::Sun {
        SUNRISE_ALT
    } else {
        -0.5667
    };
    let (transit, transit_altitude) = transit(sim, body, start);
    RiseSet {
        rise: cross(sim, body, start, horizon, true),
        set: cross(sim, body, start, horizon, false),
        transit,
        transit_altitude,
    }
}

/// Time and altitude of the body's daily culmination.
fn transit(sim: &Simulation, body: Body, start: Epoch) -> (Option<Epoch>, f64) {
    const STEP_SECS: f64 = 300.0;
    const STEPS: i64 = 288;
    let alt = |e: Epoch| sim.geometric_altitude_at(body, e);

    let mut best_t = start;
    let mut best = alt(start);
    for i in 1..=STEPS {
        let t = start + Duration::from_seconds(i as f64 * STEP_SECS);
        let a = alt(t);
        if a > best {
            best = a;
            best_t = t;
        }
    }

    let lo = best_t - Duration::from_seconds(STEP_SECS);
    let hi = best_t + Duration::from_seconds(STEP_SECS);
    for i in 0..=60 {
        let t = lo + (hi - lo) * (i as f64 / 60.0);
        let a = alt(t);
        if a > best {
            best = a;
            best_t = t;
        }
    }
    (Some(best_t), best.to_degrees())
}

fn day_start(epoch: Epoch) -> Epoch {
    let (y, m, d, _, _, _, _) = epoch.to_gregorian_utc();
    Epoch::from_gregorian_utc(y, m, d, 0, 0, 0, 0)
}

fn cross(
    sim: &Simulation,
    body: Body,
    start: Epoch,
    target_deg: f64,
    rising: bool,
) -> Option<Epoch> {
    const STEP_SECS: f64 = 300.0;
    const STEPS: i64 = 288;

    let value = |e: Epoch| sim.geometric_altitude_at(body, e).to_degrees() - target_deg;

    let mut t0 = start;
    let mut v0 = value(start);
    for i in 1..=STEPS {
        let t1 = start + Duration::from_seconds(i as f64 * STEP_SECS);
        let v1 = value(t1);
        let hit = if rising {
            v0 < 0.0 && v1 >= 0.0
        } else {
            v0 >= 0.0 && v1 < 0.0
        };
        if hit {
            return Some(bisect(&value, t0, t1));
        }
        t0 = t1;
        v0 = v1;
    }
    None
}

fn bisect(value: &impl Fn(Epoch) -> f64, mut lo: Epoch, mut hi: Epoch) -> Epoch {
    for _ in 0..32 {
        let mid = lo + (hi - lo) * 0.5;
        if value(lo).signum() == value(mid).signum() {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo + (hi - lo) * 0.5
}

/// Broad families of dated events, used for filtering the events list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventCategory {
    /// Sunrise, sunset and twilight.
    Sun,
    /// Solar and lunar eclipses.
    Eclipse,
    /// Conjunctions, oppositions, greatest elongations and apsides.
    Approach,
    /// Rise and set of the Moon and other bodies.
    RiseSet,
}

impl EventCategory {
    pub const ALL: [EventCategory; 4] = [
        EventCategory::Sun,
        EventCategory::Eclipse,
        EventCategory::Approach,
        EventCategory::RiseSet,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            EventCategory::Sun => "Sun",
            EventCategory::Eclipse => "Eclipses",
            EventCategory::Approach => "Approaches",
            EventCategory::RiseSet => "Rise / set",
        }
    }
}

/// A single dated astronomical event in the scan window.
#[derive(Debug, Clone)]
pub struct AstroEvent {
    pub time: Epoch,
    pub category: EventCategory,
    pub title: String,
    pub detail: String,
    /// Primary body involved, for optional viewing-area filtering.
    pub body: Option<Body>,
}

/// The planets bright/quick enough to bother pairing for conjunctions.
const APPROACH_PLANETS: [Body; 7] = [
    Body::Mercury,
    Body::Venus,
    Body::Mars,
    Body::Jupiter,
    Body::Saturn,
    Body::Uranus,
    Body::Neptune,
];

/// Scan a window around the current epoch for dated events of every category.
/// `past_days`/`future_days` bound the window; daily rise/set events are only
/// emitted for reasonably narrow windows to avoid flooding the list.
pub fn scan_events(sim: &Simulation, past_days: f64, future_days: f64) -> Vec<AstroEvent> {
    let start = sim.clock.epoch - Duration::from_days(past_days);
    let end = sim.clock.epoch + Duration::from_days(future_days);
    let mut out = Vec::new();

    scan_eclipses(sim, start, end, &mut out);
    scan_conjunctions(sim, start, end, &mut out);
    scan_oppositions_elongations(sim, start, end, &mut out);
    scan_apsides(sim, start, end, &mut out);

    if past_days + future_days <= 45.0 {
        scan_daily_rise_set(sim, start, end, &mut out);
    }

    out.sort_by_key(|e| e.time);
    out
}

fn scan_eclipses(sim: &Simulation, start: Epoch, end: Epoch, out: &mut Vec<AstroEvent>) {
    let sep = |e: Epoch| sim.separation_at(Body::Sun, Body::Moon, e).to_degrees();
    for (t, v) in extrema(start, end, 0.25, false, &sep) {
        if v < 0.58 {
            out.push(AstroEvent {
                time: t,
                category: EventCategory::Eclipse,
                title: "Solar eclipse".to_string(),
                detail: format!("Sun and Moon {v:.2} deg apart (somewhere on Earth)"),
                body: Some(Body::Sun),
            });
        }
    }
    for (t, v) in extrema(start, end, 0.25, true, &sep) {
        let anti = 180.0 - v;
        if anti < 1.2 {
            out.push(AstroEvent {
                time: t,
                category: EventCategory::Eclipse,
                title: "Lunar eclipse".to_string(),
                detail: format!("Moon {anti:.2} deg from Earth's shadow centre"),
                body: Some(Body::Moon),
            });
        }
    }
}

fn scan_conjunctions(sim: &Simulation, start: Epoch, end: Epoch, out: &mut Vec<AstroEvent>) {
    for (i, &a) in APPROACH_PLANETS.iter().enumerate() {
        for &b in &APPROACH_PLANETS[i + 1..] {
            let f = |e: Epoch| sim.separation_at(a, b, e).to_degrees();
            for (t, v) in extrema(start, end, 1.0, false, &f) {
                if v < 3.5 {
                    out.push(AstroEvent {
                        time: t,
                        category: EventCategory::Approach,
                        title: format!("{} - {} conjunction", a.name(), b.name()),
                        detail: format!("{v:.2} deg apart in the sky"),
                        body: Some(a),
                    });
                }
            }
        }
    }
}

fn scan_oppositions_elongations(
    sim: &Simulation,
    start: Epoch,
    end: Epoch,
    out: &mut Vec<AstroEvent>,
) {
    for body in [
        Body::Mars,
        Body::Jupiter,
        Body::Saturn,
        Body::Uranus,
        Body::Neptune,
    ] {
        let f = |e: Epoch| sim.elongation_at(body, e).to_degrees();
        for (t, v) in extrema(start, end, 1.0, true, &f) {
            if v > 150.0 {
                out.push(AstroEvent {
                    time: t,
                    category: EventCategory::Approach,
                    title: format!("{} at opposition", body.name()),
                    detail: format!("opposite the Sun, {v:.0} deg elongation - best viewing"),
                    body: Some(body),
                });
            }
        }
    }
    for body in [Body::Mercury, Body::Venus] {
        let f = |e: Epoch| sim.elongation_at(body, e).to_degrees();
        for (t, v) in extrema(start, end, 1.0, true, &f) {
            if v > 10.0 {
                let side = elongation_side(sim, body, t);
                out.push(AstroEvent {
                    time: t,
                    category: EventCategory::Approach,
                    title: format!("{} greatest elongation {side}", body.name()),
                    detail: format!("{v:.0} deg from the Sun"),
                    body: Some(body),
                });
            }
        }
    }
}

/// Whether a body sits east (evening sky) or west (morning sky) of the Sun.
fn elongation_side(sim: &Simulation, body: Body, epoch: Epoch) -> &'static str {
    let p = sim.geocentric_at(body, epoch);
    let s = sim.geocentric_at(Body::Sun, epoch);
    let diff = (p.y.atan2(p.x) - s.y.atan2(s.x)).rem_euclid(std::f64::consts::TAU);
    if diff < std::f64::consts::PI {
        "East (evening)"
    } else {
        "West (morning)"
    }
}

fn scan_apsides(sim: &Simulation, start: Epoch, end: Epoch, out: &mut Vec<AstroEvent>) {
    for body in [
        Body::Mercury,
        Body::Venus,
        Body::Earth,
        Body::Mars,
        Body::Jupiter,
    ] {
        let f = |e: Epoch| sim.heliocentric_at(body, e).length();
        for (t, v) in extrema(start, end, 2.0, false, &f) {
            out.push(apsis(t, body, "perihelion", v));
        }
        for (t, v) in extrema(start, end, 2.0, true, &f) {
            out.push(apsis(t, body, "aphelion", v));
        }
    }

    let moon = |e: Epoch| sim.geocentric_at(Body::Moon, e).length() * crate::math::AU_KM;
    for (t, v) in extrema(start, end, 0.25, false, &moon) {
        out.push(AstroEvent {
            time: t,
            category: EventCategory::Approach,
            title: "Moon at perigee".to_string(),
            detail: format!("{:.0} km from Earth (closest)", v),
            body: Some(Body::Moon),
        });
    }
    for (t, v) in extrema(start, end, 0.25, true, &moon) {
        out.push(AstroEvent {
            time: t,
            category: EventCategory::Approach,
            title: "Moon at apogee".to_string(),
            detail: format!("{:.0} km from Earth (farthest)", v),
            body: Some(Body::Moon),
        });
    }
}

fn apsis(time: Epoch, body: Body, which: &str, au: f64) -> AstroEvent {
    AstroEvent {
        time,
        category: EventCategory::Approach,
        title: format!("{} {which}", body.name()),
        detail: format!("{au:.4} AU from the Sun"),
        body: Some(body),
    }
}

fn scan_daily_rise_set(sim: &Simulation, start: Epoch, end: Epoch, out: &mut Vec<AstroEvent>) {
    // Only the Sun and Moon rise/set crossings (not the full twilight set or
    // transit), and only over a short sub-window near "now", so the scan stays
    // cheap even when the overall window spans a month. Sparse events (eclipses,
    // conjunctions, ...) still cover the full window.
    let now = sim.clock.epoch;
    let lo = start.max(now - Duration::from_days(3.0));
    let hi = end.min(now + Duration::from_days(10.0));
    let mut day = day_start(lo);
    while day <= hi {
        let sunrise = cross(sim, Body::Sun, day, SUNRISE_ALT, true);
        let sunset = cross(sim, Body::Sun, day, SUNRISE_ALT, false);
        push_riseset(
            out,
            sunrise,
            EventCategory::Sun,
            "Sunrise",
            Body::Sun,
            start,
            end,
        );
        push_riseset(
            out,
            sunset,
            EventCategory::Sun,
            "Sunset",
            Body::Sun,
            start,
            end,
        );

        let moonrise = cross(sim, Body::Moon, day, -0.5667, true);
        let moonset = cross(sim, Body::Moon, day, -0.5667, false);
        push_riseset(
            out,
            moonrise,
            EventCategory::RiseSet,
            "Moonrise",
            Body::Moon,
            start,
            end,
        );
        push_riseset(
            out,
            moonset,
            EventCategory::RiseSet,
            "Moonset",
            Body::Moon,
            start,
            end,
        );
        day += Duration::from_days(1.0);
    }
}

#[allow(clippy::too_many_arguments)]
fn push_riseset(
    out: &mut Vec<AstroEvent>,
    time: Option<Epoch>,
    category: EventCategory,
    title: &str,
    body: Body,
    start: Epoch,
    end: Epoch,
) {
    if let Some(t) = time
        && t >= start
        && t <= end
    {
        out.push(AstroEvent {
            time: t,
            category,
            title: title.to_string(),
            detail: String::new(),
            body: Some(body),
        });
    }
}

/// Rise, set and transit of a fixed star (analytic, from its hour angle) over
/// the UTC day beginning at `start`. `None` rise/set means circumpolar or never
/// up; `transit_altitude` is the culmination altitude either way.
pub fn star_rise_set(
    observer: &crate::observer::Observer,
    ra: f64,
    dec: f64,
    start: Epoch,
) -> RiseSet {
    let lat = observer.latitude_rad();
    let transit_altitude = (90.0 - (lat.to_degrees() - dec.to_degrees()).abs()).min(90.0);
    let cos_h = -(lat.tan()) * dec.tan();

    let transit = lst_time(observer, ra, start);
    if cos_h <= -1.0 || cos_h >= 1.0 {
        return RiseSet {
            rise: None,
            set: None,
            transit: Some(transit),
            transit_altitude,
        };
    }
    let h = cos_h.acos();
    RiseSet {
        rise: Some(lst_time(observer, ra - h, start)),
        set: Some(lst_time(observer, ra + h, start)),
        transit: Some(transit),
        transit_altitude,
    }
}

/// The first instant within the UTC day starting at `start` when the local
/// sidereal time equals `target` (radians).
fn lst_time(observer: &crate::observer::Observer, target: f64, start: Epoch) -> Epoch {
    const SIDEREAL_RATE: f64 = std::f64::consts::TAU * 1.002_737_909_35 / 86_400.0;
    let lst0 = (crate::earth::earth_rotation_angle(start) + observer.longitude_rad())
        .rem_euclid(std::f64::consts::TAU);
    let delta = (target.rem_euclid(std::f64::consts::TAU) - lst0).rem_euclid(std::f64::consts::TAU);
    start + Duration::from_seconds(delta / SIDEREAL_RATE)
}

/// Local extrema of `f` over `[start, end]`: sample at `step_days`, flag each
/// sampled point lower (or higher) than both neighbours, then refine.
fn extrema(
    start: Epoch,
    end: Epoch,
    step_days: f64,
    maximize: bool,
    f: &impl Fn(Epoch) -> f64,
) -> Vec<(Epoch, f64)> {
    let mut out = Vec::new();
    let total = (end - start).to_unit(hifitime::Unit::Day);
    let n = (total / step_days).ceil() as i64;
    if n < 2 {
        return out;
    }
    let at = |i: i64| start + Duration::from_days(i as f64 * step_days);
    let mut f0 = f(at(0));
    let mut f1 = f(at(1));
    for i in 1..n {
        let f2 = f(at(i + 1));
        let is_ext = if maximize {
            f1 >= f0 && f1 >= f2
        } else {
            f1 <= f0 && f1 <= f2
        };
        if is_ext {
            out.push(refine(f, at(i - 1), at(i + 1), maximize));
        }
        f0 = f1;
        f1 = f2;
    }
    out
}

/// Golden-section search for an extremum of `f` in `[lo, hi]`.
fn refine(f: &impl Fn(Epoch) -> f64, lo: Epoch, hi: Epoch, maximize: bool) -> (Epoch, f64) {
    const G: f64 = 0.618_033_988_75;
    let span = |a: Epoch, b: Epoch| (b - a).to_seconds();
    let (mut a, mut b) = (lo, hi);
    let mut c = a + Duration::from_seconds(span(a, b) * (1.0 - G));
    let mut d = a + Duration::from_seconds(span(a, b) * G);
    let (mut fc, mut fd) = (f(c), f(d));
    let better = |x: f64, y: f64| if maximize { x > y } else { x < y };
    for _ in 0..48 {
        if better(fc, fd) {
            b = d;
            d = c;
            fd = fc;
            c = a + Duration::from_seconds(span(a, b) * (1.0 - G));
            fc = f(c);
        } else {
            a = c;
            c = d;
            fc = fd;
            d = a + Duration::from_seconds(span(a, b) * G);
            fd = f(d);
        }
    }
    let t = a + Duration::from_seconds(span(a, b) * 0.5);
    (t, f(t))
}
