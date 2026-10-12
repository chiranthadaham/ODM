"""Compare two EXEs on repeatable HTTP/1.1 transfers with simulated connection latency."""
from pathlib import Path
import hashlib,http.server,json,os,socket,subprocess,sys,tempfile,threading,time
before,after=map(lambda p:Path(p).resolve(),sys.argv[1:3])
DATA=bytes(range(256))*131072
class Server(http.server.BaseHTTPRequestHandler):
    protocol_version='HTTP/1.1';connections=0;requests=0;lock=threading.Lock()
    def setup(self):
        super().setup()
        with self.lock:type(self).connections+=1
        time.sleep(.025)
    def log_message(self,*args):pass
    def handle(self):
        try:super().handle()
        except (ConnectionResetError,ConnectionAbortedError):pass
    def do_GET(self):
        with self.lock:type(self).requests+=1
        time.sleep(.015)
        a,b=0,len(DATA)-1;range=self.headers.get('Range','')
        if range:a,b=map(int,range[6:].split('-'))
        self.send_response(206 if range else 200);self.send_header('Content-Length',str(b-a+1));self.send_header('ETag','"benchmark-v1"')
        if range:self.send_header('Content-Range',f'bytes {a}-{b}/{len(DATA)}')
        self.end_headers()
        try:self.wfile.write(DATA[a:b+1])
        except OSError:pass
results=[]
for label,exe in [('before',before),('after',after)]:
    for repeat in range(3):
        Server.connections=Server.requests=0
        server=http.server.ThreadingHTTPServer(('127.0.0.1',0),Server);server.daemon_threads=True
        threading.Thread(target=server.serve_forever,daemon=True).start()
        try:
            with tempfile.TemporaryDirectory()as tmp:
                target=Path(tmp)/'sample.bin';started=time.monotonic()
                p=subprocess.run([str(exe),'--download',f'http://127.0.0.1:{server.server_port}/sample.bin',str(target),'--connections','8'],timeout=60,creationflags=subprocess.CREATE_NO_WINDOW,env=dict(os.environ,ODM_DATA_DIR=str(Path(tmp)/'state')))
                elapsed=time.monotonic()-started
                assert p.returncode==0,Path(str(target)+'.result.txt').read_text()
                assert hashlib.sha256(target.read_bytes()).digest()==hashlib.sha256(DATA).digest()
                results.append(dict(build=label,repeat=repeat+1,seconds=round(elapsed,3),bytes=len(DATA),requests=Server.requests,connections=Server.connections))
                print(json.dumps(results[-1]),flush=True)
        finally:server.shutdown();server.server_close()
if len(sys.argv)>3:Path(sys.argv[3]).write_text(json.dumps(dict(method='32 MiB HTTP/1.1 fixture, 25ms per connection setup and 15ms per request, three repeats each, full SHA256 integrity. Simulated latency; not an internet speed guarantee.',results=results),indent=2))
