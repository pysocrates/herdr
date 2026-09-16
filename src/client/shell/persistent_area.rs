//! First-class workspace area, independent of the currently selected tab.
use super::*;
use crate::protocol::pane_dock::{PaneDock, WorkspaceArea};
use crate::protocol::persistent_area::DockPlacement;

impl ClientShellState {
    pub(super) fn persistent_area(&self) -> Option<&WorkspaceArea> {
        let snapshot = self.snapshot.as_deref()?;
        let dock = self.sticky.get(&self.active_endpoint_id)?;
        if dock.boot_id != snapshot.boot_id {
            return None;
        }
        dock.areas.get(snapshot.focused_workspace_id.as_ref()?)
    }

    pub(super) fn persistent_tab_is_hidden(&self, tab: &str) -> bool {
        self.persistent_area()
            .is_some_and(|area| area.enabled && area.tab_id == tab)
    }

    pub(super) fn toggle_persistent_area(&mut self, outcome: &mut ClientShellInput) {
        if !self.sticky_supported.contains(&self.active_endpoint_id) {
            return;
        }
        let Some(snapshot) = self.snapshot.as_deref() else {
            return;
        };
        let Some(workspace) = snapshot.focused_workspace_id.clone() else {
            return;
        };
        if self.pending_requests.values().any(|pending| matches!(
            &pending.kind, PendingEndpointKind::PersistentCreate { workspace_id, .. } if workspace_id == &workspace
        )) { return; }
        let existing = self
            .persistent_area()
            .filter(|area| snapshot.tabs.iter().any(|tab| tab.tab_id == area.tab_id))
            .cloned();
        if let Some(area) = existing {
            if let Some(dock) = self.sticky.get_mut(&self.active_endpoint_id) {
                if let Some(stored) = dock.areas.get_mut(&workspace) {
                    stored.enabled = !area.enabled;
                }
                dock.focus = None;
            }
            self.send_sticky(outcome);
        } else {
            self.create_persistent_area(workspace, None, outcome);
        }
        outcome.repaint = true;
    }

    fn create_persistent_area(
        &mut self,
        workspace: String,
        move_pane: Option<String>,
        outcome: &mut ClientShellInput,
    ) {
        self.push_endpoint_method_with_kind(
            crate::api::schema::Method::TabCreate(crate::api::schema::TabCreateParams {
                workspace_id: Some(workspace.clone()),
                cwd: None,
                label: Some("Persistent".into()),
                focus: false,
                env: Default::default(),
            }),
            PendingEndpointKind::PersistentCreate {
                workspace_id: workspace,
                move_pane,
            },
            outcome,
        );
    }

    pub(super) fn move_to_persistent_area(&mut self, pane: String, outcome: &mut ClientShellInput) {
        let Some(snapshot) = self.snapshot.as_deref() else {
            return;
        };
        let Some(source) = snapshot.panes.iter().find(|p| p.pane_id == pane) else {
            return;
        };
        let workspace = source.workspace_id.clone();
        let area = self.persistent_area().cloned();
        if let Some(area) = area {
            let destination = if source.tab_id == area.tab_id {
                snapshot
                    .focused_tab_id
                    .clone()
                    .filter(|tab| tab != &area.tab_id)
            } else {
                Some(area.tab_id.clone())
            };
            let Some(tab_id) = destination else {
                return;
            };
            self.push_endpoint_method(
                crate::api::schema::Method::PaneMove(crate::api::schema::PaneMoveParams {
                    pane_id: pane,
                    destination: crate::api::schema::PaneMoveDestination::Tab {
                        tab_id,
                        target_pane_id: None,
                        split: crate::api::schema::SplitDirection::Down,
                        ratio: None,
                    },
                    focus: false,
                }),
                outcome,
            );
            if let Some(dock) = self.sticky.get_mut(&self.active_endpoint_id) {
                if let Some(area) = dock.areas.get_mut(&workspace) {
                    area.enabled = true;
                }
                dock.focus = None;
            }
            self.send_sticky(outcome);
        } else {
            self.create_persistent_area(workspace, Some(pane), outcome);
        }
    }

    pub(super) fn complete_persistent_create(
        &mut self,
        workspace: String,
        move_pane: Option<String>,
        result: Result<crate::api::schema::ResponseResult, ClientShellEndpointError>,
    ) -> (bool, Vec<ClientShellAction>) {
        let Ok(crate::api::schema::ResponseResult::TabCreated { tab, root_pane }) = result else {
            return (true, Vec::new());
        };
        let Some(snapshot) = self.snapshot.as_deref() else {
            return (false, Vec::new());
        };
        let dock = self
            .sticky
            .entry(self.active_endpoint_id.clone())
            .or_default();
        if dock.boot_id != snapshot.boot_id {
            *dock = PaneDock {
                boot_id: snapshot.boot_id.clone(),
                ..Default::default()
            };
        }
        dock.areas.insert(
            workspace,
            WorkspaceArea {
                tab_id: tab.tab_id.clone(),
                enabled: true,
                ..Default::default()
            },
        );
        dock.pane_ids.push(root_pane.pane_id);
        let mut actions = vec![ClientShellAction::PaneDock {
            endpoint_id: self.active_endpoint_id.clone(),
            dock: dock.clone(),
        }];
        if let Some(pane_id) = move_pane {
            let mut outcome = ClientShellInput::default();
            self.push_endpoint_method(
                crate::api::schema::Method::PaneMove(crate::api::schema::PaneMoveParams {
                    pane_id,
                    destination: crate::api::schema::PaneMoveDestination::Tab {
                        tab_id: tab.tab_id,
                        target_pane_id: None,
                        split: crate::api::schema::SplitDirection::Down,
                        ratio: None,
                    },
                    focus: false,
                }),
                &mut outcome,
            );
            actions.extend(outcome.actions);
        }
        (true, actions)
    }

    pub(super) fn change_persistent_placement(&mut self, outcome: &mut ClientShellInput) {
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
            area.settings.placement = match area.settings.placement {
                DockPlacement::Right => DockPlacement::Bottom,
                DockPlacement::Bottom => DockPlacement::Right,
            };
        }
        self.send_sticky(outcome);
    }
}
