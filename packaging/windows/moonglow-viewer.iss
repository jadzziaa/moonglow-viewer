; Inno Setup script for Moonglow Viewer's Windows installer.
;
;   iscc /DVersion=0.1.0 /DSource=..\..\target\dist packaging\windows\moonglow-viewer.iss
;
; Source holds moonglow-viewer.exe, mgv.exe and THIRD-PARTY-LICENSES.txt
; (packaging/windows/build-installer.ps1 puts them there). Installs per user
; by default (no administrator rights), per machine when chosen.

#ifndef Version
  #define Version "0.0.0"
#endif
#ifndef Source
  #define Source "..\..\target\dist"
#endif

[Setup]
AppId={{4A68CA89-D1D8-4AC3-A66C-43CE461AB6BD}
AppName=Moonglow Viewer
AppVersion={#Version}
AppVerName=Moonglow Viewer {#Version}
AppPublisher=The Moonglow Toolset contributors
DefaultDirName={autopf}\Moonglow Viewer
DefaultGroupName=Moonglow Viewer
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
LicenseFile=..\..\LICENSE
SetupIconFile=..\icons\moonglow-viewer.ico
UninstallDisplayIcon={app}\moonglow-viewer.exe
OutputDir={#Source}
OutputBaseFilename=MoonglowViewer-{#Version}-windows-x64-setup
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
ChangesAssociations=yes

[Tasks]
Name: desktopicon; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: mdlassoc; Description: "Open models (.mdl) with Moonglow Viewer"; GroupDescription: "File associations:"; Flags: unchecked
Name: addpath; Description: "Add the command-line tool (mgv) to PATH"; GroupDescription: "Command line:"; Flags: unchecked

[Files]
Source: "{#Source}\moonglow-viewer.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Source}\mgv.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"
Source: "{#Source}\THIRD-PARTY-LICENSES.txt"; DestDir: "{app}"
Source: "..\..\docs\manual\*"; DestDir: "{app}\manual"; Flags: recursesubdirs skipifsourcedoesntexist

[Icons]
Name: "{autoprograms}\Moonglow Viewer"; Filename: "{app}\moonglow-viewer.exe"
Name: "{autodesktop}\Moonglow Viewer"; Filename: "{app}\moonglow-viewer.exe"; Tasks: desktopicon

[Registry]
Root: HKA; Subkey: "Software\Classes\.mdl\OpenWithProgids"; ValueType: string; ValueName: "MoonglowViewer.Model"; ValueData: ""; Flags: uninsdeletevalue; Tasks: mdlassoc
Root: HKA; Subkey: "Software\Classes\MoonglowViewer.Model"; ValueType: string; ValueName: ""; ValueData: "Neverwinter Nights model"; Flags: uninsdeletekey; Tasks: mdlassoc
Root: HKA; Subkey: "Software\Classes\MoonglowViewer.Model\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\moonglow-viewer.exe,0"; Tasks: mdlassoc
Root: HKA; Subkey: "Software\Classes\MoonglowViewer.Model\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\moonglow-viewer.exe"" ""%1"""; Tasks: mdlassoc
Root: HKCU; Subkey: "Environment"; ValueType: expandsz; ValueName: "Path"; ValueData: "{olddata};{app}"; Check: NeedsAddPath(ExpandConstant('{app}')); Tasks: addpath

[Code]
function NeedsAddPath(Dir: string): Boolean;
var
  Path: string;
begin
  if not RegQueryStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', Path) then
  begin
    Result := True;
    exit;
  end;
  Result := Pos(';' + Uppercase(Dir) + ';', ';' + Uppercase(Path) + ';') = 0;
end;

[Run]
Filename: "{app}\moonglow-viewer.exe"; Description: "{cm:LaunchProgram,Moonglow Viewer}"; Flags: nowait postinstall skipifsilent
