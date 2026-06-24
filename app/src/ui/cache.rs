//! Per-frame memoisation for the expensive day-event scans. `sun_day` and
//! `body_rise_set` each run hundreds of ephemeris evaluations, and the panels
//! that show them repaint every frame while an object is selected (camera/pulse
//! animation). The results depend only on the UTC day and the observer, so we
//! cache them in egui's temp store and recompute only when that key changes.

use egui::Id;
use skynav::{Body, DayEvents, RiseSet, Simulation, events};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

fn day_key(sim: &Simulation) -> u64 {
    let (y, m, d, ..) = sim.clock.epoch.to_gregorian_utc();
    let mut hasher = DefaultHasher::new();
    (y, m, d).hash(&mut hasher);
    sim.observer.latitude_deg.to_bits().hash(&mut hasher);
    sim.observer.longitude_deg.to_bits().hash(&mut hasher);
    sim.observer.height_m.to_bits().hash(&mut hasher);
    hasher.finish()
}

pub fn sun_day(ui: &egui::Ui, sim: &Simulation) -> DayEvents {
    let key = day_key(sim);
    let id = Id::new("cache_sun_day");
    if let Some((k, value)) = ui.data(|d| d.get_temp::<(u64, DayEvents)>(id))
        && k == key
    {
        return value;
    }
    let value = events::sun_day(sim);
    ui.data_mut(|d| d.insert_temp(id, (key, value)));
    value
}

pub fn body_rise_set(ui: &egui::Ui, sim: &Simulation, body: Body) -> RiseSet {
    let key = day_key(sim);
    let id = Id::new(("cache_rise_set", body));
    if let Some((k, value)) = ui.data(|d| d.get_temp::<(u64, RiseSet)>(id))
        && k == key
    {
        return value;
    }
    let value = events::body_rise_set(sim, body);
    ui.data_mut(|d| d.insert_temp(id, (key, value)));
    value
}
