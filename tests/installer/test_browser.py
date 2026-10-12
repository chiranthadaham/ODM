import shutil
import ctypes as c,os,sys,runpy,subprocess,threading,tempfile,time,json,http.server,hashlib,zipfile,urllib.request
from pathlib import Path
root=Path(__file__).resolve().parents[2];exe=Path(os.environ['LOCALAPPDATA'])/'Programs/Open Download Manager/OpenDownloadManager.exe';sys.argv=['test',str(exe)];helper=runpy.run_path(str(root/'tests/test_ui.py'));u=helper['u'];stop=threading.Event();reports=[]
data=b'ODM browser store integration\0'*32768
class Server(http.server.BaseHTTPRequestHandler):
 def log_message(self,*a):pass
 def do_POST(self):
  reports.append(json.loads(self.rfile.read(int(self.headers['Content-Length']))));self.send_response(200);self.send_header('Access-Control-Allow-Origin','*');self.end_headers()
 def do_GET(self):
  if self.path=='/page':
   body=b'<html><body>'+b''.join(f'<a href="/{n}">{n}</a><br>'.encode() for n in ['chrome.zip','chrome.pdf','chrome.future'])+b'</body></html>';self.send_response(200);self.send_header('Content-Type','text/html');self.send_header('Content-Length',str(len(body)));self.end_headers();self.wfile.write(body);return
  a,b=0,len(data)-1;r=self.headers.get('Range')
  if r:
   x,y=r[6:].split('-');a=int(x);b=int(y) if y else b
  self.send_response(206 if r else 200);self.send_header('Content-Type','application/octet-stream');self.send_header('Content-Disposition',f'attachment; filename="{self.path[1:]}"');self.send_header('Content-Length',str(b-a+1));self.send_header('Accept-Ranges','bytes')
  if r:self.send_header('Content-Range',f'bytes {a}-{b}/{len(data)}')
  self.end_headers()
  try:
   for n in range(a,b+1,32768):self.wfile.write(data[n:min(n+32768,b+1)]);self.wfile.flush();time.sleep(.025)
  except OSError:pass
