//! Durable presentation preferences. Only opaque session identities cross boots;
//! leases, focus and public pane IDs are always rebuilt from the current snapshot.
use super::*;
use crate::protocol::pane_dock::{PaneDock, TabIdentities};

impl ClientShellState {
    pub(crate) fn accept_persistent_identities(
        &mut self,
        endpoint: &ClientEndpointId,
        generation: u64,
        snapshot: &ClientShellSnapshot,
        identities: TabIdentities,
    ) {
        if !self.sticky_supported.contains(endpoint) {
            return;
        }
        if self
            .persistent_identities
            .get(endpoint)
            .is_some_and(|(boot, old, revision, _)| {
                *old > generation
                    || (*old == generation
                        && boot == &snapshot.boot_id
                        && *revision > snapshot.revision)
            })
        {
            return;
        }
        if self
            .persistent_identities
            .get(endpoint)
            .is_some_and(|(boot, old, _, _)| *old != generation || boot != &snapshot.boot_id)
        {
            self.sticky_replayed.remove(endpoint);
        }
        let previous = self.persistent_identities.insert(
            endpoint.clone(),
            (
                snapshot.boot_id.clone(),
                generation,
                snapshot.revision,
                identities.persistent_tab_ids,
            ),
        );
        let ids = &self.persistent_identities[endpoint].3;
        let dock = self.sticky.entry(endpoint.clone()).or_default();
        if dock.boot_id != snapshot.boot_id {
            *dock = PaneDock {
                boot_id: snapshot.boot_id.clone(),
                ..Default::default()
            };
        }
        // A create response may precede the snapshot introducing its tab. Only
        // retire tabs previously observed, not a just-created backing resource.
        dock.areas.retain(|workspace, area| {
            !previous
                .as_ref()
                .is_some_and(|(_, _, _, ids)| ids.contains_key(&area.tab_id))
                || snapshot
                    .tabs
                    .iter()
                    .any(|tab| tab.tab_id == area.tab_id && &tab.workspace_id == workspace)
        });
        for saved in &self.config.preferences.persistent_areas {
            if saved.endpoint != endpoint.storage_key()
                || dock.areas.contains_key(&saved.workspace_id)
            {
                continue;
            }
            // Never infer ownership from a label, a recycled public number or tab order.
            let mut matches = snapshot.tabs.iter().filter(|tab| {
                tab.workspace_id == saved.workspace_id
                    && ids.get(&tab.tab_id) == Some(&saved.persistent_tab_id)
            });
            if let Some(tab) = matches.next().filter(|_| matches.next().is_none()) {
                let mut area = saved.area.clone();
                area.tab_id = tab.tab_id.clone();
                dock.areas.insert(saved.workspace_id.clone(), area);
            }
        }
        if !dock.areas.is_empty() {
            dock.pane_ids = snapshot
                .panes
                .iter()
                .filter(|pane| {
                    dock.areas
                        .get(&pane.workspace_id)
                        .is_some_and(|area| area.enabled && area.tab_id == pane.tab_id)
                })
                .map(|pane| pane.pane_id.clone())
                .collect();
        }
    }

    pub(super) fn persist_persistent_areas(&mut self, outcome: &mut ClientShellInput) {
        let endpoint = &self.active_endpoint_id;
        let Some(dock) = self.sticky.get(endpoint) else {
            return;
        };
        let Some((boot, _, _, ids)) = self.persistent_identities.get(endpoint) else {
            return;
        };
        if boot != &dock.boot_id || !self.sticky_supported.contains(endpoint) {
            return;
        }
        let key = endpoint.storage_key();
        let saved = &mut self.config.preferences.persistent_areas;
        let mut changed = false;
        for (workspace, area) in &dock.areas {
            let Some(identity) = ids.get(&area.tab_id).filter(|id| !id.is_empty()) else {
                continue;
            };
            let next = preferences::PersistentAreaPreference {
                endpoint: key.clone(),
                workspace_id: workspace.clone(),
                persistent_tab_id: identity.clone(),
                area: area.clone(),
            };
            if let Some(previous) = saved
                .iter_mut()
                .find(|saved| saved.endpoint == key && saved.workspace_id == *workspace)
            {
                if *previous != next {
                    *previous = next;
                    changed = true;
                }
            } else {
                saved.push(next);
                changed = true;
            }
        }
        if changed {
            self.persist_chrome_preferences(outcome);
        }
    }

    /// Called after snapshot installation and surface activation. Comparing payloads
    /// avoids snapshot/control feedback loops; negotiation resets the connection cache.
    pub(crate) fn persistent_replay(&mut self) -> Option<ClientMessage> {
        if !self.sticky_supported.contains(&self.active_endpoint_id) {
            return None;
        }
        if !self.persistent_drag {
            self.persist_persistent_areas(&mut ClientShellInput::default());
        }
        let dock = self.sticky.get(&self.active_endpoint_id)?;
        let snapshot = self.snapshot.as_deref()?;
        if dock.boot_id != snapshot.boot_id
            || self.sticky_replayed.get(&self.active_endpoint_id) == Some(dock)
        {
            return None;
        }
        let data = serde_json::to_string(dock).ok()?;
        self.sticky_replayed
            .insert(self.active_endpoint_id.clone(), dock.clone());
        Some(ClientMessage::EndpointControl {
            kind: crate::protocol::pane_dock::KIND.into(),
            data,
        })
    }
}
