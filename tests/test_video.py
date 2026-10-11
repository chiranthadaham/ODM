"""Generate a local HLS fixture and validate the Rust app's merged transport stream."""
from pathlib import Path
import functools,http.server,threading,tempfile,subprocess,sys,os,json,unittest
EXE=Path(sys.argv.pop(1)).resolve()
TOOLS=Path(os.environ.get('ODM_TEST_TOOLS',str(Path(os.environ['LOCALAPPDATA'])/'OpenDownloadManagerCpp/tools-7c28ee7807864b1ddf2f')))
class Quiet(http.server.SimpleHTTPRequestHandler):
    def log_message(self,*args):pass
class Tests(unittest.TestCase):
    def test_hls_download_and_merge(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);media=root/'server';media.mkdir();out=root/'output';out.mkdir()
            subprocess.run([str(TOOLS/'ffmpeg.exe'),'-hide_banner','-loglevel','error','-f','lavfi','-i','testsrc=size=160x90:rate=10','-f','lavfi','-i','sine=frequency=440:sample_rate=44100','-t','3','-c:v','libx264','-pix_fmt','yuv420p','-g','10','-c:a','aac','-f','hls','-hls_time','1','-hls_list_size','0',str(media/'sample.m3u8')],check=True,timeout=30,creationflags=subprocess.CREATE_NO_WINDOW)
            server=http.server.ThreadingHTTPServer(('127.0.0.1',0),functools.partial(Quiet,directory=str(media)))
            threading.Thread(target=server.serve_forever,daemon=True).start()
            try:
                target=out/'video.ts';result=subprocess.run([str(EXE),'--download',f'http://127.0.0.1:{server.server_port}/sample.m3u8',str(target)],timeout=90,env=dict(os.environ,ODM_DATA_DIR=str(root/'state'),LOCALAPPDATA=str(root/'fresh-cache')),creationflags=subprocess.CREATE_NO_WINDOW)
                report=Path(str(target)+'.result.txt').read_text(encoding='utf-8');self.assertEqual(result.returncode,0,report)
                final=next(out.glob('*.ts'));probe=subprocess.check_output([str(TOOLS/'ffprobe.exe'),'-v','error','-show_format','-show_streams','-of','json',str(final)],timeout=15,creationflags=subprocess.CREATE_NO_WINDOW)
                info=json.loads(probe);self.assertGreater(float(info['format']['duration']),2.9);self.assertEqual({s['codec_type']for s in info['streams']},{'audio','video'})
                self.assertFalse(list(out.glob('.odm-video-*')))
            finally:server.shutdown();server.server_close()
if __name__=='__main__':unittest.main()
