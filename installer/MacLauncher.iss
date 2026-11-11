; Cargo owns the version. Build through scripts/package-windows.ps1.
#ifndef MyAppVersion
  #error Run scripts/package-windows.ps1
#endif
#ifndef MyNumericVersion
  #error Missing numeric version
#endif
#ifndef MyAppExeSource
  #error Missing tested executable
#endif
#define MyAppName "macOS System Launcher"
#define MyAppExeName "win-launcher.exe"
[Setup]
AppId={{D37E84B1-29FB-4E4A-B829-57DFB0E5A73C}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
VersionInfoVersion={#MyNumericVersion}
AppPublisher=MacLauncher
AppPublisherURL=https://github.com/yubin0210/mac-windows-launcher
DefaultDirName={localappdata}\Programs\MacLauncher
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
OutputDir=..\target\installer
OutputBaseFilename=MacLauncher-Setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
CloseApplications=no
RestartApplications=no
UninstallDisplayIcon={app}\{#MyAppExeName}
[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
[Tasks]
Name: "desktopicon"; Description: "Create Desktop shortcut"; Flags: unchecked
Name: "startmenuicon"; Description: "Create Start Menu shortcut"
Name: "autostart"; Description: "Launch when Windows starts"; Flags: unchecked
[Files]
Source: "{#MyAppExeSource}"; DestDir: "{app}"; Flags: ignoreversion
[Icons]
Name: "{autoprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: startmenuicon
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon
Name: "{userstartup}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: autostart
[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "Launch MacLauncher"; Flags: nowait postinstall skipifsilent; Check: AllowLaunch
; No recursive UninstallDelete. Only tracked files are removed.
; Settings/logs live separately in {localappdata}\MacLauncher and are preserved.
[Code]
function OpenProcess(Access: DWORD; Inherit: BOOL; ProcessId: DWORD): THandle;
  external 'OpenProcess@kernel32.dll stdcall';
function QueryFullProcessImageNameW(Process: THandle; Flags: DWORD; Buffer: String; var Size: DWORD): BOOL;
  external 'QueryFullProcessImageNameW@kernel32.dll stdcall';
function GetWindowThreadProcessId(Window: HWND; var ProcessId: DWORD): DWORD;
  external 'GetWindowThreadProcessId@user32.dll stdcall';
function WaitForSingleObject(Handle: THandle; Milliseconds: DWORD): DWORD;
  external 'WaitForSingleObject@kernel32.dll stdcall';
function CloseHandle(Handle: THandle): BOOL;
  external 'CloseHandle@kernel32.dll stdcall';
function AllowLaunch: Boolean;
begin
  Result := ExpandConstant('{param:NoLaunch|0}') <> '1';
end;
function CloseInstalledLauncher: Boolean;
var
  Window: HWND;
  ProcessId, Size: DWORD;
  Process: THandle;
  Path: String;
begin
  Result := True;
  Window := FindWindowByClassName('MacWinLauncherDockClass');
  if Window = 0 then exit;
  Result := False;
  GetWindowThreadProcessId(Window, ProcessId);
  if ProcessId = 0 then exit;
  Process := OpenProcess($00101000, False, ProcessId);
  if Process = 0 then exit;
  try
    Size := 32768;
    SetLength(Path, Size);
    if not QueryFullProcessImageNameW(Process, 0, Path, Size) then exit;
    SetLength(Path, Size);
    if CompareText(ExpandFileName(Path), ExpandConstant('{app}\{#MyAppExeName}')) <> 0 then
    begin
      Result := True; { Another copy is not owned by this installation. }
      exit;
    end;
    if not PostMessage(Window, $0010, 0, 0) then exit;
    Result := WaitForSingleObject(Process, 10000) = 0;
  finally
    CloseHandle(Process);
  end;
end;
function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  Result := '';
  if not CloseInstalledLauncher then
    Result := 'Close MacLauncher from its tray and retry. Setup will not force-close it.';
end;
function InitializeUninstall: Boolean;
begin
  Result := CloseInstalledLauncher;
  if not Result then
    SuppressibleMsgBox('Close MacLauncher from its tray and retry uninstalling.', mbError, MB_OK, IDOK);
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  Shell, Shortcut: Variant;
  StartupPath, LegacyLocation, LegacyCommand: String;
begin
  if CurStep <> ssPostInstall then exit;
  { Remove an old automatic-start shortcut only if its target is this installation. }
  if not WizardIsTaskSelected('autostart') then
  begin
    StartupPath := ExpandConstant('{userstartup}\{#MyAppName}.lnk');
    if FileExists(StartupPath) then
    try
      Shell := CreateOleObject('WScript.Shell');
      Shortcut := Shell.CreateShortcut(StartupPath);
      if CompareText(Shortcut.TargetPath, ExpandConstant('{app}\{#MyAppExeName}')) = 0 then
        DeleteFile(StartupPath);
    except
      Log('Could not inspect old startup shortcut: ' + GetExceptionMessage);
    end;
  end;
  { Remove only the old script registration whose location and command we recognize. }
  if RegQueryStringValue(HKCU, 'Software\Microsoft\Windows\CurrentVersion\Uninstall\MacLauncher', 'InstallLocation', LegacyLocation) and
     RegQueryStringValue(HKCU, 'Software\Microsoft\Windows\CurrentVersion\Uninstall\MacLauncher', 'UninstallString', LegacyCommand) then
    if (CompareText(LegacyLocation, ExpandConstant('{app}')) = 0) and
       (CompareText(LegacyCommand, 'powershell.exe -ExecutionPolicy Bypass -File "' + ExpandConstant('{app}\uninstall.ps1') + '"') = 0) then
      RegDeleteKeyIncludingSubkeys(HKCU, 'Software\Microsoft\Windows\CurrentVersion\Uninstall\MacLauncher');
end;
