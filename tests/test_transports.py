"""Real local HTTP authentication, HTTP proxy, SOCKS5 and FTP transport checks."""
import ctypes as c,base64,http.server,threading,tempfile,subprocess,sys,os,json,unittest,socketserver,socket,select,hashlib,re
from pathlib import Path
EXE=Path(sys.argv.pop(1)).resolve();DATA=bytes(range(251))*8192
class Blob(c.Structure):_fields_=[('size',c.c_ulong),('data',c.POINTER(c.c_ubyte))]
def protect(data):
    raw=(c.c_ubyte*len(data)).from_buffer_copy(data);src=Blob(len(data),raw);out=Blob()
    crypt=c.WinDLL('crypt32');kernel=c.WinDLL('kernel32')
    crypt.CryptProtectData.argtypes=[c.POINTER(Blob),c.c_wchar_p,c.c_void_p,c.c_void_p,c.c_void_p,c.c_ulong,c.POINTER(Blob)]
    kernel.LocalFree.argtypes=[c.c_void_p]
    assert crypt.CryptProtectData(c.byref(src),'ODM test',None,None,None,1,c.byref(out))
    result=c.string_at(out.data,out.size);kernel.LocalFree(out.data);return result
class HTTP(http.server.BaseHTTPRequestHandler):
    proxy=False
    observed_auth=[]
    def log_message(self,*args):pass
    def do_GET(self):
        type(self).observed_auth.append(self.headers.get('Authorization'))
        if self.path=='/basic' and self.headers.get('Authorization')!='Basic '+base64.b64encode(b'user:secret').decode():
            self.send_response(401);self.send_header('WWW-Authenticate','Basic realm="ODM test"');self.send_header('Content-Length','0');self.end_headers();return
        self.send_response(200);self.send_header('Content-Length',str(len(DATA)));self.send_header('Content-Type','application/octet-stream');self.end_headers()
        try:self.wfile.write(DATA)
        except OSError:pass
class Digest(HTTP):
    def do_GET(self):
        auth=self.headers.get('Authorization','');fields={m[0]:m[1]or m[2]for m in re.findall(r'(\w+)=(?:"([^"]*)"|([^,\s]+))',auth)}
        md5=lambda s:hashlib.md5(s.encode()).hexdigest()
        expected=md5(':'.join([md5('user:ODM test:secret'),'testnonce',fields.get('nc',''),fields.get('cnonce',''),'auth',md5('GET:'+self.path)]))
        if not auth.startswith('Digest ')or fields.get('response')!=expected:
            self.send_response(401);self.send_header('WWW-Authenticate','Digest realm="ODM test", nonce="testnonce", algorithm=MD5, qop="auth"');self.send_header('Content-Length','0');self.end_headers();return
        super().do_GET()
class ProxyAuth(HTTP):
    def do_GET(self):
        if self.headers.get('Proxy-Authorization')!='Basic '+base64.b64encode(b'user:secret').decode():
            self.send_response(407);self.send_header('Proxy-Authenticate','Basic realm="ODM proxy"');self.send_header('Content-Length','0');self.end_headers();return
        super().do_GET()
class Redirect(HTTP):
    target=0
    def do_GET(self):self.send_response(302);self.send_header('Location',f'http://127.0.0.1:{type(self).target}/basic');self.send_header('Content-Length','0');self.end_headers()
class Threads(socketserver.ThreadingTCPServer):allow_reuse_address=True;daemon_threads=True
def exact(s,n):
    data=b''
    while len(data)<n:
        more=s.recv(n-len(data))
        if not more:raise ConnectionError('Connection closed')
        data+=more
    return data
class SOCKS(socketserver.BaseRequestHandler):
    hits=0
    def handle(self):
        type(self).hits+=1
        s=self.request
        try:
            version,count=exact(s,2);methods=exact(s,count);s.sendall(b'\x05\x02')
            version,n=exact(s,2);user=exact(s,n);n=exact(s,1)[0];password=exact(s,n)
            if (user,password)!=(b'user',b'secret'):s.sendall(b'\x01\x01');return
            s.sendall(b'\x01\x00');version,command,_,kind=exact(s,4)
            host=socket.inet_ntoa(exact(s,4))if kind==1 else exact(s,exact(s,1)[0]).decode()
            port=int.from_bytes(exact(s,2),'big')
            with socket.create_connection((host,port),timeout=5) as target:
                s.sendall(b'\x05\x00\x00\x01\x7f\x00\x00\x01\x00\x00')
                while True:
                    readable,_,_=select.select([s,target],[],[],5)
                    if not readable:return
                    for src in readable:
                        data=src.recv(65536)
                        if not data:return
                        (target if src is s else s).sendall(data)
        except OSError:pass
