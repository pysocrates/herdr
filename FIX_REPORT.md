# Persistent-area fix report

Branch: `feature/persistent-area` in the personal `pysocrates/herdr` fork.
This change is not an upstream release or pull request.

## 1. Persistent pane split resizing

**Symptom:** dragging an internal divider in the persistent area did not change
pane dimensions, although ordinary tabbed panes could be resized.

**Cause:** mouse-down identified the persistent backing tab, but later drag
validation required the target to be the selected ordinary tab.

**Fix:** validate the owning layout region while retaining topology checks.
Persistent splits now resize in either direction. Outer area divider changes
remain supported, with preference writes deferred until drag release.

## 2. Persistent-area restoration

**Symptom:** after reconnecting or restarting, the persistent backing tab appeared
as an ordinary tab and the area had to be configured again.

**Cause:** presentation membership and settings were connection-local. Public tab
numbers alone are not safe durable identities because they can be reused.

**Fix:** save endpoint/workspace-scoped presentation preferences (enabled state,
placement, size and backing-tab identity); preserve an opaque tab identity in the
session snapshot; restore only a matching resource and replay its presentation
state after activation. Optional JSON identity metadata leaves the frozen
endpoint codecs unchanged. Creation/snapshot ordering and stale snapshot checks
avoid dropping a just-created area or restoring unrelated resources.

**Migration limit:** tabs created before these preferences existed are not
retroactively adopted based on their `Persistent` label. Old orphaned tabs may
remain ordinary tabs, especially after repeated manual setup. The implementation
intentionally does not guess which duplicate tab to adopt or merge live panes.
Configure a fresh area with this build, or explicitly reconcile older panes.
Client-local preferences are not automatically synchronized between machines.

## 3. Selection plus edge scrolling

**Symptom:** highlighting text while dragging to the edge to scroll failed only
inside persistent panes.

**Cause:** snapshot reconciliation compared selection ownership with the ordinary
server-focused pane, clearing persistent selection and its autoscroll timer.

**Fix:** reconcile selection, word gestures and copy-mode focus using effective
client-local focus validated against the incoming snapshot. Real focus loss
still clears the selection. This fix requires reopening the client, not stopping
the server.

## Verification

The final publication check completed successfully:

- `mise exec -- cargo nextest run --locked --bin herdr -j 3`: **3,389 passed,
  6 skipped**. This is the binary unit-test target, not every integration target.
- `mise exec -- cargo build --locked -j 3`: succeeded; three dead-code warnings
  remain (`snapshot_message`, `PersistentAreaSettings::percent`,
  `render_pane_surface`).
- Real-PTY selection checks: right and bottom placement both kept the highlight
  while a stationary held drag advanced through scrollback.
- Real-PTY server restart check: restored placement and the original two-pane
  backing tab without creating a duplicate.
- Earlier regression runs on this change also passed internal resizing in both
  directions, outer divider resizing, ordinary-pane resizing while docked,
  client-only restart, and the original tab-switch/input/hide-reopen smoke test.
- Six UI hot-path architecture checks passed during development.

A direct parallel `cargo test` run encountered process-global interference and
SIGPIPE; the isolated-process Nextest run above passed. The capability-list test
was also updated to include the already-advertised optional pane-dock capability;
no frozen wire fixture was changed.

Reproduce the live tests after building (requires `uv` or a Python environment
with `pyte`):

```sh
uv run --with pyte python scripts/check_persistent_selection.py
uv run --with pyte python scripts/check_persistent_regressions.py resize
uv run --with pyte python scripts/check_persistent_regressions.py resize-right
uv run --with pyte python scripts/check_persistent_regressions.py resize-main
uv run --with pyte python scripts/check_persistent_regressions.py client-restart
uv run --with pyte python scripts/check_persistent_regressions.py server-restart
```

These scripts use isolated temporary servers and state directories, not the
user's normal session. This is still an experimental fork: advanced clipboard,
multi-client geometry and old-session migration need further coverage. Transient
Kitty/client terminal-detach errors observed during desktop launching were not
fixed by these changes and are not covered by the isolated PTY tests.
