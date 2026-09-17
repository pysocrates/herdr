"""Isolated real-PTY smoke driver for the fork (never touches normal sessions)."""
import os, pty, fcntl, termios, struct, subprocess, tempfile, select, time, json, codecs, pathlib, signal
import pyte

ROOT = pathlib.Path(__file__).resolve().parents[1]
BIN = ROOT / 'target/debug/herdr'

class SmokeScreen(pyte.Screen):
    def report_device_status(self, mode, **kwargs):
        if not kwargs.get('private'):
            super().report_device_status(mode)

class Smoke:
    def __init__(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='herdr-persistent-smoke-')
        self.base = pathlib.Path(self.tmp.name)
        self.env = {k:v for k,v in os.environ.items() if not k.startswith('HERDR_')}
        self.env.update(XDG_CONFIG_HOME=str(self.base/'config'), XDG_DATA_HOME=str(self.base/'data'), XDG_CACHE_HOME=str(self.base/'cache'), XDG_STATE_HOME=str(self.base/'state'), XDG_RUNTIME_DIR=str(self.base/'runtime'), HERDR_SOCKET_PATH=str(self.base/'api.sock'), SHELL='/bin/sh', TERM='xterm-256color', HERDR_DISABLE_SOUND='1')
        (self.base/'runtime').mkdir()
        for name in ('herdr', 'herdr-dev'):
            d = self.base/'config'/name
            d.mkdir(parents=True)
            (d/'config.toml').write_text('onboarding = false\n')
        self.screen = SmokeScreen(140, 40)
        self.stream = pyte.Stream(self.screen)
        self.decoder = codecs.getincrementaldecoder('utf-8')('replace')
        self.server_log = open(self.base/'server.log', 'wb')
        self.server = subprocess.Popen([str(BIN),'server'],env=self.env,stdout=self.server_log,stderr=subprocess.STDOUT,start_new_session=True)
        self.client = None
        self.master = None
        for _ in range(100):
            if (self.base/'api.sock').exists(): break
            if self.server.poll() is not None: raise RuntimeError((self.base/'server.log').read_text())
            time.sleep(.05)
        else: raise RuntimeError('Server did not create socket')
    def cli(self,*args,raw=False):
        result=subprocess.run([str(BIN),*args],env=self.env,capture_output=True,text=True,timeout=15)
        if result.returncode: raise RuntimeError(f'{args}: {result.stderr} {result.stdout}')
        return result.stdout if raw or not result.stdout.strip() else json.loads(result.stdout)
    def attach(self):
        self.master, slave = pty.openpty()
        fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',40,140,0,0))
        self.client = subprocess.Popen([str(BIN),'client'],stdin=slave,stdout=slave,stderr=slave,env=self.env,start_new_session=True)
        os.close(slave)
        self.pump(1)
    def pump(self, seconds=.3):
        end=time.monotonic()+seconds
        while time.monotonic()<end:
            readable,_,_=select.select([self.master],[],[],max(0,end-time.monotonic()))
            if readable:
                try: data=os.read(self.master,65536)
                except OSError: break
                if not data: break
                self.stream.feed(self.decoder.decode(data))
        return '\n'.join(self.screen.display)
    def send(self,data):
        os.write(self.master,data)
        return self.pump()
    def click(self,x,y,button=0):
        return self.send(f'\x1b[<{button};{x+1};{y+1}M\x1b[<{button};{x+1};{y+1}m'.encode())
    def textpos(self,text):
        for y,row in enumerate(self.screen.display):
            if text in row: return row.index(text),y
        raise AssertionError(f'{text!r} absent:\n'+self.pump(0))
    def click_text(self,text,button=0):
        x,y=self.textpos(text)
        return self.click(x+1,y,button)
    def wait_text(self,text,timeout=5):
        end=time.monotonic()+timeout
        while time.monotonic()<end:
            if text in self.pump(.1): return
        raise AssertionError(f'{text!r} absent:\n'+self.pump(0))
    def close(self):
        if self.client is not None:
            self.client.terminate()
            try:self.client.wait(timeout=3)
            except subprocess.TimeoutExpired:self.client.kill(); self.client.wait()
        if self.master is not None: os.close(self.master)
        try:self.cli('server','stop',raw=True)
        except Exception: self.server.terminate()
        try:self.server.wait(timeout=3)
        except subprocess.TimeoutExpired:self.server.kill();self.server.wait()
        self.server_log.close()
        self.tmp.cleanup()

if __name__ == '__main__':
    s=Smoke()
    try:
        result=s.cli('workspace','create','--cwd',str(s.base),'--label','SMOKE','--focus')['result']
        print(json.dumps(result,indent=2))
        pane=result['root_pane']['pane_id']
        s.cli('pane','rename',pane,'PERSISTENT-SHELL')
        s.attach()
        print(s.pump(1))
    finally:s.close()
