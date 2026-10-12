"""A fresh WinHTTP response timeout must fall back without damaging resumable files."""
from pathlib import Path
import http.server,json,os,subprocess,sys,tempfile,threading,time,unittest
EXE=Path(sys.argv.pop(1)).resolve();DATA=b'timeout-fallback-test'*4000
class Server(http.server.BaseHTTPRequestHandler):
    requests=[]
    def log_message(self,*args):pass
    def handle(self):
        try:super().handle()
        except (ConnectionResetError,ConnectionAbortedError):pass
    def do_GET(self):
        type(self).requests.append(self.headers.get('Range',''))
        if len(type(self).requests)==1:time.sleep(32)
        self.send_response(200);self.send_header('Content-Length',str(len(DATA)));self.send_header('Content-Type','application/octet-stream');self.end_headers()
        try:self.wfile.write(DATA)
        except OSError:pass
class Tests(unittest.TestCase):
    def test_fresh_timeout_uses_alternate_transport(self):
        Server.requests=[];server=http.server.ThreadingHTTPServer(('127.0.0.1',0),Server);server.daemon_threads=True
        threading.Thread(target=server.serve_forever,daemon=True).start()
        try:
            with tempfile.TemporaryDirectory()as tmp:
                target=Path(tmp)/'file.bin';started=time.monotonic()
                p=subprocess.run([str(EXE),'--download',f'http://127.0.0.1:{server.server_port}/file.bin',str(target)],timeout=50,env=dict(os.environ,ODM_DATA_DIR=str(Path(tmp)/'state')),creationflags=subprocess.CREATE_NO_WINDOW)
                self.assertEqual(p.returncode,0,Path(str(target)+'.result.txt').read_text());self.assertEqual(target.read_bytes(),DATA)
                self.assertEqual(Server.requests,['bytes=0-0','']);self.assertLess(time.monotonic()-started,45)
        finally:server.shutdown();server.server_close()
if __name__=='__main__':unittest.main()
