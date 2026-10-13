"""Real Medium-to-High browser handoff regression; run from elevated Windows.

Uses an isolated queue. Close ODM before running; no existing queue is modified.
"""
import ctypes as c, json, os, struct, subprocess, sys, tempfile, threading, time, http.server
from ctypes import wintypes as w
from pathlib import Path
u=c.WinDLL('user32',use_last_error=True);a=c.WinDLL('advapi32',use_last_error=True);k=c.WinDLL('kernel32',use_last_error=True)
u.FindWindowW.argtypes=[w.LPCWSTR,w.LPCWSTR];u.FindWindowW.restype=w.HWND
u.PostMessageW.argtypes=[w.HWND,w.UINT,w.WPARAM,w.LPARAM]
u.GetDlgItem.argtypes=[w.HWND,c.c_int];u.GetDlgItem.restype=w.HWND
u.SendMessageW.argtypes=[w.HWND,w.UINT,w.WPARAM,w.LPARAM];u.SendMessageW.restype=w.LPARAM
k.GetCurrentProcess.restype=w.HANDLE
k.CloseHandle.argtypes=[w.HANDLE]
a.OpenProcessToken.argtypes=[w.HANDLE,w.DWORD,c.POINTER(w.HANDLE)]
a.DuplicateTokenEx.argtypes=[w.HANDLE,w.DWORD,c.c_void_p,c.c_int,c.c_int,c.POINTER(w.HANDLE)]
a.ConvertStringSidToSidW.argtypes=[w.LPCWSTR,c.POINTER(c.c_void_p)]
a.GetLengthSid.argtypes=[c.c_void_p];a.GetLengthSid.restype=w.DWORD
a.GetTokenInformation.argtypes=[w.HANDLE,c.c_int,c.c_void_p,w.DWORD,c.POINTER(w.DWORD)]
a.GetSidSubAuthorityCount.argtypes=[c.c_void_p];a.GetSidSubAuthorityCount.restype=c.POINTER(c.c_ubyte)
a.GetSidSubAuthority.argtypes=[c.c_void_p,w.DWORD];a.GetSidSubAuthority.restype=c.POINTER(w.DWORD)
class Label(c.Structure):_fields_=[('sid',c.c_void_p),('attributes',w.DWORD)]
class Startup(c.Structure):
    _fields_=[('cb',w.DWORD),('reserved',w.LPWSTR),('desktop',w.LPWSTR),('title',w.LPWSTR),('x',w.DWORD),('y',w.DWORD),('xs',w.DWORD),('ys',w.DWORD),('xc',w.DWORD),('yc',w.DWORD),('fill',w.DWORD),('flags',w.DWORD),('show',w.WORD),('reserved2',w.WORD),('data',c.c_void_p),('stdin',w.HANDLE),('stdout',w.HANDLE),('stderr',w.HANDLE)]
class Process(c.Structure):_fields_=[('process',w.HANDLE),('thread',w.HANDLE),('pid',w.DWORD),('tid',w.DWORD)]
a.SetTokenInformation.argtypes=[w.HANDLE,c.c_int,c.c_void_p,w.DWORD]
a.CreateProcessWithTokenW.argtypes=[w.HANDLE,w.DWORD,w.LPCWSTR,w.LPWSTR,w.DWORD,c.c_void_p,w.LPCWSTR,c.POINTER(Startup),c.POINTER(Process)]
def check(value):
    if not value:raise c.WinError(c.get_last_error())
def integrity():
    token=w.HANDLE();check(a.OpenProcessToken(k.GetCurrentProcess(),8,c.byref(token)))
    try:
        buf=c.create_string_buffer(512);size=w.DWORD();check(a.GetTokenInformation(token,25,buf,len(buf),c.byref(size)))
        label=c.cast(buf,c.POINTER(Label)).contents;count=a.GetSidSubAuthorityCount(label.sid)[0]
        return a.GetSidSubAuthority(label.sid,count-1)[0]
    finally:k.CloseHandle(token)
def medium(command):
    original=w.HANDLE();token=w.HANDLE();sid=c.c_void_p()
    check(a.OpenProcessToken(k.GetCurrentProcess(),11,c.byref(original)))
    try:
        check(a.DuplicateTokenEx(original,0xF01FF,None,2,1,c.byref(token)))
        check(a.ConvertStringSidToSidW('S-1-16-8192',c.byref(sid)))
        label=Label(sid,0x20);check(a.SetTokenInformation(token,25,c.byref(label),c.sizeof(label)+a.GetLengthSid(sid)))
        startup=Startup();startup.cb=c.sizeof(startup);proc=Process()
        command=c.create_unicode_buffer(subprocess.list2cmdline(command))
        check(a.CreateProcessWithTokenW(token,0,sys.executable,command,subprocess.CREATE_NO_WINDOW,None,None,c.byref(startup),c.byref(proc)))
        k.CloseHandle(proc.thread);k.CloseHandle(proc.process)
    finally:
        if token:k.CloseHandle(token)
        k.CloseHandle(original)
        if sid:k.LocalFree(c.cast(sid,w.HANDLE))
def wait(fn,seconds=15):
    end=time.monotonic()+seconds
    while time.monotonic()<end:
        value=fn()
        if value:return value
        time.sleep(.02)
    raise AssertionError('Timed out waiting for test condition')
