//! What the user has currently picked, shared across the Sky, System and Bodies
//! views and rendered in detail by the Info panel.

use serde::{Deserialize, Serialize};
use skynav::Body;

/// A selected celestial object. Stars are referenced by their index into the
/// loaded catalogue slice.
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Selection {
    Body(Body),
    Star(usize),
}
