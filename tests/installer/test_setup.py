import os,sys,runpy,subprocess,time,json,winreg,hashlib
from pathlib import Path
assert '--allow-install-test' in sys.argv, 'Use --allow-install-test in a disposable Windows user account.'
root=Path(__file__).resolve().parents[2];install=Path(os.environ['LOCALAPPDATA'])/'Programs/Open Download Manager';exe=install/'OpenDownloadManager.exe'
sys.argv=['check',str(exe)];helper=runpy.run_path(str(root/'tests/test_ui.py'));u=helper['u']
def registry(key,name=''):
 with winreg.OpenKey(winreg.HKEY_CURRENT_USER,key) as k:return winreg.QueryValueEx(k,name)[0]
assert registry(r'Software\Microsoft\Windows\CurrentVersion\Run','OpenDownloadManager')==f'"{exe}" --background'
for name,key in [('Firefox',r'Software\Mozilla\NativeMessagingHosts\com.opendownloadmanager'),('Chromium',r'Software\Google\Chrome\NativeMessagingHosts\com.opendownloadmanager')]:
 manifest=install/name/'native-host.json';assert registry(key)==str(manifest);assert json.loads(manifest.read_text())['path']==str(exe)
assert hashlib.sha256(exe.read_bytes()).digest()==hashlib.sha256((root.parent/'Bin/OpenDownloadManager.exe').read_bytes()).digest()
h=helper['wait'](lambda:u.FindWindowW('OpenDownloadManagerRust',None));assert not u.IsWindowVisible(h),'Startup must stay in tray'
initial=helper['wait'](lambda:helper['popup']('Browser plug-in'));helper['click'](initial,2);helper['wait'](lambda:not helper['popup']('Browser plug-in'))
helper['click'](h,124);dialog=helper['wait'](lambda:helper['popup']('Browser plug-in'))
assert helper['getfield'](dialog,701)=='Chrome';assert helper['getfield'](dialog,702)=='Firefox'
helper['click'](dialog,2);helper['click'](h,119);helper['wait'](lambda:not u.FindWindowW('OpenDownloadManagerRust',None))
print('PASS: fixed folder, executable hash, startup tray, native manifests, Browser plug-in dialog')
assert subprocess.run([str(install/'unins000.exe'),'/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART'],timeout=45).returncode==0
assert not exe.exists()
for key,name in [(r'Software\Microsoft\Windows\CurrentVersion\Run','OpenDownloadManager'),(r'Software\Mozilla\NativeMessagingHosts\com.opendownloadmanager',''),(r'Software\Google\Chrome\NativeMessagingHosts\com.opendownloadmanager','')]:
 try:registry(key,name);raise AssertionError('Uninstall left registry value')
 except FileNotFoundError:pass
print('PASS: uninstall removed installed app and its own startup/native registrations')
setup=root.parent/'Bin/ODM-Setup-0.2.9.exe';assert subprocess.run([str(setup),'/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART'],timeout=60).returncode==0
h=helper['wait'](lambda:u.FindWindowW('OpenDownloadManagerRust',None));helper['click'](h,119);helper['wait'](lambda:not u.FindWindowW('OpenDownloadManagerRust',None))
print('PASS: reinstall restored app and browser connections')
