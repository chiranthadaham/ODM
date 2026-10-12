"""Real app downloads, with independent full-file references and retained evidence."""
from pathlib import Path
import concurrent.futures, datetime, hashlib, html, json, os, subprocess, time, urllib.parse, zipfile, tarfile
import sys
EXE=Path(sys.argv[1]).resolve()
ROOT=Path(sys.argv[2]).resolve() if len(sys.argv)>2 else Path('site-audit-results').resolve()
ROOT.mkdir(parents=True,exist_ok=True)
CURL=Path(os.environ['SystemRoot'])/'System32/curl.exe'
rows=json.loads(Path(__file__).with_name('sites100.json').read_text(encoding='utf-8'))
def sha(p):
    h=hashlib.sha256()
    with p.open('rb')as f:
        for b in iter(lambda:f.read(1024*1024),b''):h.update(b)
    return h.hexdigest()
def is_html(p):
    with p.open('rb')as f:b=f.read(2048).lstrip().lower()
    return b.startswith((b'<!doctype html',b'<html')) or b'<html' in b[:512]
def test(pair):
    i,r=pair;r=dict(r,id=i);folder=ROOT/f'{i:03}';folder.mkdir(exist_ok=True)
    target=folder/'download.bin';ref=folder/'reference.bin';start=time.monotonic()
    env=dict(os.environ,ODM_DATA_DIR=str(folder/'state'))
    try:
        p=subprocess.run([str(EXE),'--download',r['url'],str(target),'--connections','4'],timeout=150,env=env,creationflags=subprocess.CREATE_NO_WINDOW)
        r['appExit']=p.returncode
        report=Path(str(target)+'.result.txt')
        r['appReport']=report.read_text(encoding='utf-8') if report.exists() else 'No result report'
        if target.exists():r.update(bytes=target.stat().st_size,sha256=sha(target),html=is_html(target))
        c=subprocess.run([str(CURL),'-L','--fail','--silent','--show-error','--max-time','80','--max-filesize','134217728','-D',str(folder/'headers.txt'),'-o',str(ref),r['url']],capture_output=True,timeout=90,creationflags=subprocess.CREATE_NO_WINDOW)
        r.update(referenceExit=c.returncode,referenceError=c.stderr.decode(errors='replace')[-1500:])
        if c.returncode==0 and ref.exists():r.update(referenceBytes=ref.stat().st_size,referenceSha256=sha(ref),referenceHtml=is_html(ref))
        if p.returncode==0 and r.get('bytes',0)>0 and not r.get('html'):
            r['outcome']='PASS_HASH_MATCH' if r.get('sha256')==r.get('referenceSha256') else 'APP_COMPLETE_REFERENCE_DIFFERS' if c.returncode==0 else 'APP_COMPLETE_REFERENCE_UNAVAILABLE'
            if zipfile.is_zipfile(target):
                with zipfile.ZipFile(target)as z:
                    bad=z.testzip()
                    if bad:r['outcome']='FAIL_ARCHIVE_CRC';r['archiveError']=bad
                    else:r['archiveCheck']='ZIP CRC valid'
        elif c.returncode==0 and not r.get('referenceHtml'):r['outcome']='FAIL_APP_ONLY'
        else:r['outcome']='SITE_OR_LINK_RESTRICTION'
    except subprocess.TimeoutExpired:r['outcome']='TEST_TIMEOUT'
    except Exception as e:r.update(outcome='TEST_ERROR',error=str(e))
    r['seconds']=round(time.monotonic()-start,2)
    (folder/'result.json').write_text(json.dumps(r,indent=2),encoding='utf-8')
    # Keep evidence, not a second copy of every downloaded installer.
    if ref.exists():ref.unlink()
    return r
results=[]
with concurrent.futures.ThreadPoolExecutor(max_workers=5)as pool:
    for r in pool.map(test,enumerate(rows,1)):
        results.append(r);print(f"{len(results)}/{len(rows)} {r['site']}: {r['outcome']} ({r.get('bytes',0)} bytes)",flush=True)
        (ROOT/'progress.json').write_text(json.dumps(results,indent=2),encoding='utf-8')
meta=dict(date=datetime.datetime.now().isoformat(),version='0.2.7',binarySha256=sha(EXE),sites=100,attempts=len(rows),
 method='Full-file downloads through the Rust app CLI (same transfer engine as UI), five cases concurrently, four connections each. Fresh independent Windows curl references, SHA256 comparison, ZIP CRC when applicable. No downloaded program was executed. Browser capture was not tested across these sites.',
 selection='100 distinct publishers from the earlier curated popular-site audit, choosing manageable public samples; 10 additional format cases. This is not an authoritative ranking of the top 100 websites. Several publishers use shared GitHub or SourceForge hosting.',
 counts={s:sum(r['outcome']==s for r in results)for s in sorted({r['outcome']for r in results})},results=results)
out=ROOT/'report';out.with_suffix('.json').write_text(json.dumps(meta,indent=2),encoding='utf-8')
table=''.join('<tr><td>'+str(r['id'])+'</td><td>'+html.escape(r['site'])+'</td><td>'+html.escape(r['sample'])+'</td><td>'+html.escape(r['outcome'])+'</td><td>'+str(r.get('bytes','â€”'))+'</td><td><a href="'+html.escape(r['url'],quote=True)+'">Sample link</a><details><summary>Evidence</summary><pre>'+html.escape(json.dumps(r,indent=2))+'</pre></details></td></tr>'for r in results)
out.with_suffix('.html').write_text('<!doctype html><html><meta charset="utf-8"><title>ODM Rust â€” 100-site download audit</title><style>body{font:15px system-ui;color:#172638;background:#f6f8fa;margin:36px}main{max-width:1450px;margin:auto}h1{font-size:30px}table{width:100%;border-collapse:collapse;background:white;border-radius:16px}td,th{padding:14px;text-align:left;border-bottom:1px solid #e6eaf0}pre{white-space:pre-wrap;word-break:break-word;max-width:600px;font-size:12px}a{color:#007aff}</style><main><h1>Open Download Manager Â· Rust 0.2</h1><h2>100-site download audit</h2><p>'+html.escape(meta['method'])+'</p><p>'+html.escape(meta['selection'])+'</p><p><strong>'+html.escape(str(meta['counts']))+'</strong></p><p>App completion with a different or unavailable reference is reported separately from verified hash matches. Restrictions and timeouts are not passes.</p><table><tr><th>#</th><th>Publisher</th><th>Sample</th><th>Result</th><th>Bytes</th><th>Details</th></tr>'+table+'</table></main></html>',encoding='utf-8')
print(json.dumps(meta['counts']),flush=True)
