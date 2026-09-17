use super::*;
use crate::protocol::pane_dock::{PaneDock, TabIdentities, WorkspaceArea};
use crate::protocol::persistent_area::{DockPlacement, PersistentAreaSettings};

fn projected() -> ClientShellSnapshot {
    let mut snap = snapshot();
    let mut tab = snap.tabs[0].clone();
    tab.tab_id = "tab_persistent".into();
    tab.focused = false;
    snap.tabs.push(tab);
    let mut pane = snap.panes[0].clone();
    pane.pane_id = "persistent_pane".into();
    pane.tab_id = "tab_persistent".into();
    snap.panes.push(pane);
    snap
}

fn identities() -> TabIdentities {
    TabIdentities {
        persistent_tab_ids: HashMap::from([("tab_persistent".into(), "durable-tab-token".into())]),
    }
}

fn install_area(
    state: &mut ClientShellState,
    snap: &ClientShellSnapshot,
    placement: DockPlacement,
) {
    state.set_sticky_supported(&ClientEndpointId::Local, true);
    state.accept_persistent_identities(&ClientEndpointId::Local, 1, snap, identities());
    state.set_snapshot(Box::new(snap.clone()));
    let mut settings = PersistentAreaSettings::default();
    settings.placement = placement;
    settings.set_percent(40);
    state.sticky.insert(
        ClientEndpointId::Local,
        PaneDock {
            boot_id: snap.boot_id.clone(),
            areas: HashMap::from([(
                "ws_1".into(),
                WorkspaceArea {
                    tab_id: "tab_persistent".into(),
                    enabled: true,
                    settings,
                },
            )]),
            ..Default::default()
        },
    );
}

#[test]
fn persistent_selection_autoscroll_survives_snapshot_updates() {
    for placement in [DockPlacement::Right, DockPlacement::Bottom] {
        for down in [false, true] {
            let mut state =
                ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
            let snap = projected();
            install_area(&mut state, &snap, placement);
            let mut frame = surface();
            frame.panes[0].pane_id = "persistent_pane".into();
            frame.panes[0].scroll = Some(crate::protocol::PaneSurfaceScrollMetrics {
                offset_from_bottom: 20,
                max_offset_from_bottom: 40,
                viewport_rows: 2,
            });
            state.set_pane_surface(frame);
            state.compose(106, 20).unwrap();
            let hit = state.hits.panes[0].clone();
            let mouse = |kind, row| {
                RawInputEvent::Mouse(crossterm::event::MouseEvent {
                    kind,
                    column: hit.inner_rect.x,
                    row,
                    modifiers: KeyModifiers::empty(),
                })
            };
            state.handle_raw_events(vec![mouse(
                MouseEventKind::Down(MouseButton::Left),
                hit.inner_rect.y + if down { 0 } else { 1 },
            )]);
            state.handle_raw_events(vec![mouse(
                MouseEventKind::Drag(MouseButton::Left),
                if down {
                    hit.inner_rect.bottom()
                } else {
                    hit.inner_rect.y.saturating_sub(1)
                },
            )]);
            assert!(state.selection.as_ref().unwrap().is_dragging());
            let before = state
                .selection_autoscroll
                .as_ref()
                .unwrap()
                .offset_from_bottom;
            // Scroll/output snapshots retain the ordinary tab's focus, not the dock's.
            state.set_snapshot(Box::new(snap.clone()));
            assert!(
                state
                    .selection
                    .as_ref()
                    .is_some_and(crate::selection::Selection::is_dragging),
                "snapshot must not cancel the persistent pane's selection"
            );
            let deadline = state
                .selection_autoscroll_deadline
                .expect("timer survives snapshot");
            state.tick_selection_autoscroll(deadline);
            assert_eq!(
                state
                    .selection_autoscroll
                    .as_ref()
                    .unwrap()
                    .offset_from_bottom,
                if down { before - 1 } else { before + 1 }
            );
            assert_eq!(
                state.snapshot.as_ref().unwrap().focused_tab_id,
                snap.focused_tab_id
            );
            // Real focus loss must still cancel selection and the timer.
            state
                .sticky
                .get_mut(&ClientEndpointId::Local)
                .unwrap()
                .focus = None;
            state.set_snapshot(Box::new(snap));
            assert!(state.selection.is_none());
            assert!(state.selection_autoscroll.is_none());
        }
    }
}

