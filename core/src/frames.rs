//! Reference frames used by the simulation.
//!
//! Positions from the analytic ephemeris are heliocentric, rectangular and
//! referred to the ecliptic and mean equinox of J2000.0. Helpers here move
//! vectors between that ecliptic frame and the equatorial J2000 frame.

pub use crate::math::{ecliptic_to_equatorial, equatorial_to_ecliptic};

/// The reference frame a position vector is expressed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frame {
    /// Ecliptic, mean equinox of J2000.0 (native VSOP87 frame).
    EclipticJ2000,
    /// Equatorial, mean equinox of J2000.0.
    EquatorialJ2000,
}