if len(sys.argv)>1 and sys.argv[1]=='--client':
    exe,request,result,data=sys.argv[2:]
    try:
        p=subprocess.Popen([exe,'--native-host'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,creationflags=subprocess.CREATE_NO_WINDOW,env=dict(os.environ,ODM_DATA_DIR=data))
        raw=Path(request).read_bytes();p.stdin.write(struct.pack('<I',len(raw))+raw);p.stdin.flush()
        prefix=p.stdout.read(4);reply=json.loads(p.stdout.read(struct.unpack('<I',prefix)[0])) if len(prefix)==4 else {'ok':False,'error':'No native reply'}
        p.stdin.close();p.wait(timeout=5)
        buf=c.create_string_buffer(64);size=w.DWORD();k.QueryInformationJobObject(None,2,buf,64,c.byref(size));output={'integrity':integrity(),'job_flags':int.from_bytes(buf.raw[16:20],'little'),'reply':reply}
    except Exception as e:output={'error':str(e)}
    Path(result).write_text(json.dumps(output));sys.exit(0)

EXE=Path(sys.argv[1]).resolve();assert integrity()==12288,'Run elevated for the High-integrity server fixture'
assert not u.FindWindowW('OpenDownloadManagerRust',None),'Close ODM before this isolated test'
DATA=b'cross-integrity-download\0'*4096
class Server(http.server.BaseHTTPRequestHandler):
    def log_message(self,*args):pass
    def do_GET(self):
        self.send_response(200);self.send_header('Content-Length',str(len(DATA)));self.send_header('Content-Type','application/octet-stream');self.end_headers();self.wfile.write(DATA)
with tempfile.TemporaryDirectory() as tmp:
    root=Path(tmp);data=root/'state';data.mkdir();env=dict(os.environ,ODM_DATA_DIR=str(data))
    app=subprocess.Popen([str(EXE),'--background'],env=env)
    server=http.server.ThreadingHTTPServer(('127.0.0.1',0),Server);threading.Thread(target=server.serve_forever,daemon=True).start()
    try:
        h=wait(lambda:u.FindWindowW('OpenDownloadManagerRust',None))
        def launch(request,index):
            req=root/f'request-{index}.json';result=root/f'result-{index}.json';req.write_text(json.dumps(request))
            medium([sys.executable,str(Path(__file__).resolve()),'--client',str(EXE),str(req),str(result),str(data)])
            return result
        def reply(result):
            wait(lambda:result.exists());value=json.loads(result.read_text());assert value.get('integrity')==8192,value;return value['reply']
        assert reply(launch({'action':'ping-app'},0))['ok'];print('PASS: Medium helper connects to High app',flush=True)
        url=f'http://127.0.0.1:{server.server_port}/unknown.format'
        result=launch({'action':'download','url':url,'filename':'unknown.format','verifyAccess':True},1)
        prompt=wait(lambda:u.FindWindowW('ODMWorkflowRust','New download — Save and Start'))
        time.sleep(.3);u.PostMessageW(prompt,0x111,2,0)
        assert reply(result)['canceled'];print('PASS: Medium helper receives cancellation',flush=True)
        result=launch({'action':'download','url':url,'filename':'unknown.format','verifyAccess':False},2)
        prompt=wait(lambda:u.FindWindowW('ODMWorkflowRust','New download — Save and Start'))
        target=root/'download.bin';field=wait(lambda:u.GetDlgItem(prompt,201));text=c.create_unicode_buffer(str(target));check(u.SendMessageW(field,0xC,0,c.addressof(text)));u.PostMessageW(prompt,0x111,203,0)
        assert reply(result)['queued']
        try:wait(lambda:target.exists())
        except AssertionError:
            print('QUEUE DIAGNOSTIC:',(data/'queue.json').read_text(),flush=True);raise
        assert target.read_bytes()==DATA
        complete=wait(lambda:u.FindWindowW('ODMWorkflowRust','Download complete'));u.PostMessageW(complete,0x111,2,0)
        print('PASS: Medium-to-High confirmed download preserves bytes',flush=True)
        result=launch({'action':'download','url':'file:///C:/Windows/win.ini'},3)
        assert not reply(result)['ok'];print('PASS: invalid browser URL rejected',flush=True)
        foreign=root/'OtherODMCopy.exe';os.link(EXE,foreign)
        saved_exe=EXE;EXE=foreign
        try:assert not reply(launch({'action':'ping-app'},5))['ok']
        finally:EXE=saved_exe
        print('PASS: different executable path rejected',flush=True)
        u.PostMessageW(h,0x111,119,0);app.wait(timeout=15)
        coldfile=launch({'action':'ping-app'},4);cold=reply(coldfile);print('Cold-start process context:',coldfile.read_text(),flush=True);assert cold['ok'],cold;print('PASS: Medium helper cold-starts app',flush=True)
    finally:
        h=u.FindWindowW('OpenDownloadManagerRust',None)
        if h:u.PostMessageW(h,0x111,119,0);wait(lambda:not u.FindWindowW('OpenDownloadManagerRust',None))
        if app.poll() is None:app.wait(timeout=15)
        server.shutdown();server.server_close()
