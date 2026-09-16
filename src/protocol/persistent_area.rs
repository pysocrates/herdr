//! Workspace-scoped persistent-area presentation settings and pure geometry.
//!
//! These settings are independent of pane ownership and split trees. Store them
//! per workspace in optional presentation data, not in a frozen core codec.
use ratatui::layout::Rect;
use serde::{Deserialize, Deserializer, Serialize};

pub const DEFAULT_PERCENT: u16 = 35;
pub const MIN_PERCENT: u16 = 15;
pub const MAX_PERCENT: u16 = 75;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DockPlacement {
    #[default]
    Right,
    Bottom,
}

/// The persistent area's share of the usable span, excluding the divider.
///
/// Missing settings/fields use right placement and 35 percent. Construction,
/// updates, and deserialization all clamp the percentage to 15 through 75.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PersistentAreaSettings {
    pub placement: DockPlacement,
    #[serde(deserialize_with = "deserialize_percent")]
    percent: u16,
}

impl Default for PersistentAreaSettings {
    fn default() -> Self {
        Self::new(DockPlacement::Right, DEFAULT_PERCENT)
    }
}

impl PersistentAreaSettings {
    pub fn new(placement: DockPlacement, percent: u16) -> Self {
        Self {
            placement,
            percent: clamp_percent(percent),
        }
    }

    pub fn percent(self) -> u16 {
        self.percent
    }

    pub fn set_percent(&mut self, percent: u16) {
        self.percent = clamp_percent(percent);
    }

    pub fn layout(self, area: Rect) -> PersistentAreaLayout {
        split_persistent_area(area, self.placement, self.percent)
    }
}

pub fn clamp_percent(percent: u16) -> u16 {
    percent.clamp(MIN_PERCENT, MAX_PERCENT)
}

