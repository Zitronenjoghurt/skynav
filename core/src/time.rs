use hifitime::Duration;
pub use hifitime::Epoch;

/// Simulation clock: the current instant plus a play/pause state and a
/// fast-forward rate (simulated seconds per real second).
#[derive(Debug, Clone, Copy)]
pub struct SimClock {
    pub epoch: Epoch,
    pub rate: f64,
    pub playing: bool,
}

impl SimClock {
    pub fn new(epoch: Epoch) -> Self {
        Self {
            epoch,
            rate: 1.0,
            playing: true,
        }
    }

    /// Advance the clock by `real_dt` real seconds, scaled by the current rate.
    pub fn advance(&mut self, real_dt: f64) {
        if self.playing {
            self.epoch += Duration::from_seconds(self.rate * real_dt);
        }
    }

    pub fn jde_tdb(&self) -> f64 {
        self.epoch.to_jde_tdb_days()
    }
}
