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
Name: "{userdesktop}\Open Download Manager"; Filename: "{app}\OpenDownloadManager.exe"; WorkingDir: "{app}"
[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "OpenDownloadManager"; ValueData: """{app}\OpenDownloadManager.exe"" --background"
Root: HKCU; Subkey: "Software\Mozilla\NativeMessagingHosts\com.opendownloadmanager"; ValueType: string; ValueData: "{app}\Firefox\native-host.json"
Root: HKCU; Subkey: "Software\Google\Chrome\NativeMessagingHosts\com.opendownloadmanager"; ValueType: string; ValueData: "{app}\Chromium\native-host.json"
[Run]
Filename: "{app}\OpenDownloadManager.exe"; Parameters: "--background"; Flags: nowait runasoriginaluser; Description: "Start ODM in the system tray"
[UninstallDelete]
Type: files; Name: "{app}\Firefox\native-host.json"
Type: files; Name: "{app}\Chromium\native-host.json"
[Code]
function InitializeSetup(): Boolean;
begin
  Result := not CheckForMutexes('Local\OpenDownloadManagerRust');
  if not Result then MsgBox('ODM is running in the system tray. Choose Exit from its tray menu before installing an update.', mbInformation, MB_OK);
end;

procedure InitializeUninstallProgressForm();
var Window: HWND;
    Attempts: Integer;
begin
  if not CheckForMutexes('Local\OpenDownloadManagerRust') then Exit;
  if MsgBox('ODM is running in the system tray. Close it and continue uninstalling? Active downloads will be paused and downloaded files will be kept.', mbConfirmation, MB_YESNO) <> idYes then Abort;
  Window := FindWindowByClassName('OpenDownloadManagerRust');
  if Window <> 0 then PostMessage(Window, $0111, 119, 0);
  Attempts := 0;
  while CheckForMutexes('Local\OpenDownloadManagerRust') and (Attempts < 300) do begin
    Sleep(100);
    Attempts := Attempts + 1;
  end;
  if CheckForMutexes('Local\OpenDownloadManagerRust') then begin
    MsgBox('ODM is still finishing its downloads. Choose Exit from its tray menu, wait for it to close, then run uninstall again.', mbError, MB_OK);
    Abort;
  end;
end;

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

