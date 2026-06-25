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

/// Human-friendly time-until / time-since string relative to `now`, e.g.
/// "in 2d 4h", "in 35m", "12m ago", "now". Keeps the two largest non-zero units.
pub fn humanize_until(target: Epoch, now: Epoch) -> String {
    let secs = (target - now).to_seconds();
    let abs = secs.abs();
    if abs < 60.0 {
        return "now".to_string();
    }
    let mut remaining = abs as i64;
    let parts = [("y", 365 * 86_400), ("d", 86_400), ("h", 3_600), ("m", 60)];
    let mut chunks = Vec::new();
    for (unit, size) in parts {
        let n = remaining / size;
        if n > 0 {
            chunks.push(format!("{n}{unit}"));
            remaining %= size;
        }
        if chunks.len() == 2 {
            break;
        }
    }
    let body = chunks.join(" ");
    if secs >= 0.0 {
        format!("in {body}")
    } else {
        format!("{body} ago")
    }
}
