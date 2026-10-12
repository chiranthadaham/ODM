"""Real Windows app: checksum rejection, backup recovery, range resume and settings."""
import sys,runpy,os,json,tempfile,subprocess,threading,http.server,hashlib,time
from pathlib import Path
exe=Path(sys.argv[1]).resolve()
sys.argv=['helpers',str(exe)]
helper=runpy.run_path(str(Path(__file__).with_name('test_ui.py')))
u=helper['u'];wait=helper['wait'];click=helper['click'];field=helper['getfield'];setfield=helper['setfield'];popup=helper['popup'];send=helper['send']
assert not u.FindWindowW('OpenDownloadManagerRust',None),'Close ODM before running this isolated test.'
DATA=bytes(range(256))*16384
SHA=hashlib.sha256(DATA).hexdigest()
ranges=[]
class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self,*args):pass
    def do_GET(self):
        payload=DATA[::-1] if self.path.startswith('/changed') else DATA
        start=0;end=len(payload)-1;requested=self.headers.get('Range')
        if self.path.startswith('/changed') and self.headers.get('If-Range')=='"old"':requested=None
        if requested:
            ranges.append(requested)
            a,b=requested.removeprefix('bytes=').split('-');start=int(a);end=int(b) if b else end
            if start>=len(DATA):self.send_response(416);self.end_headers();return
            self.send_response(206)
            self.send_header('Content-Range',f'bytes {start}-{end}/{len(DATA)}')
        else:self.send_response(200)
        self.send_header('Content-Type','application/octet-stream')
        if not self.path.startswith('/novalidator'):self.send_header('ETag','"fixture-v1"')
        self.send_header('Content-Length',str(end-start+1));self.end_headers()
        try:self.wfile.write(payload[start:end+1])
        except (ConnectionResetError,BrokenPipeError):pass
server=http.server.ThreadingHTTPServer(('127.0.0.1',0),Handler)
threading.Thread(target=server.serve_forever,daemon=True).start()
url=f'http://127.0.0.1:{server.server_port}/sample.pdf'
with tempfile.TemporaryDirectory() as tmp:
    root=Path(tmp);env=dict(os.environ,ODM_DATA_DIR=str(root/'state'));state=root/'state';state.mkdir()
    for name,expected,code in [('good',SHA,0),('bad','0'*64,1)]:
        target=root/(name+'.bin')
        result=subprocess.run([str(exe),'--download',url,str(target),'--sha256',expected,'--connections','1'],env=env,timeout=60)
        assert result.returncode==code,(name,result.returncode)
        if code==0:assert target.read_bytes()==DATA
        else:
            assert not target.exists(),'Mismatched file was marked final'
            assert 'SHA-256 mismatch' in Path(str(target)+'.result.txt').read_text()
            assert Path(str(target)+'.part').exists() or (Path(str(target)+'.segments')/'assembled.part').exists(),'Partial data was lost'
    changed=root/'changed.bin';Path(str(changed)+'.part').write_bytes(DATA[:1024])
    changed_url=url.replace('/sample.pdf','/changed.pdf')
    result=subprocess.run([str(exe),'--download',changed_url,str(changed),'--etag','"old"','--sha256',hashlib.sha256(DATA[::-1]).hexdigest()],env=env,timeout=60)
    assert result.returncode==0 and changed.read_bytes()==DATA[::-1],'Changed server content was appended to old bytes'
    unvalidated=root/'unvalidated.bin';segments=Path(str(unvalidated)+'.segments');segments.mkdir()
    no_validator=url.replace('/sample.pdf','/novalidator.pdf')
    (segments/'metadata.json').write_text(json.dumps(dict(url=no_validator,validator='',total=len(DATA),count=2)))
    for i in range(2):(segments/f'{i}.part').write_bytes(b'X'*(len(DATA)//2))
    result=subprocess.run([str(exe),'--download',no_validator,str(unvalidated),'--sha256',SHA],env=env,timeout=60)
    assert result.returncode==0 and unvalidated.read_bytes()==DATA,'Unvalidated stale segments were reused'
    target=root/'resume.pdf';offset=1024*1024
    Path(str(target)+'.part').write_bytes(DATA[:offset])
    Path(str(target)+'.part.validator').write_text('"fixture-v1"')
    details=dict(url=url,path=str(target),error='',etag='"fixture-v1"',modified='',user_agent='',referer='',expected_sha256=SHA)
    queue=dict(format=1,connections=1,limit_kib=0,max_active=1,organize=True,hash_downloads=True,jobs=[dict(details=details,state=1,done=offset,total=len(DATA),added=1,id=123)])
    (state/'queue.backup.json').write_text(json.dumps(queue));(state/'queue.json').write_text('{truncated')
    app=subprocess.Popen([str(exe)],env=env)
    try:
        h=wait(lambda:u.FindWindowW('OpenDownloadManagerRust',None));table=wait(lambda:u.GetDlgItem(h,130))
        wait(lambda:send(table,0x1004)==1)
        assert (state/'recovery-notice.txt').exists()
        # Select the recovered row without relying on pixel coordinates.
        send(table,0x100,0x24) # Home selects the first row through the native keyboard handler.
        wait(lambda:send(table,0x100C,-1,2)==0)
        click(h,110)
        complete=wait(lambda:popup('Download complete'),45)
        assert target.read_bytes()==DATA
        assert f'bytes={offset}-' in ranges,ranges
        click(complete,2)
        click(h,174);dialog=wait(lambda:popup('Verify SHA-256'))
        send(dialog,0x111,385);time.sleep(.5);wait(lambda:field(dialog,384)=='SHA-256 verified',20)
        assert field(dialog,383)==SHA
        setfield(dialog,381,'0'*64);send(dialog,0x111,385);wait(lambda:'Mismatch' in field(dialog,384),20)
        click(dialog,2)
        click(h,117);settings=wait(lambda:popup('Settings'))
        assert send(u.GetDlgItem(settings,316),0xF0)==1
        assert send(u.GetDlgItem(settings,317),0xF0)==1
        click(settings,314);wait(lambda:not popup('Settings'))
        click(h,122);new=wait(lambda:popup('New download'))
        setfield(new,600,url);setfield(new,601,str(root/'organized'));click(new,604)
        start=wait(lambda:popup('New download — Save and Start'))
        assert Path(field(start,201))==root/'organized/Documents/sample.pdf',field(start,201)
        setfield(start,208,SHA);click(start,203)
        complete=wait(lambda:popup('Download complete'),45)
        assert (root/'organized/Documents/sample.pdf').read_bytes()==DATA
        click(complete,2);click(h,119);app.wait(timeout=20)
        saved=json.loads((state/'queue.json').read_text())
        assert saved['organize'] and saved['hash_downloads']
        assert any(j['details'].get('integrity_status')=='SHA-256 verified' for j in saved['jobs'])
        assert json.loads((state/'queue.backup.json').read_text())['format']==1
    finally:
        if app.poll() is None:
            h=u.FindWindowW('OpenDownloadManagerRust',None)
            if h:click(h,119)
            app.wait(timeout=20)
server.shutdown()
print('PASS: matching/mismatching downloads, changed-server restart, stale unvalidated segment rejection, truncated queue recovery, saved range resume, post-download verification, organized save prompt, settings persistence and valid backup.')
