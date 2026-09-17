use super::*;

pub(super) const CONTROL_WIDTH: u16 = 27;

#[derive(Default)]
pub(super) struct PersistentHits {
    pub toggle: Rect,
    pub placement: Rect,
    pub divider: Rect,
}

pub(super) fn render_controls(
    buffer: &mut Buffer,
    bar: Rect,
    enabled: bool,
    supported: bool,
    palette: &Palette,
) -> PersistentHits {
    if bar.width < 12 || bar.height == 0 || !supported {
        return PersistentHits::default();
    }
    let width = CONTROL_WIDTH.min(bar.width);
    let toggle_width = width.saturating_sub(if enabled { 8 } else { 0 });
    let toggle = Rect::new(bar.right().saturating_sub(width), bar.y, toggle_width, 1);
    let label = if enabled {
        " Persistent: on "
    } else {
        " Persistent area + "
    };
    let style = Style::default()
        .fg(if enabled {
            palette.accent
        } else {
            palette.overlay1
        })
        .bg(palette.surface0);
    render::put_text(buffer, toggle.x, toggle.y, toggle.width, label, style);
    let placement = if enabled {
        Rect::new(toggle.right(), bar.y, width - toggle.width, 1)
    } else {
        Rect::default()
    };
    render::put_text(
        buffer,
        placement.x,
        placement.y,
        placement.width,
        " rotate ",
        style,
    );
    PersistentHits {
        toggle,
        placement,
        divider: Rect::default(),
    }
}

impl ClientShellState {
    pub(super) fn persistent_tab_at(&self, point: (u16, u16)) -> Option<String> {
        let (cols, rows) = self.last_composed_size?;
        let area = self.persistent_area().filter(|area| area.enabled)?;
        contains(
            area.settings
                .layout(self.layout(cols, rows).pane_surface)
                .persistent,
            point,
        )
        .then(|| area.tab_id.clone())
    }

    pub(super) fn resize_persistent_area(
        &mut self,
        point: (u16, u16),
        outcome: &mut ClientShellInput,
    ) {
        let Some((cols, rows)) = self.last_composed_size else {
            return;
        };
        let surface = self.layout(cols, rows).pane_surface;
        let Some(workspace) = self
            .snapshot
            .as_deref()
            .and_then(|s| s.focused_workspace_id.clone())
        else {
            return;
        };
        if let Some(area) = self
            .sticky
            .get_mut(&self.active_endpoint_id)
            .and_then(|dock| dock.areas.get_mut(&workspace))
        {
            let (span, remaining) = match area.settings.placement {
                crate::protocol::persistent_area::DockPlacement::Right => {
                    (surface.width, surface.right().saturating_sub(point.0 + 1))
                }
                crate::protocol::persistent_area::DockPlacement::Bottom => {
                    (surface.height, surface.bottom().saturating_sub(point.1 + 1))
                }
            };
            if span > 1 {
                area.settings.set_percent((u32::from(remaining) * 100 / u32::from(span - 1)).min(100) as u16);
            }
        }
        self.send_sticky(outcome);
    }

    pub(super) fn handle_persistent_mouse(
        &mut self,
        mouse: crossterm::event::MouseEvent,
        outcome: &mut ClientShellInput,
    ) -> bool {
        use crossterm::event::{MouseButton, MouseEventKind};
        if self.overlay.is_some() {
            self.persistent_drag = false;
            return false;
        }
        let point = (mouse.column, mouse.row);
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if contains(self.persistent_hits.toggle, point) {
                    self.toggle_persistent_area(outcome);
                    return true;
                }
                if contains(self.persistent_hits.placement, point) {
                    self.change_persistent_placement(outcome);
                    return true;
                }
                if contains(self.persistent_hits.divider, point) {
                    self.persistent_drag = true;
                    return true;
                }
            }
            MouseEventKind::Drag(MouseButton::Left) if self.persistent_drag => {
                self.resize_persistent_area(point, outcome);
                return true;
            }
            MouseEventKind::Up(MouseButton::Left) if self.persistent_drag => {
                self.persistent_drag = false;
                self.resize_persistent_area(point, outcome);
                return true;
            }
            _ => {}
        }
        false
    }
}
