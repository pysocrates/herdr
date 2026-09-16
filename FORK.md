# Persistent-area fork

This is a personal fork of [herdrdev/herdr](https://github.com/herdrdev/herdr),
with a persistent terminal area alongside the normal tabbed workspace.
The original project attribution and Apache-2.0 license are retained.

## Build and launch

The feature lives on `feature/persistent-area`.

```sh
git clone --branch feature/persistent-area https://github.com/pysocrates/herdr.git
cd herdr
mise trust
mise install
mise exec -- cargo build --locked -j 3
./target/debug/herdr
```

## Use

- Open a workspace and click **Persistent area +** beside the tabs.
- The area starts on the right; **rotate** places it at the bottom.
- Drag the divider to adjust its size.
- Switch normal tabs while the persistent terminal remains visible and interactive.
- Right-click a pane to move it to the persistent area or back to the tabbed area.
- Hide/reopen the area without terminating its shell.

The area uses a backing tab named `Persistent`. That tab is hidden from the tab
bar while the area is enabled and is available as a normal tab when collapsed.
Presentation settings are client-local and workspace-scoped, not a promise of
cross-restart layout persistence. The server and client must support the optional
pane-dock capability; frozen generation-1 endpoint codecs are unchanged.

## Verification and limits

Targeted geometry, menu, cleanup, cross-tab focus, and server live-pane tests pass.
A real-PTY smoke test also checks creation, tab switching, correctly targeted
terminal input without tab navigation, right/bottom placement, and hide/reopen.

This is an experimental feature, not an upstream release. Divider dragging,
move-last-pane behavior, advanced mouse/clipboard operations, reconnects, and
multi-client geometry need more exhaustive regression coverage. No upstream pull
request is implied by this fork.
