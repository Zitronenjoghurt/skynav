//! Shared helpers for drawing egui labels on top of the wgpu views.

use egui::{Color32, FontId, Painter, Pos2, Rect, pos2};
use glam::{Mat4, Vec3};

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
    Some(pos2(
        rect.left() + (ndc.x * 0.5 + 0.5) * rect.width(),
        rect.top() + (1.0 - (ndc.y * 0.5 + 0.5)) * rect.height(),
    ))
}

/// Draw a small text label with a translucent background at `pos`.
pub fn label_at(painter: &Painter, pos: Pos2, text: &str, color: Color32) {
    let galley = painter.layout_no_wrap(text.to_string(), FontId::proportional(12.0), color);
    let rect = Rect::from_min_size(pos, galley.size()).expand(3.0);
    painter.rect_filled(rect, 3.0, Color32::from_black_alpha(170));
    painter.galley(pos, galley, color);
}