fn deserialize_percent<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u16, D::Error> {
    u16::deserialize(deserializer).map(clamp_percent)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PersistentAreaLayout {
    pub tabbed: Rect,
    pub persistent: Rect,
    pub divider: Rect,
}

/// Partition `area` into tabbed content, a one-cell divider, and persistent
/// content, in that order along the selected axis. No allocation or mutation.
///
/// The percentage applies after reserving the divider; integer rounding is
/// downward for persistent content. Both content regions get at least one cell
/// on the split axis. When fewer than three cells are available (or the other
/// axis is empty), tabbed content keeps the whole area and the other regions
/// are empty. Callers should skip this helper when the persistent area is off.
/// Rectangles whose coordinates plus dimensions overflow are clipped first.
pub fn split_persistent_area(
    area: Rect,
    placement: DockPlacement,
    percent: u16,
) -> PersistentAreaLayout {
    let area = Rect {
        x: area.x,
        y: area.y,
        width: area.width.min(u16::MAX - area.x),
        height: area.height.min(u16::MAX - area.y),
    };
    let span = match placement {
        DockPlacement::Right => area.width,
        DockPlacement::Bottom => area.height,
    };
    if span < 3 || area.width == 0 || area.height == 0 {
        let empty = Rect::new(area.right(), area.bottom(), 0, 0);
        return PersistentAreaLayout {
            tabbed: area,
            persistent: empty,
            divider: empty,
        };
    }

    let usable = span - 1;
    // Widen before multiplication: terminal geometry may span all of u16.
    let persistent = (u32::from(usable) * u32::from(clamp_percent(percent)) / 100) as u16;
    let persistent = persistent.clamp(1, usable - 1);
    let tabbed = usable - persistent;
    match placement {
        DockPlacement::Right => PersistentAreaLayout {
            tabbed: Rect::new(area.x, area.y, tabbed, area.height),
            divider: Rect::new(area.x + tabbed, area.y, 1, area.height),
            persistent: Rect::new(area.x + tabbed + 1, area.y, persistent, area.height),
        },
        DockPlacement::Bottom => PersistentAreaLayout {
            tabbed: Rect::new(area.x, area.y, area.width, tabbed),
            divider: Rect::new(area.x, area.y + tabbed, area.width, 1),
            persistent: Rect::new(area.x, area.y + tabbed + 1, area.width, persistent),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_serialization_are_neutral() {
        let settings = PersistentAreaSettings::default();
        assert_eq!(settings.placement, DockPlacement::Right);
        assert_eq!(settings.percent(), 35);
        assert_eq!(
            serde_json::from_str::<PersistentAreaSettings>("{}").unwrap(),
            settings
        );
        assert_eq!(
            serde_json::to_value(settings).unwrap(),
            serde_json::json!({"placement": "right", "percent": 35})
        );
        let bottom: PersistentAreaSettings =
            serde_json::from_str(r#"{"placement":"bottom"}"#).unwrap();
        assert_eq!(
            bottom,
            PersistentAreaSettings::new(DockPlacement::Bottom, 35)
        );
        assert_eq!(
            serde_json::from_value::<PersistentAreaSettings>(serde_json::to_value(bottom).unwrap())
                .unwrap(),
            bottom
        );
    }

    #[test]
    fn percent_is_clamped_at_every_entry_point() {
        for (input, expected) in [
            (0, 15),
            (14, 15),
            (35, 35),
            (75, 75),
            (76, 75),
            (u16::MAX, 75),
        ] {
            let mut settings = PersistentAreaSettings::new(DockPlacement::Right, input);
            assert_eq!(settings.percent(), expected);
            settings.set_percent(input);
            assert_eq!(settings.percent(), expected);
            let decoded: PersistentAreaSettings =
                serde_json::from_value(serde_json::json!({"percent": input})).unwrap();
            assert_eq!(decoded.percent(), expected);
            assert_eq!(
                split_persistent_area(Rect::new(0, 0, 101, 10), DockPlacement::Right, input),
                settings.layout(Rect::new(0, 0, 101, 10))
            );
        }
    }

    #[test]
    fn right_and_bottom_ratios_exclude_divider_and_preserve_offsets() {
        for percent in [15, 35, 75] {
            let tabbed = 100 - percent;
            let right =
                split_persistent_area(Rect::new(7, 11, 101, 20), DockPlacement::Right, percent);
            assert_eq!(right.tabbed, Rect::new(7, 11, tabbed, 20));
            assert_eq!(right.divider, Rect::new(7 + tabbed, 11, 1, 20));
            assert_eq!(right.persistent, Rect::new(8 + tabbed, 11, percent, 20));
            let bottom =
                split_persistent_area(Rect::new(7, 11, 20, 101), DockPlacement::Bottom, percent);
            assert_eq!(bottom.tabbed, Rect::new(7, 11, 20, tabbed));
            assert_eq!(bottom.divider, Rect::new(7, 11 + tabbed, 20, 1));
            assert_eq!(bottom.persistent, Rect::new(7, 12 + tabbed, 20, percent));
        }
    }

    fn assert_partition(area: Rect, result: PersistentAreaLayout) {
        let rects = [result.tabbed, result.divider, result.persistent];
        let mut cells = 0_u64;
        for rect in rects {
            assert!(rect.x >= area.x && rect.y >= area.y);
            assert!(rect.right() <= area.right() && rect.bottom() <= area.bottom());
            cells += u64::from(rect.width) * u64::from(rect.height);
        }
        assert_eq!(cells, u64::from(area.width) * u64::from(area.height));
        for (index, a) in rects.iter().enumerate() {
            for b in &rects[index + 1..] {
                if a.width == 0 || a.height == 0 || b.width == 0 || b.height == 0 {
                    continue;
                }
                assert!(
                    a.right() <= b.x || b.right() <= a.x || a.bottom() <= b.y || b.bottom() <= a.y
                );
            }
        }
    }

    #[test]
    fn tiny_sizes_are_bounded_disjoint_and_cover_the_input() {
        for width in 0..9 {
            for height in 0..9 {
                for placement in [DockPlacement::Right, DockPlacement::Bottom] {
                    for percent in [0, 15, 35, 75, u16::MAX] {
                        let area = Rect::new(23, 47, width, height);
                        let result = split_persistent_area(area, placement, percent);
                        assert_partition(area, result);
                        let span = if placement == DockPlacement::Right {
                            width
                        } else {
                            height
                        };
                        if span < 3 || width == 0 || height == 0 {
                            assert_eq!(result.tabbed, area);
                            assert_eq!(result.persistent.width, 0);
                            assert_eq!(result.divider.width, 0);
                        } else {
                            assert!(result.tabbed.width > 0 && result.tabbed.height > 0);
                            assert!(result.persistent.width > 0 && result.persistent.height > 0);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn large_sizes_and_overflowing_offsets_do_not_wrap() {
        for placement in [DockPlacement::Right, DockPlacement::Bottom] {
            let area = Rect {
                x: 0,
                y: 0,
                width: u16::MAX,
                height: u16::MAX,
            };
            assert_partition(area, split_persistent_area(area, placement, 75));
            let overflowing = Rect {
                x: u16::MAX - 8,
                y: u16::MAX - 8,
                width: 100,
                height: 100,
            };
            let clipped = Rect::new(overflowing.x, overflowing.y, 8, 8);
            let result = split_persistent_area(overflowing, placement, 35);
            assert_partition(clipped, result);
            assert_eq!(result, split_persistent_area(clipped, placement, 35));
        }
    }
}
