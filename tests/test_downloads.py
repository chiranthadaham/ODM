"""Integration tests for the compiled C++ executable; Python is only a test tool."""
import http.server
from pathlib import Path
import subprocess
import sys
import tempfile
import threading
import unittest

EXE=Path(sys.argv.pop(1)).resolve() if len(sys.argv)>1 else Path('OpenDownloadManager-Cpp.exe').resolve()
DATA=bytes(range(256))*4096
class Server(http.server.BaseHTTPRequestHandler):
    def log_message(self,*args):pass
    def do_GET(self):
        offset=0
        if self.path!='/ignore' and self.headers.get('Range') and self.headers.get('If-Range')=='"v1"':
            offset=int(self.headers['Range'][6:].split('-')[0])
        self.send_response(206 if offset else 200)
        self.send_header('ETag','"v1"')
        self.send_header('Content-Length',str(len(DATA)-offset))
        if offset:self.send_header('Content-Range',f'bytes {offset}-{len(DATA)-1}/{len(DATA)}')
        self.end_headers()
        try:self.wfile.write(DATA[offset:])
        except (BrokenPipeError,ConnectionResetError,ConnectionAbortedError):pass
class Tests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.server=http.server.ThreadingHTTPServer(('127.0.0.1',0),Server)
        threading.Thread(target=cls.server.serve_forever,daemon=True).start()
        cls.url=f'http://127.0.0.1:{cls.server.server_port}'
    @classmethod
    def tearDownClass(cls):cls.server.shutdown();cls.server.server_close()
    def setUp(self):self.tmp=tempfile.TemporaryDirectory();self.target=Path(self.tmp.name)/'file.bin'
    def tearDown(self):self.tmp.cleanup()
    def run_download(self,route='/file',*args):
        result=subprocess.run([str(EXE),'--download',self.url+route,str(self.target),*args],timeout=25,creationflags=subprocess.CREATE_NO_WINDOW)
        report=Path(str(self.target)+'.result.txt').read_text()
        return result.returncode,report
    def test_full_download(self):
        code,report=self.run_download();self.assertEqual(code,0,report);self.assertEqual(self.target.read_bytes(),DATA)
    def test_pause_resume(self):
        code,report=self.run_download('/file','--pause-after','65536');self.assertEqual(code,2,report)
        part=Path(str(self.target)+'.part');self.assertTrue(part.exists());self.assertLess(part.stat().st_size,len(DATA))
        code,report=self.run_download('/file','--etag','"v1"');self.assertEqual(code,0,report);self.assertEqual(self.target.read_bytes(),DATA)
    def test_ignored_range(self):
        Path(str(self.target)+'.part').write_bytes(DATA[:20000])
        code,report=self.run_download('/ignore','--etag','"v1"');self.assertEqual(code,0,report);self.assertEqual(self.target.read_bytes(),DATA)
    def test_missing_validator(self):
        Path(str(self.target)+'.part').write_bytes(b'incorrect content')
        code,report=self.run_download();self.assertEqual(code,0,report);self.assertEqual(self.target.read_bytes(),DATA)
    def test_destination_preserved(self):
        self.target.write_bytes(b'preserve me')
        code,report=self.run_download();self.assertEqual(code,0,report);self.assertEqual(self.target.read_bytes(),b'preserve me')
        self.assertEqual(self.target.with_name('file (1).bin').read_bytes(),DATA)
    def test_invalid_protocol(self):
        result=subprocess.run([str(EXE),'--download','file:///C:/Windows/win.ini',str(self.target)],timeout=15,creationflags=subprocess.CREATE_NO_WINDOW)
        self.assertEqual(result.returncode,1);self.assertFalse(self.target.exists())
if __name__=='__main__':unittest.main()
