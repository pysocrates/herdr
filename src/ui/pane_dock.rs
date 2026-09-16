//! Layout for additional client-selected panes in the same workspace.
use super::tab_surface::*;
use crate::{
    app::AppState,
    layout::{PaneId, PaneInfo},
    terminal::TerminalRuntimeRegistry,
};
use ratatui::{layout::Rect, widgets::Borders};

pub(crate) fn dock_main_area(area: Rect, count: usize) -> Rect {
    if count == 0 {
        area
    } else {
        Rect::new(area.x, area.y, area.width / 2, area.height)
    }
}

pub(crate) fn compute_docked_surface(
    app: &AppState,
    runtimes: &TerminalRuntimeRegistry,
    target: Option<TabSurfaceTarget>,
    area: Rect,
    resize: bool,
    cell_size: crate::kitty_graphics::HostCellSize,
    panes: &[PaneId],
    focus: Option<PaneId>,
    persistent: Option<(TabSurfaceTarget, crate::protocol::persistent_area::PersistentAreaSettings)>,
) -> TabSurfaceLayout {
    if let Some((dock_target, settings)) = persistent {
        let geometry = settings.layout(area);
        let mut layout = compute_tab_surface_for(app, runtimes, target, geometry.tabbed, resize, cell_size);
        if !geometry.persistent.is_empty() {
            let mut dock = compute_tab_surface_for(app, runtimes, Some(dock_target), geometry.persistent, resize, cell_size);
            for pane in &mut dock.pane_infos { pane.is_focused = false; }
            layout.pane_infos.extend(dock.pane_infos);
            layout.split_borders.extend(dock.split_borders);
        }
        if let Some(focus) = focus {
            for pane in &mut layout.pane_infos { pane.is_focused = pane.id == focus; }
        }
        return layout;
    }
    let main = dock_main_area(area, panes.len());
    let mut layout = compute_tab_surface_for(app, runtimes, target, main, resize, cell_size);
    let Some(target) = target else {
        return layout;
    };
    let ws = &app.workspaces[target.workspace_index];
    for (index, &id) in panes.iter().enumerate() {
        let top = (usize::from(area.height) * index / panes.len()) as u16;
        let bottom = (usize::from(area.height) * (index + 1) / panes.len()) as u16;
        let rect = Rect::new(
            main.right(),
            area.y + top,
            area.width - main.width,
            bottom - top,
        );
        let borders = if app.pane_borders.draws_borders() {
            Borders::ALL
        } else {
            Borders::NONE
        };
        let inner = super::panes::pane_inner_rect(rect, borders);
        let (inner_rect, scrollbar_rect) = app
            .runtime_for_pane_in_workspace(runtimes, target.workspace_index, id)
            .map(|rt| {
                let geometry =
                    super::panes::stable_scrollbar_gutter(rt, inner, app.pane_scrollbars);
                if resize
                    && ws
                        .terminal_id(id)
                        .is_some_and(|id| !app.direct_attach_resize_locks.contains(id))
                {
                    rt.resize(
                        geometry.0.height,
                        geometry.0.width,
                        cell_size.width_px,
                        cell_size.height_px,
                    );
                }
                geometry
            })
            .unwrap_or((inner, None));
        layout.pane_infos.push(PaneInfo {
            id,
            rect,
            inner_rect,
            scrollbar_rect,
            borders,
            is_focused: false,
        });
    }
    if let Some(focus) = focus {
        if layout.pane_infos.iter().any(|pane| pane.id == focus) {
            for pane in &mut layout.pane_infos {
                pane.is_focused = pane.id == focus;
            }
        }
    }
    layout
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dock_geometry_is_bounded_and_empty_dock_is_free() {
        for width in 0..120 {
            let area = Rect::new(2, 3, width, 24);
            assert_eq!(dock_main_area(area, 0), area);
            for count in [1, 15, 32] {
                let main = dock_main_area(area, count);
                assert!(main.right() <= area.right());
                assert_eq!(main.height, area.height);
            }
        }
    }
}
