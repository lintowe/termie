; Inno Setup script for termie — builds a per-user installer exe.
; Compile with:  ISCC.exe /DMyAppVersion=0.2.5 installer.iss
; Mirrors install.ps1: install to %LOCALAPPDATA%\Programs\termie, Start-menu +
; desktop shortcuts, add to PATH, the "Open in termie" context-menu verb, an
; App Paths entry so `termie` resolves from Win+R, and a clean uninstaller.

#ifndef MyAppVersion
  #define MyAppVersion "0.0.0"
#endif
#define MyAppName "termie"
#define MyAppExe "termie.exe"
#define MyAppPublisher "lintowe"
#define MyAppUrl "https://github.com/lintowe/termie"

[Setup]
AppId={{47F9CB96-4F69-487E-911F-C94B7B93F27D}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppUrl}
AppSupportURL={#MyAppUrl}
DefaultDirName={localappdata}\Programs\termie
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
DisableDirPage=auto
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=dist
OutputBaseFilename=termie-{#MyAppVersion}-windows-x64-setup
SetupIconFile=assets\icon.ico
UninstallDisplayIcon={app}\{#MyAppExe}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ChangesEnvironment=yes
CloseApplications=yes

[Tasks]
Name: "desktopicon"; Description: "Create a &desktop shortcut"; GroupDescription: "Additional shortcuts:"

[Files]
Source: "target\release\{#MyAppExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "assets\*"; DestDir: "{app}\assets"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExe}"; WorkingDir: "{%USERPROFILE}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExe}"; WorkingDir: "{%USERPROFILE}"; Tasks: desktopicon

[Registry]
; add the install dir to the user PATH (deduped by NeedsAddPath); Inno broadcasts
; WM_SETTINGCHANGE because ChangesEnvironment=yes, so shells pick it up
Root: HKCU; Subkey: "Environment"; ValueType: expandsz; ValueName: "Path"; ValueData: "{olddata};{app}"; Check: NeedsAddPath('{app}')

; "Open in termie" — right-click a folder or its background. %V is the clicked dir
Root: HKCU; Subkey: "Software\Classes\Directory\shell\termie"; ValueType: string; ValueName: ""; ValueData: "Open in termie"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Directory\shell\termie"; ValueType: string; ValueName: "Icon"; ValueData: "{app}\{#MyAppExe}"
Root: HKCU; Subkey: "Software\Classes\Directory\shell\termie\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExe}"" --cwd ""%V"""
Root: HKCU; Subkey: "Software\Classes\Directory\Background\shell\termie"; ValueType: string; ValueName: ""; ValueData: "Open in termie"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Directory\Background\shell\termie"; ValueType: string; ValueName: "Icon"; ValueData: "{app}\{#MyAppExe}"
Root: HKCU; Subkey: "Software\Classes\Directory\Background\shell\termie\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExe}"" --cwd ""%V"""

; App Paths so `termie` resolves from Win+R / ShellExecute
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\App Paths\termie.exe"; ValueType: string; ValueName: ""; ValueData: "{app}\{#MyAppExe}"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\App Paths\termie.exe"; ValueType: string; ValueName: "Path"; ValueData: "{app}"

[Run]
Filename: "{app}\{#MyAppExe}"; Description: "Launch termie"; Flags: nowait postinstall skipifsilent

[Code]
function NeedsAddPath(Param: string): boolean;
var
  OrigPath: string;
begin
  if not RegQueryStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', OrigPath) then
  begin
    Result := True;
    exit;
  end;
  Result := Pos(';' + Uppercase(ExpandConstant(Param)) + ';', ';' + Uppercase(OrigPath) + ';') = 0;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  OrigPath, AppDir, NewPath: string;
begin
  if CurUninstallStep = usUninstall then
  begin
    if RegQueryStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', OrigPath) then
    begin
      AppDir := ExpandConstant('{app}');
      NewPath := ';' + OrigPath + ';';
      StringChangeEx(NewPath, ';' + AppDir + ';', ';', True);
      if (Length(NewPath) > 0) and (NewPath[1] = ';') then Delete(NewPath, 1, 1);
      if (Length(NewPath) > 0) and (NewPath[Length(NewPath)] = ';') then Delete(NewPath, Length(NewPath), 1);
      RegWriteExpandStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', NewPath);
    end;
  end;
end;
