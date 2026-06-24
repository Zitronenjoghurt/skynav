use skynav::Epoch;

/// The current wall-clock instant as a UTC epoch (works native and on the web).
#[cfg(not(target_arch = "wasm32"))]
pub fn now_epoch() -> Epoch {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    Epoch::from_unix_seconds(secs)
}

#[cfg(target_arch = "wasm32")]
pub fn now_epoch() -> Epoch {
    Epoch::from_unix_seconds(js_sys::Date::now() / 1000.0)
}
