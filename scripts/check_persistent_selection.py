"""Live selection edge-scroll regression in isolated PTYs.
Run after building: uv run --with pyte python scripts/check_persistent_selection.py
"""
import os
import re
from persistent_smoke_support import Smoke, ROOT

for placement in ('right', 'bottom'):
    s = Smoke()
    try:
        def send(data, seconds):
            os.write(s.master, data)
            return s.pump(seconds)
        ws = s.cli('workspace', 'create', '--cwd', str(ROOT), '--label', 'SELECTION')['result']
        wid = ws['workspace']['workspace_id']
        s.attach()
        s.click_text('Persistent area +')
        s.wait_text('Persistent: on')
        tabs = s.cli('tab', 'list', '--workspace', wid)['result']['tabs']
        dock = next(t['tab_id'] for t in tabs if t['label'] == 'Persistent')
        panes = s.cli('pane', 'list', '--workspace', wid)['result']['panes']
        pane = next(p['pane_id'] for p in panes if p['tab_id'] == dock)
        if placement == 'bottom':
            s.click_text('rotate')
        s.cli('pane', 'run', pane, "i=1; while [ $i -le 180 ]; do printf 'ROW-%03d selection text\\n' $i; i=$((i+1)); done")
        s.wait_text('ROW-180')
        x, y = s.textpos('ROW-180')
        # Move well into scrollback before dragging towards the bottom edge.
        for _ in range(20):
            send(f'\x1b[<64;{x+5};{y+1}M'.encode(), .04)
        s.pump(.3)
        def visible_rows():
            return [int(m.group(1)) for line in s.screen.display
                    for m in re.finditer(r'ROW-(\d+)', line[x:])]
        before = visible_rows()
        assert before and max(before) < 180, ('wheel did not enter scrollback', before)
        col = x + 4
        row = 35
        normal = s.screen.buffer[38][col]
        normal_style = (normal.bg, normal.fg, normal.reverse)
        send(f'\x1b[<0;{col+1};{row+1}M'.encode(), .05)
        send(f'\x1b[<32;{col+1};40M'.encode(), .15)
        s.pump(1.1)
        after = visible_rows()
        assert after and max(after) > max(before) + 5, ('held drag did not scroll', before, after)
        selected = s.screen.buffer[38][col]
        assert (selected.bg, selected.fg, selected.reverse) != normal_style, 'selection highlight was lost'
        # Keep holding with no further mouse motion; the timer must continue.
        s.pump(.4)
        later = visible_rows()
        assert max(later) > max(after), ('autoscroll stopped during held selection', after, later)
        send(f'\x1b[<0;{col+1};40m'.encode(), .1)
        print(f'PASS {placement}: held drag scrolls {max(before)} -> {max(after)} -> {max(later)} with highlight retained', flush=True)
    finally:
        s.close()
