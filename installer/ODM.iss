#define AppVersion "0.2.0"
#ifndef ChromeId
  #define ChromeId "jancefngmifbnaldahilleoinncjppjh"
#endif
[Setup]
AppId={{D7A80F45-7A9D-45C9-9F17-62ACAE677AF0}
AppName=Open Download Manager
AppVersion={#AppVersion}
AppPublisher=Open Download Manager contributors
DefaultDirName={localappdata}\Programs\Open Download Manager
UsePreviousAppDir=no
DisableDirPage=yes
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
SetupIconFile=..\assets\app.ico
OutputDir=..\..\Bin
OutputBaseFilename=ODM-Setup-{#AppVersion}
Compression=lzma2/fast
SolidCompression=yes
WizardStyle=modern
LicenseFile=..\LICENSE
UninstallDisplayIcon={app}\OpenDownloadManager.exe
CloseApplications=yes
RestartApplications=no
AppMutex=Local\OpenDownloadManagerRust
SetupLogging=yes
[Files]
Source: "..\..\Bin\OpenDownloadManager.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\Bin\browser-stores.json"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\Bin\README.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\Bin\licenses\*"; DestDir: "{app}\licenses"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "..\..\Bin\Firefox\extension\*"; DestDir: "{app}\Firefox\extension"; Flags: ignoreversion recursesubdirs
Source: "..\..\Bin\Chromium\extension\*"; DestDir: "{app}\Chromium\extension"; Flags: ignoreversion recursesubdirs
[Icons]
Name: "{autoprograms}\Open Download Manager"; Filename: "{app}\OpenDownloadManager.exe"
[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "OpenDownloadManager"; ValueData: """{app}\OpenDownloadManager.exe"" --background"
Root: HKCU; Subkey: "Software\Mozilla\NativeMessagingHosts\com.opendownloadmanager"; ValueType: string; ValueData: "{app}\Firefox\native-host.json"
Root: HKCU; Subkey: "Software\Google\Chrome\NativeMessagingHosts\com.opendownloadmanager"; ValueType: string; ValueData: "{app}\Chromium\native-host.json"
[Run]
Filename: "{app}\OpenDownloadManager.exe"; Parameters: "--background --browser-plugins"; Flags: nowait runasoriginaluser; Description: "Start ODM and choose your browser plug-in"
[UninstallDelete]
Type: files; Name: "{app}\Firefox\native-host.json"
Type: files; Name: "{app}\Chromium\native-host.json"
[Code]
function JsonEscape(Value: String): String;
begin
  StringChangeEx(Value, '\', '\\', True);
  StringChangeEx(Value, '"', '\"', True);
  Result := Value;
end;
procedure CurStepChanged(CurStep: TSetupStep);
var AppPath, Common: String;
    Lines: TArrayOfString;
begin
  if CurStep = ssPostInstall then begin
    AppPath := JsonEscape(ExpandConstant('{app}\OpenDownloadManager.exe'));
    Common := '{"name":"com.opendownloadmanager","description":"Open Download Manager","path":"' + AppPath + '","type":"stdio",';
    SetArrayLength(Lines, 1);
    Lines[0] := Common + '"allowed_extensions":["opendownloadmanager@local"]}';
    if not SaveStringsToUTF8FileWithoutBOM(ExpandConstant('{app}\Firefox\native-host.json'), Lines, False) then RaiseException('Cannot write Firefox native host.');
    Lines[0] := Common + '"allowed_origins":["chrome-extension://{#ChromeId}/"]}';
    if not SaveStringsToUTF8FileWithoutBOM(ExpandConstant('{app}\Chromium\native-host.json'), Lines, False) then RaiseException('Cannot write Chrome native host.');
  end;
end;
procedure RemoveOwnRegistration(Subkey, Name, Expected: String);
var Value: String;
begin
  if RegQueryStringValue(HKCU, Subkey, Name, Value) and (Value = Expected) then begin
    RegDeleteValue(HKCU, Subkey, Name);
    if Name = '' then RegDeleteKeyIfEmpty(HKCU, Subkey);
  end;
end;
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then begin
    RemoveOwnRegistration('Software\Microsoft\Windows\CurrentVersion\Run', 'OpenDownloadManager', ExpandConstant('"{app}\OpenDownloadManager.exe" --background'));
    RemoveOwnRegistration('Software\Mozilla\NativeMessagingHosts\com.opendownloadmanager', '', ExpandConstant('{app}\Firefox\native-host.json'));
    RemoveOwnRegistration('Software\Google\Chrome\NativeMessagingHosts\com.opendownloadmanager', '', ExpandConstant('{app}\Chromium\native-host.json'));
  end;
end;
