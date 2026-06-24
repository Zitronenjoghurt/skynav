pub mod body;
pub mod catalog;
pub mod constellations;
pub mod earth;
pub mod ephemeris;
pub mod events;
pub mod frames;
pub mod math;
pub mod observer;
pub mod simulation;
pub mod sky;
pub mod time;

pub use body::Body;
pub use catalog::Star;
pub use constellations::Constellation;
pub use ephemeris::{AnalyticEphemeris, Ephemeris};
pub use events::{DayEvents, RiseSet};
pub use observer::Observer;
pub use simulation::Simulation;
pub use sky::Horizontal;
pub use time::{Epoch, SimClock};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::AU_KM;

    fn j2000() -> Epoch {
        Epoch::from_gregorian_utc_hms(2000, 1, 1, 12, 0, 0)
    }

    #[test]
    fn earth_is_about_one_au_from_sun() {
        let sim = Simulation::new(j2000());
        let d = sim.heliocentric(Body::Earth).length();
        assert!((0.97..1.03).contains(&d), "earth distance was {d} AU");
    }

    #[test]
    fn moon_is_about_a_quarter_million_km_away() {
        let sim = Simulation::new(j2000());
        let d = sim.geocentric(Body::Moon).length() * AU_KM;
        assert!(
            (350_000.0..410_000.0).contains(&d),
            "moon distance was {d} km"
        );
    }

    #[test]
    fn sun_is_at_the_heliocentric_origin() {
        let sim = Simulation::new(j2000());
        assert_eq!(sim.heliocentric(Body::Sun), math::DVec3::ZERO);
    }

    #[test]
    fn observer_sits_on_earth_surface() {
        let sim = Simulation::new(j2000());
        let r_km = sim.observer_equatorial().length() * AU_KM;
        assert!(
            (6300.0..6400.0).contains(&r_km),
            "observer radius {r_km} km"
        );
    }

    #[test]
    fn sun_rises_before_it_sets() {
        let mut sim = Simulation::new(Epoch::from_gregorian_utc_hms(2026, 3, 20, 0, 0, 0));
        sim.observer = Observer::new(51.5, 0.0, 0.0);
        let ev = events::sun_day(&sim);
        let sunrise = ev.sunrise.expect("sunrise");
        let sunset = ev.sunset.expect("sunset");
        assert!(sunrise < sunset);
        let alt = sim.geometric_altitude_at(Body::Sun, sunrise).to_degrees();
        assert!((alt + 0.833).abs() < 0.1, "altitude at sunrise was {alt}");
    }

    #[test]
    fn polaris_altitude_tracks_latitude() {
        let mut sim = Simulation::new(j2000());
        sim.observer = Observer::new(50.0, 10.0, 0.0);
        let polaris = catalog::load_stars()
            .into_iter()
            .find(|s| s.name == "Polaris")
            .unwrap();
        let h = sim.observed_star(&polaris).unwrap();
        assert!(
            (h.altitude_deg() - 50.0).abs() < 2.0,
            "polaris altitude was {}",
            h.altitude_deg()
        );
    }
}
