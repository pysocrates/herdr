//! Client-owned sticky selection. The optional server control supplies live pane
//! presentation, not a second terminal or a mutation of shared tab membership.
use super::*;
use crate::protocol::pane_dock::PaneDock;
#[cfg(test)]
use crate::protocol::pane_dock::MAX_PANES;

impl ClientShellState {
    pub(crate) fn set_sticky_supported(&mut self, endpoint: &ClientEndpointId, supported: bool) {
        if supported {
            self.sticky_supported.insert(endpoint.clone());
        } else {
            self.sticky_supported.remove(endpoint);
            self.sticky.remove(endpoint);
        }
    }

    pub(super) fn pane_is_sticky(&self, pane: &str) -> bool {
        if let Some(area) = self.persistent_area() {
            return area.enabled && self.snapshot.as_deref().is_some_and(|snapshot| snapshot.panes.iter().any(|p| p.pane_id == pane && p.tab_id == area.tab_id));
        }
        self.sticky
            .get(&self.active_endpoint_id)
            .is_some_and(|dock| {
                self.snapshot
                    .as_deref()
                    .is_some_and(|snapshot| snapshot.boot_id == dock.boot_id)
                    && dock.pane_ids.iter().any(|id| id == pane)
            })
    }

    pub(super) fn send_sticky(&self, outcome: &mut ClientShellInput) {
        let Some(dock) = self.sticky.get(&self.active_endpoint_id) else {
            return;
        };
        if let Ok(data) = serde_json::to_string(dock) {
            outcome.requests.push(ClientMessage::EndpointControl {
                kind: crate::protocol::pane_dock::KIND.into(),
                data,
            });
            outcome.repaint = true;
        }
    }

    #[cfg(test)]
    pub(super) fn toggle_sticky(&mut self, pane: String, outcome: &mut ClientShellInput) {
        if !self.sticky_supported.contains(&self.active_endpoint_id) {
            return;
        }
        let Some(snapshot) = self.snapshot.as_deref() else {
            return;
        };
        if !snapshot.panes.iter().any(|p| p.pane_id == pane) {
            return;
        }
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
        if dock.pane_ids.contains(&pane) {
            dock.pane_ids.retain(|id| id != &pane);
            if dock.focus.as_ref().is_some_and(|(_, id)| id == &pane) {
                dock.focus = None;
            }
        } else if dock.pane_ids.len() < MAX_PANES {
            dock.pane_ids.push(pane);
        } else {
            self.set_endpoint_error("At most 32 panes can be sticky in one connection");
            return;
        }
        self.send_sticky(outcome);
    }

    /// A mouse focus on an extra pane must not navigate to its owning tab.
    pub(super) fn focus_sticky(&mut self, pane: &str, outcome: &mut ClientShellInput) -> bool {
        let Some(snapshot) = self.snapshot.as_deref() else {
            return false;
        };
        let extra = self.pane_is_sticky(pane)
            && snapshot.panes.iter().any(|p| {
                p.pane_id == pane
                    && Some(&p.workspace_id) == snapshot.focused_workspace_id.as_ref()
                    && Some(&p.tab_id) != snapshot.focused_tab_id.as_ref()
            });
        if extra {
            let focus = snapshot
                .focused_tab_id
                .clone()
                .map(|tab| (tab, pane.to_owned()));
            if let Some(dock) = self.sticky.get_mut(&self.active_endpoint_id) {
                dock.focus = focus;
            }
            self.send_sticky(outcome);
        } else if let Some(dock) = self.sticky.get_mut(&self.active_endpoint_id) {
            if dock.focus.take().is_some() {
                self.send_sticky(outcome);
            }
        }
        extra
    }

    pub(super) fn sticky_focus(&self) -> Option<String> {
        let snapshot = self.snapshot.as_deref()?;
        let dock = self.sticky.get(&self.active_endpoint_id)?;
        let (tab, pane) = dock.focus.as_ref()?;
        (dock.boot_id == snapshot.boot_id
            && Some(tab) == snapshot.focused_tab_id.as_ref()
            && snapshot.panes.iter().any(|p| &p.pane_id == pane))
        .then(|| pane.clone())
    }

    pub(super) fn reconcile_sticky(&mut self, snapshot: &ClientShellSnapshot) {
        if let Some(dock) = self.sticky.get_mut(&self.active_endpoint_id) {
            if dock.boot_id != snapshot.boot_id {
                *dock = PaneDock {
                    boot_id: snapshot.boot_id.clone(),
                    ..Default::default()
                };
                return;
            }
            if !dock.areas.is_empty() {
                dock.pane_ids = snapshot.panes.iter().filter(|pane| dock.areas.get(&pane.workspace_id).is_some_and(|area| area.enabled && area.tab_id == pane.tab_id)).map(|pane| pane.pane_id.clone()).collect();
            }
            dock.pane_ids
                .retain(|id| snapshot.panes.iter().any(|pane| &pane.pane_id == id));
            if dock.focus.as_ref().is_some_and(|(tab, pane)| {
                Some(tab) != snapshot.focused_tab_id.as_ref() || !dock.pane_ids.contains(pane)
            }) {
                dock.focus = None;
            }
        }
    }
}
