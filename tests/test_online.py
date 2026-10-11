"""Optional real HTTPS tests; set ODM_ONLINE_TESTS=1 to enable network downloads."""
from pathlib import Path
import tempfile,subprocess,sys,os,unittest
EXE=Path(sys.argv.pop(1)).resolve()
@unittest.skipUnless(os.environ.get('ODM_ONLINE_TESTS')=='1','Opt-in internet test')
class Tests(unittest.TestCase):
    def download(self,url,name):
        with tempfile.TemporaryDirectory()as tmp:
            root=Path(tmp);target=root/name
            p=subprocess.run([str(EXE),'--download',url,str(target)],timeout=90,env=dict(os.environ,ODM_DATA_DIR=str(root/'state')),creationflags=subprocess.CREATE_NO_WINDOW)
            report=Path(str(target)+'.result.txt').read_text(encoding='utf-8');self.assertEqual(p.returncode,0,report);return target.read_bytes()
    def test_github_text_over_https(self):self.assertIn(b'Permission is hereby granted',self.download('https://raw.githubusercontent.com/rust-lang/rust/master/LICENSE-MIT','LICENSE.txt'))
    def test_mozilla_pdf_over_https(self):self.assertTrue(self.download('https://raw.githubusercontent.com/mozilla/pdf.js/master/web/compressed.tracemonkey-pldi-09.pdf','document.pdf').startswith(b'%PDF-'))
if __name__=='__main__':unittest.main()
