"""Verify an interrupted HTTP connection resumes automatically with a validator."""
from pathlib import Path
import http.server,threading,tempfile,subprocess,sys,socket,unittest
EXE=Path(sys.argv.pop(1)).resolve();DATA=bytes(range(256))*4096
class Server(http.server.BaseHTTPRequestHandler):
    requests=[]
    def log_message(self,*args):pass
    def do_GET(self):
        type(self).requests.append(self.headers.get('Range',''))
        first=len(type(self).requests)==1;start=0 if first else int(self.headers['Range'][6:].split('-')[0])
        self.send_response(200 if first else 206);self.send_header('ETag','"recover-v1"');self.send_header('Content-Length',str(len(DATA)-start))
        if not first:self.send_header('Content-Range',f'bytes {start}-{len(DATA)-1}/{len(DATA)}')
        self.end_headers()
        if first:
            self.wfile.write(DATA[:65536]);self.wfile.flush();self.connection.shutdown(socket.SHUT_RDWR);self.connection.close();self.close_connection=True
        else:self.wfile.write(DATA[start:])
class Tests(unittest.TestCase):
    def test_recovers_and_resumes_partial_data(self):
        Server.requests=[];server=http.server.ThreadingHTTPServer(('127.0.0.1',0),Server);threading.Thread(target=server.serve_forever,daemon=True).start()
        try:
            with tempfile.TemporaryDirectory()as tmp:
                target=Path(tmp)/'file.bin';result=subprocess.run([str(EXE),'--download',f'http://127.0.0.1:{server.server_port}/file.bin',str(target)],timeout=20,creationflags=subprocess.CREATE_NO_WINDOW)
                report=Path(str(target)+'.result.txt').read_text(encoding='utf-8');self.assertEqual(result.returncode,0,report);self.assertEqual(target.read_bytes(),DATA)
                self.assertEqual(Server.requests,['bytes=0-0','bytes=65536-']);self.assertEqual(report.splitlines()[1],'')
        finally:server.shutdown();server.server_close()
if __name__=='__main__':unittest.main()
