//! Conversion from Linkage Blaze's pixel-space items to Device Envoy draw items.
//!
//! Shared example code: useful beyond the examples, but kept private here until
//! it proves itself.

use crate::render::Item2d;
use device_envoy_core::cyd::display::DrawItem;

/// Convert a Linkage Blaze pixel-space item into the equivalent Device Envoy
/// draw item, for Device Envoy's streaming `draw_items` alongside its own items
/// such as bitmaps.
pub(super) fn device_draw_item(item_2d: Item2d) -> DrawItem {
    match item_2d {
        Item2d::Stroke {
            start,
            end,
            color,
            pixel_width,
        } => DrawItem::Stroke {
            start,
            end,
            color,
            pixel_width,
        },
        Item2d::Ellipse {
            center,
            axis_a,
            axis_b,
            color,
        } => DrawItem::Ellipse {
            center,
            axis_a,
            axis_b,
            color,
        },
        Item2d::Circle {
            center,
            pixel_radius,
            color,
        } => DrawItem::Circle {
            center,
            pixel_radius,
            color,
        },
    }
}