#[test]
fn persistent_split_drag_targets_owning_tab_and_local_ratio_in_both_placements() {
    for placement in [DockPlacement::Right, DockPlacement::Bottom] {
        for direction in [
            PaneSurfaceSplitDirection::Horizontal,
            PaneSurfaceSplitDirection::Vertical,
        ] {
            let mut state =
                ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
            install_area(&mut state, &projected(), placement);
            let bounds = state.layout(126, 42).pane_surface;
            let rect = state
                .persistent_area()
                .unwrap()
                .settings
                .layout(Rect::new(0, 0, bounds.width, bounds.height))
                .persistent;
            let mut frame = surface();
            let horizontal = direction == PaneSurfaceSplitDirection::Horizontal;
            let origin = if horizontal { rect.x } else { rect.y };
            let length = if horizontal { rect.width } else { rect.height };
            let pos = origin + length / 2;
            frame.splits = vec![PaneSurfaceSplit {
                direction,
                pos,
                area: SurfaceRect {
                    x: rect.x,
                    y: rect.y,
                    width: rect.width,
                    height: rect.height,
                },
                hit_rect: if horizontal {
                    SurfaceRect {
                        x: pos,
                        y: rect.y,
                        width: 1,
                        height: rect.height,
                    }
                } else {
                    SurfaceRect {
                        x: rect.x,
                        y: pos,
                        width: rect.width,
                        height: 1,
                    }
                },
                path: vec![false, true],
            }];
            state.set_pane_surface(frame);
            state.compose(126, 42).unwrap();
            let hit = state.hits.pane_splits[0].clone();
            let start = (hit.hit_rect.x, hit.hit_rect.y);
            let mouse = |kind, point: (u16, u16)| {
                RawInputEvent::Mouse(crossterm::event::MouseEvent {
                    kind,
                    column: point.0,
                    row: point.1,
                    modifiers: KeyModifiers::empty(),
                })
            };
            state.handle_raw_events(vec![mouse(MouseEventKind::Down(MouseButton::Left), start)]);
            assert!(matches!(
                state.chrome_drag,
                Some(ClientChromeDrag::PaneSplit { .. })
            ));
            let end = if horizontal {
                (hit.area.x + length * 3 / 4, start.1)
            } else {
                (start.0, hit.area.y + length * 3 / 4)
            };
            let drag =
                state.handle_raw_events(vec![mouse(MouseEventKind::Drag(MouseButton::Left), end)]);
            assert!(
                matches!(&drag.actions[..], [ClientShellAction::Endpoint { request, .. }]
                if matches!(&request.method, crate::api::schema::Method::LayoutSetSplitRatio(params)
                    if params.tab_id.as_deref() == Some("tab_persistent") && params.path == vec![false, true]
                    && (params.ratio - f32::from(length * 3 / 4) / f32::from(length)).abs() < 0.001))
            );
            let release_end = if horizontal {
                (end.0 + 1, end.1)
            } else {
                (end.0, end.1 + 1)
            };
            let release = state.handle_raw_events(vec![mouse(
                MouseEventKind::Up(MouseButton::Left),
                release_end,
            )]);
            assert_eq!(release.actions.len(), 1, "release flushes final ratio");
            assert!(state.chrome_drag.is_none());
        }
    }
}

#[test]
fn persistent_preferences_survive_client_and_server_restart_without_creation() {
    let path = std::env::temp_dir().join(format!(
        "herdr-persistent-{}.json",
        crate::workspace::new_persistent_id()
    ));
    let config =
        || ClientShellConfig::from_config(&Config::default()).with_preferences_path(path.clone());
    let mut original = ClientShellState::new(config());
    let mut snap = projected();
    install_area(&mut original, &snap, DockPlacement::Bottom);
    original.send_sticky(&mut ClientShellInput::default());
    let saved = preferences::load(&path).unwrap();
    assert_eq!(saved.persistent_areas.len(), 1);
    assert_eq!(
        saved.persistent_areas[0].persistent_tab_id,
        "durable-tab-token"
    );
    for new_boot in [false, true] {
        let mut restored = ClientShellState::new(config());
        if new_boot {
            snap.boot_id = "new-boot".into();
        }
        restored.set_sticky_supported(&ClientEndpointId::Local, true);
        restored.accept_persistent_identities(&ClientEndpointId::Local, 2, &snap, identities());
        restored.set_snapshot(Box::new(snap.clone()));
        let area = restored.persistent_area().unwrap();
        assert_eq!(area.settings, saved.persistent_areas[0].area.settings);
        assert!(restored.persistent_tab_is_hidden("tab_persistent"));
        assert!(
            restored.pending_requests.is_empty(),
            "restore never creates a tab"
        );
        assert!(restored.persistent_replay().is_some());
        assert!(restored.persistent_replay().is_none(), "no feedback loop");
        restored.set_sticky_supported(&ClientEndpointId::Local, true);
        restored.accept_persistent_identities(&ClientEndpointId::Local, 3, &snap, identities());
        assert!(
            restored.persistent_replay().is_some(),
            "same-boot reconnect replays"
        );
        restored.toggle_persistent_area(&mut ClientShellInput::default());
        assert!(!restored.persistent_area().unwrap().enabled);
        restored.toggle_persistent_area(&mut ClientShellInput::default());
        assert!(restored.pending_requests.is_empty(), "reuse backing tab");
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn persistent_restore_rejects_recycled_ids_missing_capability_and_other_endpoints() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    let snap = projected();
    install_area(&mut state, &snap, DockPlacement::Right);
    state.send_sticky(&mut ClientShellInput::default());
    let prefs = state.config.preferences.clone();
    for (supported, endpoint, token) in [
        (false, "local", "durable-tab-token"),
        (true, "other", "durable-tab-token"),
        (true, "local", "new-unrelated-tab"),
    ] {
        let mut config = ClientShellConfig::from_config(&Config::default());
        config.preferences = prefs.clone();
        config.preferences.persistent_areas[0].endpoint = endpoint.into();
        let mut restored = ClientShellState::new(config);
        restored.set_sticky_supported(&ClientEndpointId::Local, supported);
        let mut ids = identities();
        ids.persistent_tab_ids
            .insert("tab_persistent".into(), token.into());
        restored.accept_persistent_identities(&ClientEndpointId::Local, 1, &snap, ids);
        restored.set_snapshot(Box::new(snap.clone()));
        assert!(restored.persistent_area().is_none());
        assert!(restored.pending_requests.is_empty());
    }
}
