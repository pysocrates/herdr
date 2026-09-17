//! Optional, connection-local pane presentation. Not part of a frozen core codec.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceArea {
    pub tab_id: String,
    pub enabled: bool,
    #[serde(default)]
    pub settings: super::persistent_area::PersistentAreaSettings,
}

pub const CAPABILITY: &str = "pane_dock_v1";
pub const KIND: &str = "shell.pane-dock.v1";
pub const MAX_PANES: usize = 32;

/// Optional JSON snapshot extension. Kept outside the frozen binary snapshot.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TabIdentities {
    #[serde(default)]
    pub persistent_tab_ids: std::collections::HashMap<String, String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaneDock {
    pub boot_id: String,
    #[serde(default)]
    pub areas: std::collections::HashMap<String, WorkspaceArea>,
    pub pane_ids: Vec<String>,
    #[serde(default)]
    pub focus: Option<(String, String)>, // (viewed tab, focused pane)
}
