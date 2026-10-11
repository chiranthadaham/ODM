import unittest,http.server,threading,tempfile,subprocess,sys,time
from pathlib import Path
exe=Path(sys.argv.pop(1)).resolve();data=b'\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1'+bytes(range(256))*400
class Server(http.server.BaseHTTPRequestHandler):
 def log_message(self,*a):pass
 def do_GET(self):
  if self.path=='/redirect.doc':self.send_response(302);self.send_header('Location','/sample.doc');self.end_headers();return
  if self.path=='/forbidden.doc':code,kind,body=403,'text/html',b'Browser verification required'
  elif self.path=='/fake.doc':code,kind,body=200,'text/html',b'<html>Downloading...<script>location.href="/sample.doc"</script></html>'
  else:code,kind,body=200,'application/msword',data
  self.send_response(code);self.send_header('Content-Type',kind);self.send_header('Content-Length',str(len(body)));self.end_headers()
  try:self.wfile.write(body)
  except OSError:pass
class Tests(unittest.TestCase):
 @classmethod
 def setUpClass(cls):cls.server=http.server.ThreadingHTTPServer(('127.0.0.1',0),Server);threading.Thread(target=cls.server.serve_forever,daemon=True).start()
 @classmethod
 def tearDownClass(cls):cls.server.shutdown();cls.server.server_close()
 def download(self,route):
  self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup);self.path=Path(self.temp.name)/'sample.doc';r=subprocess.run([str(exe),'--download',f'http://127.0.0.1:{self.server.server_port}'+route,str(self.path)],timeout=15,creationflags=subprocess.CREATE_NO_WINDOW);self.report=Path(str(self.path)+'.result.txt').read_text();return r.returncode
 def test_small_doc(self):self.assertEqual(self.download('/sample.doc'),0,self.report);self.assertEqual(self.path.read_bytes(),data)
 def test_http_redirect(self):self.assertEqual(self.download('/redirect.doc'),0,self.report);self.assertEqual(self.path.read_bytes(),data)
 def test_forbidden_explained_without_retries(self):
  began=time.monotonic();self.assertEqual(self.download('/forbidden.doc'),1);self.assertIn('complete verification',self.report);self.assertLess(time.monotonic()-began,3);self.assertFalse(self.path.exists())
 def test_html_is_not_saved_as_doc(self):self.assertEqual(self.download('/fake.doc'),1);self.assertIn('web page instead',self.report);self.assertFalse(self.path.exists())
if __name__=='__main__':unittest.main()

