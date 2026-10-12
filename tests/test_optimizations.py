"""Exercise response reuse, rate-limit fallback and bounded mirror redirects."""
import http.server,threading,tempfile,subprocess,sys,time,unittest
from pathlib import Path
EXE=Path(sys.argv.pop(1)).resolve();DATA=bytes(range(251))*12000
class Server(http.server.BaseHTTPRequestHandler):
    requests=[];lock=threading.Lock()
    def log_message(self,*a):pass
    def do_GET(self):
        route=self.path;rg=self.headers.get('Range','')
        with self.lock:self.requests.append((route,rg))
        if (route.startswith('/probe-') and rg) or route=='/always-limited.exe':
            code=int(route.split('-')[1].split('.')[0]) if route.startswith('/probe-') else 429
            self.send_response(code)
            if code==429:self.send_header('Retry-After','1')
            self.send_header('Content-Length','0');self.end_headers();return
        if route=='/long-delay.exe':
            self.send_response(429);self.send_header('Retry-After','60');self.send_header('Content-Length','0');self.end_headers();return
        if route=='/publisher.exe':
            self.send_response(302);self.send_header('Location','/landing/file.exe');self.end_headers();return
        if route in ['/landing/file.exe','/loop.exe','/unsafe.exe']:
            target='loop.exe' if route=='/loop.exe' else 'file:///C:/Windows/win.ini' if route=='/unsafe.exe' else '../final.exe?one=1&amp;two=2'
            payload=f'<meta content="5; URL={target}" http-equiv="Refresh">'.encode()
            self.send_response(200);self.send_header('Content-Type','text/html');self.send_header('Content-Length',str(len(payload)));self.end_headers();self.wfile.write(payload);return
        if route=='/limited.exe' and rg and rg!='bytes=0-0':
            self.send_response(429);self.send_header('Content-Length','0');self.end_headers();return
        ranged=route=='/limited.exe' and bool(rg);start,end=0,len(DATA)-1
        if ranged:start,end=map(int,rg[6:].split('-'))
        self.send_response(206 if ranged else 200);self.send_header('Content-Type','application/octet-stream');self.send_header('ETag','"stable"');self.send_header('Content-Length',str(end-start+1))
        if ranged:self.send_header('Content-Range',f'bytes {start}-{end}/{len(DATA)}')
        self.end_headers()
        try:self.wfile.write(DATA[start:end+1])
        except OSError:pass
class Tests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):cls.server=http.server.ThreadingHTTPServer(('127.0.0.1',0),Server);threading.Thread(target=cls.server.serve_forever,daemon=True).start()
    @classmethod
    def tearDownClass(cls):cls.server.shutdown();cls.server.server_close()
    def setUp(self):self.temp=tempfile.TemporaryDirectory();self.target=Path(self.temp.name)/'file.exe';Server.requests=[]
    def tearDown(self):self.temp.cleanup()
    def download(self,route):
        p=subprocess.run([str(EXE),'--download',f'http://127.0.0.1:{self.server.server_port}'+route,str(self.target)],timeout=30,creationflags=subprocess.CREATE_NO_WINDOW)
        self.report=Path(str(self.target)+'.result.txt').read_text(encoding='utf-8');return p.returncode
    def test_full_probe_response_is_used_once(self):
        self.assertEqual(self.download('/full.exe'),0,self.report);self.assertEqual(self.target.read_bytes(),DATA);self.assertEqual(len(Server.requests),1)
    def test_single_connection_skips_range_probe(self):
        p=subprocess.run([str(EXE),'--download',f'http://127.0.0.1:{self.server.server_port}/single.unknown',str(self.target),'--connections','1'],timeout=30,creationflags=subprocess.CREATE_NO_WINDOW)
        self.assertEqual(p.returncode,0);self.assertEqual(self.target.read_bytes(),DATA)
        self.assertEqual(Server.requests,[('/single.unknown','')])
    def test_parallel_limit_falls_back_with_integrity(self):
        self.assertEqual(self.download('/limited.exe'),0,self.report);self.assertEqual(self.target.read_bytes(),DATA);self.assertIn('Connections: 1',self.report)
    def test_rejected_probe_uses_normal_get(self):
        for code in (403,405,416):
            with self.subTest(code=code):
                Server.requests=[]
                if self.target.exists():self.target.unlink()
                self.assertEqual(self.download(f'/probe-{code}.exe'),0,self.report)
                self.assertEqual(self.target.read_bytes(),DATA)
                self.assertEqual([r[1] for r in Server.requests],['bytes=0-0',''])
    def test_rate_limited_probe_respects_retry_after(self):
        began=time.monotonic();self.assertEqual(self.download('/probe-429.exe'),0,self.report)
        self.assertGreaterEqual(time.monotonic()-began,.95);self.assertEqual(self.target.read_bytes(),DATA)
        self.assertEqual([r[1] for r in Server.requests],['bytes=0-0',''])
    def test_long_retry_after_does_not_retry_early(self):
        self.assertEqual(self.download('/long-delay.exe'),1);self.assertEqual(len(Server.requests),1);self.assertFalse(self.target.exists())
    def test_actual_rate_limit_is_bounded(self):
        self.assertEqual(self.download('/always-limited.exe'),1);self.assertEqual(len(Server.requests),2);self.assertFalse(self.target.exists())
    def test_relative_refresh_uses_final_http_destination(self):
        self.assertEqual(self.download('/publisher.exe'),0,self.report);self.assertEqual(self.target.read_bytes(),DATA);self.assertIn(('/final.exe?one=1&two=2',''),Server.requests)
    def test_refresh_loop_is_bounded(self):
        self.assertEqual(self.download('/loop.exe'),1);self.assertLessEqual(len(Server.requests),5);self.assertFalse(self.target.exists())
    def test_refresh_cannot_open_local_files(self):
        self.assertEqual(self.download('/unsafe.exe'),1);self.assertFalse(self.target.exists());self.assertEqual(len(Server.requests),1)
if __name__=='__main__':unittest.main()