class FTP(socketserver.StreamRequestHandler):
    def handle(self):
        passive=None;offset=0
        def reply(s):self.wfile.write((s+'\r\n').encode());self.wfile.flush()
        reply('220 ODM FTP test')
        try:
            while True:
                line=self.rfile.readline().decode().strip()
                if not line:break
                command,_,arg=line.partition(' ');command=command.upper()
                if command=='USER':reply('331 Password required')
                elif command=='PASS':reply('230 Logged in')
                elif command=='PWD':reply('257 "/"')
                elif command=='SYST':reply('215 UNIX Type: L8')
                elif command in ('TYPE','CWD','OPTS'):reply('200 OK')
                elif command=='SIZE':reply('213 '+str(len(DATA)))
                elif command=='MDTM':reply('213 20261011000000')
                elif command=='REST':offset=int(arg);reply('350 Restart accepted')
                elif command in ('EPSV','PASV'):
                    if passive:passive.close()
                    passive=socket.socket();passive.bind(('127.0.0.1',0));passive.listen(1);port=passive.getsockname()[1]
                    reply(f'229 Entering Extended Passive Mode (|||{port}|)'if command=='EPSV'else f'227 Entering Passive Mode (127,0,0,1,{port//256},{port%256})')
                elif command=='RETR':
                    reply('150 Opening data connection')
                    with passive.accept()[0] as stream:stream.sendall(DATA[offset:])
                    reply('226 Transfer complete')
                elif command=='QUIT':reply('221 Bye');break
                else:reply('502 Unsupported')
        except OSError:pass
        finally:
            if passive:passive.close()
class Tests(unittest.TestCase):
    def run_transfer(self,url,network=None,expected=0):
        with tempfile.TemporaryDirectory()as tmp:
            root=Path(tmp);state=root/'state';state.mkdir();target=root/'file.bin'
            if network:
                base=dict(proxy='',proxyUser='',proxyPassword='',serverOrigin='',serverUser='',serverPassword='');base.update(network)
                encrypted=protect(json.dumps(base).encode());(state/'network.dat').write_bytes(encrypted);self.assertNotIn(b'secret',encrypted)
            result=subprocess.run([str(EXE),'--download',url,str(target)],timeout=30,env=dict(os.environ,ODM_DATA_DIR=str(state)),creationflags=subprocess.CREATE_NO_WINDOW)
            report=Path(str(target)+'.result.txt').read_text(encoding='utf-8');self.assertEqual(result.returncode,expected,report)
            if expected==0:self.assertEqual(target.read_bytes(),DATA)
            else:self.assertFalse(target.exists())
    def serve(self,handler,fn,http=False):
        server=(http_server if http else Threads)(('127.0.0.1',0),handler)
        threading.Thread(target=server.serve_forever,daemon=True).start()
        try:fn(server.server_address[1])
        finally:server.shutdown();server.server_close()
    def test_http_basic_authentication(self):self.serve(HTTP,lambda p:self.run_transfer(f'http://127.0.0.1:{p}/basic',dict(serverOrigin=f'http://127.0.0.1:{p}',serverUser='user',serverPassword='secret')),True)
    def test_http_proxy(self):self.serve(HTTP,lambda p:self.run_transfer('http://odm.invalid/file.bin',dict(proxy=f'http://127.0.0.1:{p}')),True)
    def test_http_digest_authentication(self):self.serve(Digest,lambda p:self.run_transfer(f'http://127.0.0.1:{p}/digest',dict(serverOrigin=f'http://127.0.0.1:{p}',serverUser='user',serverPassword='secret')),True)
    def test_http_proxy_authentication(self):self.serve(ProxyAuth,lambda p:self.run_transfer('http://odm.invalid/file.bin',dict(proxy=f'http://127.0.0.1:{p}',proxyUser='user',proxyPassword='secret')),True)
    def test_credentials_are_not_sent_to_a_redirected_origin(self):
        HTTP.observed_auth=[]
        def run(target):
            Redirect.target=target
            self.serve(Redirect,lambda origin:self.run_transfer(f'http://127.0.0.1:{origin}/redirect',dict(serverOrigin=f'http://127.0.0.1:{origin}',serverUser='user',serverPassword='secret'),expected=1),True)
        self.serve(HTTP,run,True);self.assertTrue(HTTP.observed_auth);self.assertFalse(any(HTTP.observed_auth))
    def test_authenticated_socks5(self):
        SOCKS.hits=0
        self.serve(HTTP,lambda p:self.serve(SOCKS,lambda s:self.run_transfer(f'http://127.0.0.1:{p}/file.bin',dict(proxy=f'socks5h://127.0.0.1:{s}',proxyUser='user',proxyPassword='secret'))),True)
        self.assertGreater(SOCKS.hits,0)
    def test_ftp_download(self):self.serve(FTP,lambda p:self.run_transfer(f'ftp://127.0.0.1:{p}/file.bin'))
http_server=http.server.ThreadingHTTPServer
if __name__=='__main__':unittest.main()
