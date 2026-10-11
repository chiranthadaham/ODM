from pathlib import Path
import concurrent.futures,hashlib,json,urllib.request,zipfile,shutil,argparse
parser=argparse.ArgumentParser();parser.add_argument("--yt-dlp-channel",choices=["stable","nightly"],default="nightly");options=parser.parse_args()
base=Path(__file__).resolve().parent/'video-runtime'
tools=base/'tools';notices=base/'licenses';tools.mkdir(exist_ok=True,parents=True);notices.mkdir(exist_ok=True)
def read(url):
    req=urllib.request.Request(url,headers={'User-Agent':'OpenDownloadManager-build'})
    with urllib.request.urlopen(req,timeout=90) as r:return r.read()
def provision(repo,filename,kind):
    meta=json.loads(read('https://api.github.com/repos/'+repo+'/releases/latest'))
    asset=next(a for a in meta['assets'] if a['name']==filename)
    cache=base/filename
    if not cache.exists() or cache.stat().st_size!=asset['size']:cache.write_bytes(read(asset['browser_download_url']))
    digest=hashlib.sha256(cache.read_bytes()).hexdigest()
    expected=asset.get('digest')
    if expected and expected.startswith('sha256:'):assert digest==expected[7:]
    if kind=='yt-dlp':
        sums=read(next(a['browser_download_url'] for a in meta['assets'] if a['name']=='SHA2-256SUMS')).decode()
        expected=next(line.split()[0] for line in sums.splitlines() if line.split()[-1].lstrip('*')==filename)
        assert digest==expected
        shutil.copy2(cache,tools/'yt-dlp.exe')
        for name in ['LICENSE','THIRD_PARTY_LICENSES.txt']:
            try:(notices/('yt-dlp-'+name)).write_bytes(read('https://raw.githubusercontent.com/yt-dlp/yt-dlp/'+('master' if options.yt_dlp_channel=='nightly' else meta['tag_name'])+'/'+name))
            except Exception:pass
    else:
        with zipfile.ZipFile(cache) as z:
            for item in z.infolist():
                p=Path(item.filename)
                if kind=='deno' and p.name=='deno.exe':(tools/p.name).write_bytes(z.read(item))
                elif kind=='ffmpeg' and (p.suffix.lower()=='.dll' or p.name in ['ffmpeg.exe','ffprobe.exe']):
                    (tools/p.name).write_bytes(z.read(item))
                elif kind=='ffmpeg' and ('license' in p.name.lower() or p.name.lower().startswith('copying')) and not item.is_dir():
                    (notices/('ffmpeg-'+p.name)).write_bytes(z.read(item))
        if kind=='deno':(notices/'deno-LICENSE.md').write_bytes(read('https://raw.githubusercontent.com/denoland/deno/'+meta['tag_name']+'/LICENSE.md'))
    print(kind,meta['tag_name'],digest,flush=True)
    return {'component':kind,'repository':repo,'version':meta['tag_name'],'asset':filename,'url':asset['browser_download_url'],'sha256':digest,'release':meta['html_url']}
items=[('yt-dlp/yt-dlp-nightly-builds' if options.yt_dlp_channel=='nightly' else 'yt-dlp/yt-dlp','yt-dlp.exe','yt-dlp'),('denoland/deno','deno-x86_64-pc-windows-msvc.zip','deno'),('yt-dlp/FFmpeg-Builds','ffmpeg-master-latest-win64-gpl-shared.zip','ffmpeg')]
with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:versions=list(pool.map(lambda x:provision(*x),items))
(base/'versions.json').write_text(json.dumps(versions,indent=2),encoding='utf-8')
(notices/'COMPONENTS-AND-SOURCE.txt').write_text('\n\n'.join(v['component']+' '+v['version']+'\n'+v['release']+'\nSource: https://github.com/'+v['repository'] for v in versions)+'\n\nFFmpeg GPL source/build information: https://github.com/yt-dlp/FFmpeg-Builds\nFFmpeg upstream: https://git.ffmpeg.org/ffmpeg.git\n',encoding='utf-8')
print('Video runtime ready',flush=True)
