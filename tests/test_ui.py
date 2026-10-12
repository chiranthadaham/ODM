"""Exercise the actual native Rust UI with an isolated queue and local server."""
import ctypes as c
from ctypes import wintypes as w
import http.server, threading, tempfile, subprocess, sys, time, os, json, struct, unittest
from pathlib import Path
EXE=Path(sys.argv.pop(1)).resolve()
u=c.WinDLL('user32',use_last_error=True)
u.FindWindowW.argtypes=[w.LPCWSTR,w.LPCWSTR];u.FindWindowW.restype=w.HWND
u.GetDlgItem.argtypes=[w.HWND,c.c_int];u.GetDlgItem.restype=w.HWND
u.GetClientRect.argtypes=[w.HWND,c.POINTER(w.RECT)];u.GetClientRect.restype=w.BOOL
u.SendMessageW.argtypes=[w.HWND,w.UINT,w.WPARAM,w.LPARAM];u.SendMessageW.restype=w.LPARAM
u.PostMessageW.argtypes=[w.HWND,w.UINT,w.WPARAM,w.LPARAM]
u.SetWindowTextW.argtypes=[w.HWND,w.LPCWSTR]
u.GetWindowTextW.argtypes=[w.HWND,w.LPWSTR,c.c_int]
u.IsWindowVisible.argtypes=[w.HWND];u.IsWindowVisible.restype=w.BOOL
u.SendMessageTimeoutW.argtypes=[w.HWND,w.UINT,w.WPARAM,w.LPARAM,w.UINT,w.UINT,c.POINTER(c.c_size_t)]
def send(h,msg,wp=0,lp=0):
    result=c.c_size_t()
    assert u.SendMessageTimeoutW(h,msg,wp,lp,2,3000,c.byref(result)),('Window message timed out',hex(msg))
    return result.value
def wait(fn,seconds=15):
    end=time.monotonic()+seconds
    while time.monotonic()<end:
        v=fn()
        if v:return v
        time.sleep(.05)
    names=[]
    @c.WINFUNCTYPE(w.BOOL,w.HWND,w.LPARAM)
    def enum(h,_):
        b=c.create_unicode_buffer(2048);u.GetWindowTextW(h,b,len(b))
        cls=c.create_unicode_buffer(100);u.GetClassNameW(h,cls,len(cls))
        if cls.value in ('ODMWorkflowRust','OpenDownloadManagerRust','#32770'):
            names.append((cls.value,b.value))
            @c.WINFUNCTYPE(w.BOOL,w.HWND,w.LPARAM)
            def child(h,_):
                b=c.create_unicode_buffer(2048);u.GetWindowTextW(h,b,len(b))
                if b.value:names.append(b.value)
                return True
            u.EnumChildWindows(h,child,0)
        return True
    u.EnumWindows(enum,0)
    raise AssertionError('Timed out waiting for UI state; windows: '+repr(names))
def popup(title):return u.FindWindowW('ODMWorkflowRust',title)
def click(h,id):
    u.PostMessageW(h,0x111,id,0)
def setfield(h,id,s):
    control=wait(lambda:u.GetDlgItem(h,id))
    b=c.create_unicode_buffer(s)
    assert u.SendMessageW(control,0xC,0,c.addressof(b)),c.get_last_error()
    assert getfield(h,id)==s,(id,getfield(h,id),s)
def getfield(h,id):
    b=c.create_unicode_buffer(32768);u.SendMessageW(u.GetDlgItem(h,id),0xD,len(b),c.addressof(b));return b.value
class Server(http.server.BaseHTTPRequestHandler):
    def log_message(self,*args):pass
    def do_GET(self):
        data=b'rust-download-test\0'*65536
        self.send_response(200);self.send_header('Content-Length',str(len(data)));self.send_header('ETag','"ui"');self.end_headers()
        try:
            for n in range(0,len(data),65536):self.wfile.write(data[n:n+65536]);self.wfile.flush();time.sleep(.025)
        except OSError:pass