server=http.server.ThreadingHTTPServer(('127.0.0.1',0),Server);threading.Thread(target=server.serve_forever,daemon=True).start();url=f'http://127.0.0.1:{server.server_port}'
with tempfile.TemporaryDirectory(prefix='odm-store-') as temp:
 d=Path(temp);downloads=d/'downloads';downloads.mkdir();env=dict(os.environ,ODM_DATA_DIR=str(d/'state'));app=subprocess.Popen([str(exe),'--background'],env=env)
 def drive():
  seen=set()
  while not stop.wait(.05):
   h=helper['popup']('New download — Save and Start')
   if h and h not in seen:
    control=u.GetDlgItem(h,203)
    if control and u.IsWindowEnabled(control):
     file=Path(helper['getfield'](h,201)).name;helper['setfield'](h,201,str(downloads/file));helper['click'](h,203);seen.add(h)
   complete=helper['popup']('Download complete')
   if complete:helper['click'](complete,2)
   if not h:seen.clear()
 u.IsWindowEnabled.argtypes=[c.c_void_p];threading.Thread(target=drive,daemon=True).start()
 try:
  helper['wait'](lambda:u.FindWindowW('OpenDownloadManagerRust',None))
  cfg=d/'config.json';cfg.write_text(json.dumps(dict(profile=str(d/'chrome'),downloads=str(downloads),url=url,extension=str(root/'browser/Chromium/extension'))))
  result=subprocess.run(['node',str(Path(__file__).with_name('chrome.cjs')),str(cfg)],env=env,capture_output=True,text=True,timeout=90);print('Chrome:',result.stdout,result.stderr,flush=True);assert result.returncode==0
  for n in ['chrome.zip','chrome.pdf','chrome.future']:assert (downloads/n).read_bytes()==data
  print('PASS: Chrome downloaded three formats through installed native host; SHA-256 identical',flush=True)
  # Firefox native hosts run at Firefox's integrity level. Let the native host
  # start the app rather than reuse this elevated test runner's instance.
  helper['click'](u.FindWindowW('OpenDownloadManagerRust',None),119);app.wait(timeout=10)
  fixture='''(async()=>{while(!consentAccepted)await new Promise(r=>setTimeout(r,100));let result={};try{result.ping=await browser.runtime.sendNativeMessage(HOST,{action:"ping"});result.manual=await sendDownload(BASE+"/firefox.zip");const id=await browser.downloads.download({url:BASE+"/firefox.pdf",filename:"firefox.pdf",saveAs:false});for(let n=0;n<200;n++){const item=(await browser.downloads.search({id}))[0];if(item.state!=="in_progress"){result.capture={state:item.state,error:item.error};break;}await new Promise(r=>setTimeout(r,100));}}catch(e){result.error=e.message;}await fetch(BASE+"/report",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify(result)});})();'''.replace('BASE',json.dumps(url))
  extension=root/'browser/Firefox/extension';xpi=d/'test.xpi'
  with zipfile.ZipFile(xpi,'w') as z:
   for p in extension.iterdir():
    if p.name=='manifest.json':
     m=json.loads(p.read_text());m['background']['scripts'].append('integration.js');z.writestr(p.name,json.dumps(m))
    else:z.write(p,p.name)
   z.writestr('integration.js',fixture)
  log=open(d/'firefox-driver.log','w');driver=subprocess.Popen([str(Path(os.environ.get('ODM_GECKODRIVER') or shutil.which('geckodriver') or 'geckodriver.exe')),'--port','4445'],stdout=log,stderr=subprocess.STDOUT,env=dict(env,MOZ_REMOTE_ALLOW_SYSTEM_ACCESS='1'),creationflags=0x08000000);session=None
  def api(method,p,payload=None):
   req=urllib.request.Request('http://127.0.0.1:4445'+p,data=None if payload is None else json.dumps(payload).encode(),method=method,headers={'Content-Type':'application/json'})
   try:
    with urllib.request.urlopen(req,timeout=30) as response:return json.load(response)['value']
   except urllib.error.HTTPError as error:raise RuntimeError(error.read().decode()) from error
  try:
   time.sleep(1);s=api('POST','/session',{'capabilities':{'alwaysMatch':{'browserName':'firefox','moz:firefoxOptions':{'binary':r'C:\Program Files\Mozilla Firefox\firefox.exe','args':['-headless'],'prefs':{'browser.download.folderList':2,'browser.download.dir':str(d/'firefox-downloads'),'browser.download.useDownloadDir':True}}}}});session=s['sessionId'];api('POST',f'/session/{session}/moz/addon/install',{'path':str(xpi),'temporary':True})
   end=time.time()+15;handle=None
   while time.time()<end and not handle:
    for h in api('GET',f'/session/{session}/window/handles'):
     api('POST',f'/session/{session}/window',{'handle':h})
     if api('GET',f'/session/{session}/url').endswith('/consent.html'):handle=h;break
    time.sleep(.1)
   assert handle,'Firefox onboarding did not open'
   api('POST',f'/session/{session}/execute/sync',{'script':'document.querySelector("#accept").scrollIntoView({block:"center"}); return document.readyState;','args':[]});time.sleep(.3)
   element=api('POST',f'/session/{session}/element',{'using':'css selector','value':'#accept'});api('POST',f'/session/{session}/element/'+element['element-6066-11e4-a52e-4f735466cecf']+'/click',{})
   end=time.time()+40
   while not reports and time.time()<end:time.sleep(.1)
   assert reports,reports;print('Firefox:',json.dumps(reports),flush=True);assert not reports[0].get('error');assert reports[0]['ping']['ok'];assert reports[0]['capture']['state']=='interrupted'
   helper['wait'](lambda:(downloads/'firefox.pdf').exists());assert (downloads/'firefox.zip').read_bytes()==data;assert (downloads/'firefox.pdf').read_bytes()==data
   print('PASS: Firefox explicit consent, installed native ping, manual and automatic downloads; SHA-256 identical',flush=True)
  finally:
   if session:
    try:api('DELETE',f'/session/{session}')
    except Exception:pass
   driver.terminate();driver.wait(timeout=10);log.close()
 finally:
  stop.set();h=u.FindWindowW('OpenDownloadManagerRust',None)
  if h:helper['click'](h,119)
  try:app.wait(timeout=10)
  except subprocess.TimeoutExpired:app.terminate()
server.shutdown()
