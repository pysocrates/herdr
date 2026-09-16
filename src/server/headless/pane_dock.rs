//! Connection-local presentation only: terminals and shared tab layouts never move.
use super::*;

impl HeadlessServer {
    pub(super) fn set_pane_dock(
        &mut self,
        client_id: u64,
        mut dock: protocol::pane_dock::PaneDock,
    ) -> bool {
        if dock.boot_id != self.client_shell_boot_id
            || dock.pane_ids.len() > protocol::pane_dock::MAX_PANES
            || !self
                .clients
                .get(&client_id)
                .is_some_and(ClientConnection::is_active_shell_client)
        {
            return false;
        }
        if dock.areas.len() > 128 { return false; }
        dock.areas.retain(|workspace, area| {
            self.app.parse_tab_id(&area.tab_id).is_some_and(|(ws, _)| self.app.public_workspace_id(ws) == *workspace)
        });
        if !dock.areas.is_empty() {
            dock.pane_ids.clear();
            for area in dock.areas.values().filter(|area| area.enabled) {
                if let Some((ws, tab)) = self.app.parse_tab_id(&area.tab_id) {
                    for pane in self.app.state.workspaces[ws].tabs[tab].layout.pane_ids() {
                        if let Some(id) = self.app.public_pane_id(ws, pane) { dock.pane_ids.push(id); }
                    }
                }
            }
        }
        let mut seen = HashSet::new();
        dock.pane_ids
            .retain(|id| self.app.parse_pane_id(id).is_some() && seen.insert(id.clone()));
        if let Some((tab_id, pane_id)) = &dock.focus {
            let valid = self
                .app
                .parse_tab_id(tab_id)
                .zip(self.app.parse_pane_id(pane_id))
                .is_some_and(|((ws, _), (pane_ws, _))| ws == pane_ws)
                && dock.pane_ids.contains(pane_id)
                && self.shell_tab_id_for_client(client_id).as_ref() == Some(tab_id);
            if !valid {
                dock.focus = None;
            }
        }
        let client = self.clients.get_mut(&client_id).expect("validated client");
        if client.pane_dock == dock {
            return false;
        }
        let geometry_changed = client.pane_dock.pane_ids != dock.pane_ids || client.pane_dock.areas != dock.areas;
        client.pane_dock = dock;
        client.shell_snapshot = None;
        client.request_recompute();
        if geometry_changed { self.apply_shell_tab_geometry(client_id, false); }
        true
    }

    /// Only these additional, visible sources widen output fanout. Unrelated hidden
    /// tabs retain the existing early exit. Work is O(sticky panes), never all terminals.
    pub(super) fn persistent_target(&self, client_id: u64) -> Option<(crate::ui::TabSurfaceTarget, protocol::persistent_area::PersistentAreaSettings)> {
        let client = self.clients.get(&client_id)?;
        let target = self.shell_target_for_client(client_id)?;
        client.pane_dock.areas.values().find_map(|area| {
            if !area.enabled { return None; }
            let (workspace_index, tab_index) = self.app.parse_tab_id(&area.tab_id)?;
            (workspace_index == target.workspace_index && tab_index != target.tab_index).then_some((crate::ui::TabSurfaceTarget { workspace_index, tab_index }, area.settings))
        })
    }

    pub(super) fn docked_panes(&self, client_id: u64) -> Vec<crate::layout::PaneId> {
        if let Some((target, _)) = self.persistent_target(client_id) {
            return self.app.state.workspaces[target.workspace_index].tabs[target.tab_index].layout.pane_ids();
        }
        if self.clients.get(&client_id).is_some_and(|client| !client.pane_dock.areas.is_empty()) { return Vec::new(); }
        let Some(client) = self.clients.get(&client_id) else {
            return Vec::new();
        };
        if client.pane_dock.pane_ids.is_empty() {
            return Vec::new();
        }
        let Some(target) = self.shell_target_for_client(client_id) else {
            return Vec::new();
        };
        let tab = &self.app.state.workspaces[target.workspace_index].tabs[target.tab_index];
        client
            .pane_dock
            .pane_ids
            .iter()
            .filter_map(|id| {
                let (ws, pane) = self.app.parse_pane_id(id)?;
                (ws == target.workspace_index && !tab.panes.contains_key(&pane)).then_some(pane)
            })
            .collect()
    }

    pub(super) fn dock_focus(&self, client_id: u64) -> Option<crate::layout::PaneId> {
        let client = self.clients.get(&client_id)?;
        let (tab, pane) = client.pane_dock.focus.as_ref()?;
        if self.shell_tab_id_for_client(client_id).as_ref() != Some(tab) {
            return None;
        }
        let target = self.shell_target_for_client(client_id)?;
        let (ws, pane) = self.app.parse_pane_id(pane)?;
        (ws == target.workspace_index).then_some(pane)
    }
}
