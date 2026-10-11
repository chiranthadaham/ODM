"""Exercise real parallel range transfers, resume and defensive fallback."""
import http.server
from pathlib import Path
import subprocess,sys,tempfile,threading,time,unittest
EXE=Path(sys.argv.pop(1)).resolve()
DATA=bytes(range(251))*9000
class Server(http.server.BaseHTTPRequestHandler):
    lock=threading.Lock();active=0;peak=0;requests=[];version=1
    def log_message(self,*args):pass
    def do_GET(self):
        cls=type(self);range_header=self.headers.get('Range','');start=0;end=len(DATA)-1
        with cls.lock:cls.requests.append((self.path,range_header,self.headers.get('If-Range')))
        etag=f'"v{cls.version}"';payload=DATA if cls.version==1 else DATA[::-1]
        ranged=bool(range_header)
        if ranged:
            interval=range_header[6:].split('-');start=int(interval[0]);end=int(interval[1]) if interval[1] else len(payload)-1
        probe=range_header=='bytes=0-0'
        if self.path=='/ignore-after-probe' and not probe:ranged=False;start=0;end=len(payload)-1
        self.send_response(206 if ranged else 200);self.send_header('ETag',etag)
        self.send_header('Content-Length',str(end-start+1))
        if ranged:self.send_header('Content-Range',f'bytes {start}-{end+1 if self.path=="/bad-range" and not probe else end}/{len(payload)}')
        self.end_headers()
        if ranged and not probe:
            with cls.lock:cls.active+=1;cls.peak=max(cls.peak,cls.active)
        try:
            for at in range(start,end+1,16384):self.wfile.write(payload[at:min(at+16384,end+1)]);self.wfile.flush();time.sleep(.003)
        except (BrokenPipeError,ConnectionResetError,ConnectionAbortedError):pass
        finally:
            if ranged and not probe:
                with cls.lock:cls.active-=1
class Tests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.server=http.server.ThreadingHTTPServer(('127.0.0.1',0),Server);threading.Thread(target=cls.server.serve_forever,daemon=True).start();cls.url=f'http://127.0.0.1:{cls.server.server_port}'
    @classmethod
    def tearDownClass(cls):cls.server.shutdown();cls.server.server_close()
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.target=Path(self.temp.name)/'file.bin'
        with Server.lock:Server.requests=[];Server.peak=0;Server.version=1
    def tearDown(self):self.temp.cleanup()
    def download(self,route='/ranges',*options):
        p=subprocess.run([str(EXE),'--download',self.url+route,str(self.target),*options],timeout=30,creationflags=subprocess.CREATE_NO_WINDOW)
        return p.returncode,Path(str(self.target)+'.result.txt').read_text()
    def test_parallel_integrity(self):
        code,report=self.download();self.assertEqual(code,0,report);self.assertEqual(self.target.read_bytes(),DATA);self.assertGreaterEqual(Server.peak,2);self.assertIn('Connections: 8',report)
        self.assertFalse(Path(str(self.target)+'.segments').exists())
    def test_segmented_pause_resume(self):
        code,report=self.download('/ranges','--pause-after','65536');self.assertEqual(code,2,report)
        folder=Path(str(self.target)+'.segments');self.assertTrue(folder.exists());self.assertGreater(sum(p.stat().st_size for p in folder.glob('*.part')),0)
        with Server.lock:Server.requests=[]
        code,report=self.download();self.assertEqual(code,0,report);self.assertEqual(self.target.read_bytes(),DATA)
        # At least one resumed range starts beyond the original segment boundary.
        boundaries={len(DATA)*i//8 for i in range(8)}
        self.assertTrue(any(int(r[1][6:].split('-')[0]) not in boundaries for r in Server.requests if r[1] and r[1]!='bytes=0-0'))
    def test_ignored_ranges_fall_back(self):
        code,report=self.download('/ignore-after-probe');self.assertEqual(code,0,report);self.assertEqual(self.target.read_bytes(),DATA);self.assertIn('Connections: 1',report)
    def test_invalid_ranges_fall_back(self):
        code,report=self.download('/bad-range');self.assertEqual(code,0,report);self.assertEqual(self.target.read_bytes(),DATA)
    def test_changed_remote_restarts_segments(self):
        code,report=self.download('/ranges','--pause-after','65536');self.assertEqual(code,2,report)
        with Server.lock:Server.version=2
        code,report=self.download();self.assertEqual(code,0,report);self.assertEqual(self.target.read_bytes(),DATA[::-1])
    def test_single_connection_setting(self):
        code,report=self.download('/ranges','--connections','1');self.assertEqual(code,0,report);self.assertEqual(self.target.read_bytes(),DATA);self.assertIn('Connections: 1',report)
if __name__=='__main__':unittest.main()
