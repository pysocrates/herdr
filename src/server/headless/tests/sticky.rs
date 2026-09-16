use super::*;

#[tokio::test]
async fn sticky_panes_render_live_accept_input_and_do_not_move_between_tabs() {
    let mut server = test_headless_server();
    let mut ws = crate::workspace::Workspace::test_new("sticky");
    ws.test_add_tab(Some("other"));
    ws.switch_tab(0);
    server.app.state.workspaces = vec![ws, crate::workspace::Workspace::test_new("elsewhere")];
    server.app.state.ensure_test_terminals();
    server.app.state.active = Some(0);
    server.app.state.selected = 0;
    server.app.state.mode = crate::app::Mode::Terminal;
    let pane = server.app.state.workspaces[0].tabs[0].root_pane;
    let terminal_id = server.app.state.workspaces[0]
        .terminal_id(pane)
        .unwrap()
        .clone();
    let (runtime, mut input_rx) = crate::terminal::TerminalRuntime::test_with_channel(80, 24);
    runtime.test_process_pty_bytes(b"STICKY LIVE OUTPUT");
    server
        .app
        .terminal_runtimes
        .insert(terminal_id.clone(), runtime);
    let pane_id = server.app.public_pane_id(0, pane).unwrap();
    let home_tab = server.app.public_tab_id(0, 0).unwrap();
    let other_tab = server.app.public_tab_id(0, 1).unwrap();
    let elsewhere = server.app.public_tab_id(1, 0).unwrap();
    let (_control, render_rx) = connect_test_shell(&mut server, 7, 80, 24);
    let dock = crate::protocol::pane_dock::PaneDock {
        boot_id: server.client_shell_boot_id.clone(),
        pane_ids: vec![pane_id.clone()],
        focus: None,
        ..Default::default()
    };
    assert!(server.set_pane_dock(7, dock.clone()));
    server.render_and_stream();
    let surface = recv_pane_surface(&render_rx, "home surface");
    assert_eq!(
        surface
            .panes
            .iter()
            .filter(|p| p.pane_id == pane_id)
            .count(),
        1
    );
    assert!(server.docked_panes(7).is_empty(), "no duplicate in own tab");

    assert!(server.focus_shell_client_on_tab(7, &other_tab));
    server.apply_shell_tab_geometry(7, false);
    server.render_and_stream();
    let surface = recv_pane_surface(&render_rx, "sticky surface");
    assert_eq!(surface.panes.len(), 2);
    assert!(frame_text(&surface.frame).contains("STICKY LIVE OUTPUT"));
    let shown = surface.panes.iter().find(|p| p.pane_id == pane_id).unwrap();
    assert!(shown.rect.x >= 40);
    assert_eq!(
        server
            .app
            .terminal_runtimes
            .get(&terminal_id)
            .unwrap()
            .current_size(),
        (shown.inner_rect.height, shown.inner_rect.width)
    );
    assert!(server.pty_sources_visible_to_any_render_target(&HashSet::from([pane])));
    assert!(server.shell_client_views_pane(7, 0, pane));

    let mut focused = dock.clone();
    focused.focus = Some((other_tab.clone(), pane_id.clone()));
    assert!(server.set_pane_dock(7, focused));
    server.handle_server_event(ServerEvent::ClientShellPaneInput {
        client_id: 7,
        pane_id: pane_id.clone(),
        events: vec![crate::protocol::ClientPaneInputEvent::TextCommit(
            "hello".into(),
        )],
    });
    assert_eq!(input_rx.try_recv().unwrap(), Bytes::from_static(b"hello"));
    assert_eq!(server.shell_tab_id_for_client(7).as_ref(), Some(&other_tab));
    assert_eq!(server.app.public_tab_id(0, 0).as_ref(), Some(&home_tab));
    assert_eq!(
        server.app.state.workspaces[0].tabs[0].terminal_id(pane),
        Some(&terminal_id)
    );

    server
        .app
        .terminal_runtimes
        .get(&terminal_id)
        .unwrap()
        .test_process_pty_bytes(b"\r\nUPDATED");
    server.render_and_stream();
    let surface = recv_pane_surface(&render_rx, "updated sticky");
    assert!(frame_text(&surface.frame).contains("UPDATED"));
    assert!(surface
        .panes
        .iter()
        .any(|p| p.pane_id == pane_id && p.focused));

    assert!(server.focus_shell_client_on_tab(7, &elsewhere));
    server.apply_shell_tab_geometry(7, false);
    assert!(server.docked_panes(7).is_empty());
    assert!(!server.shell_client_views_pane(7, 0, pane));
    assert!(!server.pty_sources_visible_to_any_render_target(&HashSet::from([pane])));
    assert!(server.focus_shell_client_on_tab(7, &other_tab));
    let mut unstick = dock;
    unstick.pane_ids.clear();
    assert!(server.set_pane_dock(7, unstick));
    server.render_and_stream();
    let surface = recv_pane_surface(&render_rx, "unstuck surface");
    assert_eq!(surface.panes.len(), 1);
    assert_eq!(surface.panes[0].rect.width, 80);
    shutdown_test_runtimes(&mut server);
}
