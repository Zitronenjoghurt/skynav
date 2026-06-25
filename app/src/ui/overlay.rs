//! Shared helpers for drawing egui labels on top of the wgpu views.

use egui::{Color32, FontId, Painter, Pos2, Rect, pos2};
use glam::{Mat4, Vec3, Vec4};

/// Project a world-space direction to a screen position within `rect`, or
/// `None` if it is behind the camera or off-screen.
pub fn project(view_proj: Mat4, point: Vec3, rect: Rect) -> Option<Pos2> {
    let clip = view_proj * point.extend(1.0);
    if clip.w <= 0.0 {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    if ndc.x.abs() > 1.0 || ndc.y.abs() > 1.0 {
        return None;
    }
    Some(clip_to_screen(clip, rect))
}

/// Project a line segment, clipped against the near plane so a segment that
/// crosses behind the camera is trimmed rather than dropped. Off-screen
/// endpoints are kept (egui clips them to `rect`), so lines no longer vanish at
/// the view edges. Returns `None` only when the whole segment is behind.
pub fn project_segment(view_proj: Mat4, a: Vec3, b: Vec3, rect: Rect) -> Option<[Pos2; 2]> {
    const NEAR: f32 = 1.0e-4;
    let mut ca = view_proj * a.extend(1.0);
    let mut cb = view_proj * b.extend(1.0);
    if ca.w < NEAR && cb.w < NEAR {
        return None;
    }
    if ca.w < NEAR {
        let t = (NEAR - ca.w) / (cb.w - ca.w);
        ca = lerp4(ca, cb, t);
    } else if cb.w < NEAR {
        let t = (NEAR - cb.w) / (ca.w - cb.w);
        cb = lerp4(cb, ca, t);
    }
    Some([clip_to_screen(ca, rect), clip_to_screen(cb, rect)])
}

fn lerp4(a: Vec4, b: Vec4, t: f32) -> Vec4 {
    a + (b - a) * t
}

fn clip_to_screen(clip: Vec4, rect: Rect) -> Pos2 {
    let ndc = clip.truncate() / clip.w;
    pos2(
        rect.left() + (ndc.x * 0.5 + 0.5) * rect.width(),
        rect.top() + (1.0 - (ndc.y * 0.5 + 0.5)) * rect.height(),
    )
}

/// Draw a small text label with a translucent background at `pos`.
pub fn label_at(painter: &Painter, pos: Pos2, text: &str, color: Color32) {
    let galley = painter.layout_no_wrap(text.to_string(), FontId::proportional(12.0), color);
    let rect = Rect::from_min_size(pos, galley.size()).expand(3.0);
    painter.rect_filled(rect, 3.0, Color32::from_black_alpha(170));
    painter.galley(pos, galley, color);
}