class Tests(unittest.TestCase):
    def test_native_interface_and_browser_handoff(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);env=dict(os.environ,ODM_DATA_DIR=str(root/'state'))
            server=http.server.ThreadingHTTPServer(('127.0.0.1',0),Server)
            threading.Thread(target=server.serve_forever,daemon=True).start()
            url=f'http://127.0.0.1:{server.server_port}/sample.bin'
            app=subprocess.Popen([str(EXE)],env=env)
            try:
                h=wait(lambda:u.FindWindowW('OpenDownloadManagerRust',None))
                self.assertFalse(u.GetDlgItem(h,123), 'Organize button must be removed')
                wait(lambda:u.GetDlgItem(h,124))
                self.assertEqual(getfield(h,124),'Browser plug-in')
                subprocess.run([str(EXE),'--background','--browser-plugins'],env=env,timeout=5)
                plugin=wait(lambda:popup('Browser plug-in'))
                self.assertEqual(getfield(plugin,701),'Chrome')
                self.assertEqual(getfield(plugin,702),'Firefox')
                click(plugin,2);wait(lambda:not popup('Browser plug-in'))
                table=wait(lambda:u.GetDlgItem(h,130))
                def columns_fit():
                    bounds=w.RECT()
                    if not u.GetClientRect(table,c.byref(bounds)):return False
                    return sum(send(table,0x101D,i) for i in range(8))<=bounds.right
                wait(columns_fit)
                click(h,122);new=wait(lambda:popup('New download'))
                setfield(new,600,url);setfield(new,601,str(root));click(new,604)
                start=wait(lambda:popup('New download — Save and Start'))
                self.assertEqual(getfield(start,204),url)
                setfield(start,201,str(root/'sample.bin'));click(start,203)
                progress=wait(lambda:popup('Download progress'))
                complete=wait(lambda:popup('Download complete'))
                self.assertEqual((root/'sample.bin').read_bytes(),b'rust-download-test\0'*65536)
                click(complete,2);wait(lambda:not popup('Download complete'))
                click(h,117);settings=wait(lambda:popup('Settings'))
                setfield(settings,310,'3');setfield(settings,311,'4');setfield(settings,312,'256');click(settings,314)
                wait(lambda:not popup('Settings'))
                click(h,117);settings=wait(lambda:popup('Settings'))
                self.assertEqual([getfield(settings,id)for id in (310,311,312)],['3','4','256']);click(settings,2)
                host=subprocess.Popen([str(EXE),'--native-host'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,env=env)
                request=json.dumps({'action':'download','url':url,'filename':'browser.bin','verifyAccess':True}).encode()
                host.stdin.write(struct.pack('<I',len(request))+request);host.stdin.flush()
                start=wait(lambda:popup('New download — Save and Start'));click(start,2)
                length=struct.unpack('<I',host.stdout.read(4))[0];reply=json.loads(host.stdout.read(length));self.assertTrue(reply['canceled'])
                host.stdin.close();host.wait(timeout=5);host.stdout.close()
                u.PostMessageW(h,0x10,0,0);wait(lambda:not u.IsWindowVisible(h))
                host=subprocess.Popen([str(EXE),'--native-host'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,env=env)
                request=json.dumps({'action':'download','url':url,'filename':'browser.bin','verifyAccess':True}).encode()
                host.stdin.write(struct.pack('<I',len(request))+request);host.stdin.flush()
                start=wait(lambda:popup('New download — Save and Start'));wait(lambda:u.GetDlgItem(start,203));click(start,203)
                length=struct.unpack('<I',host.stdout.read(4))[0];reply=json.loads(host.stdout.read(length));self.assertTrue(reply['ok'])
                host.stdin.close();host.wait(timeout=5);host.stdout.close()
                self.assertFalse(u.IsWindowVisible(h))
                progress=wait(lambda:popup('Download progress'));click(progress,504)
                wait(lambda:getfield(progress,504)=='Resume');click(progress,504)
                complete=wait(lambda:popup('Download complete'),25);self.assertEqual((root/'browser.bin').read_bytes(),b'rust-download-test\0'*65536);click(complete,2)
                subprocess.run([str(EXE)],env=env,timeout=5);wait(lambda:u.IsWindowVisible(h))
                table=u.GetDlgItem(h,130);header=send(table,0x101F)
                u.PostMessageW(header,0x201,1,50|(15<<16));u.PostMessageW(header,0x202,0,50|(15<<16));time.sleep(.3)
                u.PostMessageW(table,0x201,1,50|(44<<16));u.PostMessageW(table,0x202,0,50|(44<<16));time.sleep(.3);click(h,116)
                def removed_one():
                    try:
                        q=json.loads((root/'state/queue.json').read_text())
                        return len(q['jobs'])==1 and q['jobs'][0]['details']['path'].endswith('sample.bin')
                    except (OSError,ValueError):return False
                wait(removed_one);click(h,121)
                click(h,119);app.wait(timeout=15)
                queue=json.loads((root/'state/queue.json').read_text());self.assertEqual(queue['max_active'],3);self.assertEqual(len(queue['jobs']),0)
                self.assertTrue((root/'sample.bin').is_file());self.assertTrue((root/'browser.bin').is_file())
            finally:
                if app.poll() is None:app.terminate();app.wait(timeout=5)
                server.shutdown();server.server_close()
if __name__=='__main__':unittest.main()
