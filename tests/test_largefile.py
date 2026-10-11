"""Check large segmented assembly, stale assembly files, and destination collisions."""
import http.server,threading,tempfile,subprocess,sys,json,hashlib,unittest
from pathlib import Path
EXE=Path(sys.argv.pop(1)).resolve();DATA=bytes(range(256))*262144
class Server(http.server.BaseHTTPRequestHandler):
    def log_message(self,*args):pass
    def do_GET(self):
        interval=self.headers.get('Range','bytes=0-').removeprefix('bytes=');a,b=interval.split('-');a=int(a);b=int(b)if b else len(DATA)-1
        self.send_response(206);self.send_header('Content-Range',f'bytes {a}-{b}/{len(DATA)}');self.send_header('Content-Length',str(b-a+1));self.send_header('ETag','"large-v1"');self.end_headers()
        try:self.wfile.write(DATA[a:b+1])
        except OSError:pass
class HTTP(http.server.ThreadingHTTPServer):request_queue_size=64
class Tests(unittest.TestCase):
    def test_large_assembly_never_overwrites_existing_file(self):
        server=HTTP(('127.0.0.1',0),Server);threading.Thread(target=server.serve_forever,daemon=True).start()
        try:
            with tempfile.TemporaryDirectory()as tmp:
                target=Path(tmp)/'résumé file.bin';target.write_bytes(b'keep original');url=f'http://127.0.0.1:{server.server_port}/large.bin'
                folder=Path(str(target)+'.segments');folder.mkdir();(folder/'metadata.json').write_text(json.dumps(dict(url=url,validator='"large-v1"',total=len(DATA),count=128)))
                (folder/'assembled.part').write_bytes(b'stale assembly data')
                result=subprocess.run([str(EXE),'--download',url,str(target)],timeout=60,creationflags=subprocess.CREATE_NO_WINDOW)
                report=Path(str(target)+'.result.txt').read_text(encoding='utf-8');self.assertEqual(result.returncode,0,report)
                self.assertEqual(target.read_bytes(),b'keep original');final=target.with_name('résumé file (1).bin');self.assertEqual(final.stat().st_size,len(DATA))
                self.assertEqual(hashlib.sha256(final.read_bytes()).digest(),hashlib.sha256(DATA).digest());self.assertFalse(folder.exists())
        finally:server.shutdown();server.server_close()
if __name__=='__main__':unittest.main()
