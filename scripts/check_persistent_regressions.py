"""Real terminal regression checks, isolated from the user's sessions.

Build first with `mise exec -- cargo build --locked`.
Run with `uv run --with pyte python scripts/check_persistent_regressions.py MODE`.
Modes: resize, resize-right, resize-main, client-restart, server-restart.
Only isolated test clients/servers are terminated. Captures go to .local/.
"""
from persistent_smoke_support import Smoke, ROOT, BIN, SmokeScreen
import os, subprocess, time, codecs, sys
import pyte
(ROOT/'.local').mkdir(exist_ok=True)
s=Smoke()
mode=sys.argv[1] if len(sys.argv)>1 else 'resize'
def drag(x,y,dx,dy):
    s.send(f'\x1b[<0;{x+1};{y+1}M'.encode())
    s.send(f'\x1b[<32;{dx+1};{dy+1}M'.encode())
    s.send(f'\x1b[<0;{dx+1};{dy+1}m'.encode())
    s.pump(.5)
def stop_client():
    s.client.terminate(); s.client.wait(timeout=5)
    os.close(s.master);s.master=None;s.client=None
    s.screen=SmokeScreen(140,40);s.stream=pyte.Stream(s.screen)
    s.decoder=codecs.getincrementaldecoder('utf-8')('replace')
try:
    ws=s.cli('workspace','create','--cwd',str(s.base),'--label','REGRESSION','--focus')['result']
    wid=ws['workspace']['workspace_id']
    s.attach();s.wait_text('Persistent area +');s.click_text('Persistent area +');s.wait_text('rotate')
    tabs=s.cli('tab','list','--workspace',wid)['result']['tabs']
    dock=next(t['tab_id'] for t in tabs if t['label']=='Persistent')
    panes=s.cli('pane','list','--workspace',wid)['result']['panes']
    pane=next(p['pane_id'] for p in panes if p['tab_id']==dock)
    if mode in ('resize','resize-right','resize-main'):
        subject_tab=dock
        if mode=='resize-main':
            subject_tab=ws['tab']['tab_id']
            pane=ws['root_pane']['pane_id']
        direction='right' if mode=='resize-right' else 'down'
        result=s.cli('pane','split',pane,'--direction',direction,'--no-focus')['result']
        panes=s.cli('pane','list','--workspace',wid)['result']['panes']
        other=next(p['pane_id'] for p in panes if p['tab_id']==subject_tab and p['pane_id']!=pane)
        for p,label in [(pane,'P1LIVE'),(other,'P2LIVE')]:
            s.cli('pane','run',p,f"printf '\\033[2J\\033[H{label}\\n'")
        s.wait_text('P2LIVE');s.wait_text('P1LIVE')
        x,y=s.textpos('P2LIVE')
        axis=0 if direction=='right' else 1
        before=(x,y)[axis]
        print('before second pane marker',x,y,flush=True)
        lines=s.screen.display
        if direction=='down':
            candidates=[i for i in range(2,y) if '─' in lines[i][x:]]
            assert candidates, 'No split handle visible'
            divider=candidates[-1]
            drag(x+5,divider,x+5,divider+5)
        else:
            left,top=s.textpos('P1LIVE')
            candidates=[i for i in range(left+1,x) if lines[top+3][i]=='│']
            assert candidates, 'No horizontal split handle visible'
            divider=candidates[-1]
            drag(divider,top+3,divider+5,top+3)
        s.wait_text('P2LIVE');after=s.textpos('P2LIVE')[axis]
        print('after second pane marker',after,flush=True)
        assert after>=before+3, f'Persistent split failed to resize: before={before}, after={after}'
        print('PASS internal split drag:',mode,flush=True)
        if mode!='resize-main':
            # Main/persistent divider also changes usable width.
            x,y=s.textpos('P1LIVE');before=x
            divider=s.screen.display[5].index('│',30)
            drag(divider,5,divider-12,5)
            s.wait_text('P1LIVE');after=s.textpos('P1LIVE')[0]
            assert after<before-5, f'Outer divider did not resize: {before}->{after}'
            print('PASS outer persistent divider drag',flush=True)
    else:
        s.cli('pane','split',pane,'--direction','down','--no-focus')
        s.click_text('rotate');s.pump(.5)
        stop_client()
        if mode=='server-restart':
            s.cli('server','stop',raw=True);s.server.wait(timeout=10)
            s.server=subprocess.Popen([str(BIN),'server'],env=s.env,stdout=s.server_log,stderr=subprocess.STDOUT,start_new_session=True)
            deadline=time.monotonic()+10
            while time.monotonic()<deadline:
                if s.server.poll() is not None:raise AssertionError('Server exited')
                try:
                    tabs=s.cli('tab','list','--workspace',wid)['result']['tabs']
                    break
                except Exception:time.sleep(.1)
            else:raise AssertionError('Server restart failed')
        s.attach();s.wait_text('Persistent: on')
        tabs=s.cli('tab','list','--workspace',wid)['result']['tabs']
        assert len([t for t in tabs if t['label']=='Persistent'])==1, 'Duplicated backing tab'
        # Check correct tab is hidden, not merely renamed or recreated.
        assert next(t['tab_id'] for t in tabs if t['label']=='Persistent')==dock
        panes=s.cli('pane','list','--workspace',wid)['result']['panes']
        assert len([p for p in panes if p['tab_id']==dock])==2, 'Persistent split tree not restored'
        pane=next(p['pane_id'] for p in panes if p['tab_id']==dock)
        s.cli('pane','run',pane,"printf '\\033[2J\\033[HRESTORED-MARKER\\n'")
        s.wait_text('RESTORED-MARKER')
        assert s.textpos('RESTORED-MARKER')[1]>10,'Bottom placement not restored'
        print('PASS',mode,'restored persistent area, placement and original backing tab',flush=True)
    (ROOT/f'.local/regression-{mode}.txt').write_text(s.pump())
finally:
    (ROOT/f'.local/regression-{mode}-last.txt').write_text('\n'.join(s.screen.display))
    s.close()
