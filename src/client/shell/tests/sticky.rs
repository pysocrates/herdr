use super::*;

fn sticky_shell() -> ClientShellState {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    state.set_sticky_supported(&ClientEndpointId::Local, true);
    state
}

#[test]
fn persistent_menu_creates_an_area_and_queues_move() {
    let mut state = sticky_shell();
    state.open_pane_context_menu("pane_1".into(), 2, 2);
    let Some(ClientShellOverlay::ContextMenu(menu)) = &state.overlay else { panic!("menu"); };
    let index = menu.items().iter().position(|item| item.label == "Move to persistent area").unwrap();
    let mut outcome = ClientShellInput::default();
    state.activate_context_menu_item(index, &mut outcome);
    assert!(matches!(&outcome.actions[..], [ClientShellAction::Endpoint { request, .. }] if matches!(&request.method, crate::api::schema::Method::TabCreate(params) if !params.focus)));
    assert!(state.pending_requests.values().any(|pending| matches!(&pending.kind, PendingEndpointKind::PersistentCreate { move_pane: Some(pane), .. } if pane == "pane_1")));
}

#[test]
fn sticky_focus_does_not_navigate_and_is_scoped_to_workspace_and_tab() {
    let mut state = sticky_shell();
    state.toggle_sticky("pane_1".into(), &mut ClientShellInput::default());
    let mut update = snapshot();
    let mut second = update.panes[0].clone();
    second.pane_id = "pane_2".into();
    second.tab_id = "tab_2".into();
    update.panes.push(second);
    update.focused_tab_id = Some("tab_2".into());
    update.focused_pane_id = Some("pane_2".into());
    state.set_snapshot(Box::new(update.clone()));
    let mut outcome = ClientShellInput::default();
    state.push_endpoint_method(
        crate::api::schema::Method::PaneFocus(crate::api::schema::PaneTarget {
            pane_id: "pane_1".into(),
        }),
        &mut outcome,
    );
    assert!(
        outcome.actions.is_empty(),
        "must not invoke shared PaneFocus"
    );
    assert_eq!(state.focused_pane_id().as_deref(), Some("pane_1"));
    assert_eq!(
        state.snapshot.as_ref().unwrap().focused_tab_id.as_deref(),
        Some("tab_2")
    );
    update.focused_workspace_id = Some("ws_2".into());
    update.focused_tab_id = Some("tab_3".into());
    state.set_snapshot(Box::new(update));
    assert!(!state.focus_sticky("pane_1", &mut outcome));
    assert!(state.sticky_focus().is_none());
}

#[test]
fn sticky_cleanup_handles_close_restart_and_missing_capability() {
    let mut state = sticky_shell();
    state.toggle_sticky("pane_1".into(), &mut ClientShellInput::default());
    let mut update = snapshot();
    update.panes.clear();
    state.set_snapshot(Box::new(update));
    assert!(!state.pane_is_sticky("pane_1"));
    state.set_snapshot(Box::new(snapshot()));
    state.toggle_sticky("pane_1".into(), &mut ClientShellInput::default());
    let mut update = snapshot();
    update.boot_id = "next-boot".into();
    state.set_snapshot(Box::new(update));
    assert!(!state.pane_is_sticky("pane_1"));
    state.set_sticky_supported(&ClientEndpointId::Local, false);
    state.open_pane_context_menu("pane_1".into(), 2, 2);
    let Some(ClientShellOverlay::ContextMenu(menu)) = &state.overlay else {
        panic!("menu");
    };
    assert!(!menu
        .items()
        .iter()
        .any(|item| item.action == ClientContextMenuAction::ToggleSticky));
}
