//! The set of celestial objects the user has marked as personally observed.
//! Persisted across sessions and shared with the Checklist, Info, Bodies and
//! Visible panels.

use crate::ui::Selection;
use serde::{Deserialize, Serialize};
use skynav::{Body, Star};
use std::collections::HashSet;

/// Stable string keys of every observed object. Keys are name-based so they
/// survive catalogue reordering (unlike a star's slice index).
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Observed {
    keys: HashSet<String>,
}

impl Observed {
    /// The key for a selectable object, or `None` for things that cannot be
    /// checklisted (an unnamed star).
    pub fn key_for(sel: Selection, stars: &[Star]) -> Option<String> {
        match sel {
            Selection::Body(body) => Some(body_key(body)),
            Selection::Star(i) => stars
                .get(i)
                .filter(|s| !s.name.is_empty())
                .map(|s| star_key(&s.name)),
        }
    }

    pub fn contains(&self, key: &str) -> bool {
        self.keys.contains(key)
    }

    pub fn is_observed(&self, sel: Selection, stars: &[Star]) -> bool {
        Self::key_for(sel, stars).is_some_and(|k| self.keys.contains(&k))
    }

    pub fn toggle(&mut self, key: &str) {
        if !self.keys.remove(key) {
            self.keys.insert(key.to_string());
        }
    }
}

pub fn body_key(body: Body) -> String {
    format!("body:{}", body.name())
}

pub fn star_key(name: &str) -> String {
    format!("star:{name}")
}

pub fn constellation_key(name: &str) -> String {
    format!("con:{name}")
}
