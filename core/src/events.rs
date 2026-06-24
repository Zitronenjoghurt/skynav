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
    let start = day_start(sim.clock.epoch);
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
    let start = day_start(sim.clock.epoch);
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
