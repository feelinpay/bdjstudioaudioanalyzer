; Script de instalación Inno Setup 6 para BDJ Studio Audio Analyzer
; Control de Calidad Forense para DJs · 100% Offline
; No requiere elevación administrativa (ADR 09).

#ifndef MyAppVersion
  #define MyAppVersion "1.0.0"
#endif
#define MyAppName "BDJ Studio Audio Analyzer"
#define MyAppPublisher "BDJ Studio"
#define MyAppExeName "bdj_studio_audio_analyzer.exe"

[Setup]
AppId={{E57D8A29-4B8C-4D1E-8B32-7A4B1C9D0E81}}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
OutputDir=.
OutputBaseFilename=BDJ_Studio_Audio_Analyzer_Setup_{#MyAppVersion}
Compression=lzma2/ultra64
SolidCompression=yes
PrivilegesRequired=lowest
ArchitecturesInstallIn64BitMode=x64compatible
SetupIconFile=..\frontend\windows\runner\resources\app_icon.ico
UninstallDisplayIcon={app}\{#MyAppExeName}
WizardStyle=modern

[Languages]
Name: "spanish"; MessagesFile: "compiler:Languages\Spanish.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: checkedonce

[Files]
Source: "..\frontend\build\windows\x64\runner\Release\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "..\logo.png"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{group}\Desinstalar {#MyAppName}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#MyAppName}}"; Flags: nowait postinstall skipifsilent

[UninstallRun]
; Cerrar procesos activos antes de desinstalar
Filename: "{sys}\taskkill.exe"; Parameters: "/F /IM {#MyAppExeName} /T"; Flags: runhidden
Filename: "{sys}\taskkill.exe"; Parameters: "/F /IM bdja_worker.exe /T"; Flags: runhidden
Filename: "{sys}\taskkill.exe"; Parameters: "/F /IM bdja_cli.exe /T"; Flags: runhidden
Filename: "{sys}\cmd.exe"; Parameters: "/c ping 127.0.0.1 -n 2 > nul"; Flags: runhidden

[UninstallDelete]
; Política BDJ Studio (compartida con las demás apps): nada sobrevive a la desinstalación.
Type: filesandordirs; Name: "{userappdata}\BDJ Studio\BDJ Studio Audio Analyzer"
Type: filesandordirs; Name: "{userappdata}\BDJ Studio\bdj_studio_audio_analyzer"
Type: filesandordirs; Name: "{userappdata}\BDJ Studio Audio Analyzer"
Type: filesandordirs; Name: "{userappdata}\bdj_studio_audio_analyzer"
Type: filesandordirs; Name: "{localappdata}\bdj_studio_audio_analyzer"
Type: filesandordirs; Name: "{localappdata}\BDJ Studio\BDJ Studio Audio Analyzer"
Type: filesandordirs; Name: "{localappdata}\BDJ Studio\bdj_studio_audio_analyzer"
Type: filesandordirs; Name: "{app}"

[Code]
// Eliminación forzada y recursiva de todos los datos en AppData al desinstalar.
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  DataDir: String;
begin
  if CurUninstallStep = usPostUninstall then
  begin
    // 1. Borrar datos de Roaming
    DataDir := ExpandConstant('{userappdata}\BDJ Studio\BDJ Studio Audio Analyzer');
    if DirExists(DataDir) then DelTree(DataDir, True, True, True);

    DataDir := ExpandConstant('{userappdata}\BDJ Studio\bdj_studio_audio_analyzer');
    if DirExists(DataDir) then DelTree(DataDir, True, True, True);

    DataDir := ExpandConstant('{userappdata}\BDJ Studio Audio Analyzer');
    if DirExists(DataDir) then DelTree(DataDir, True, True, True);

    DataDir := ExpandConstant('{userappdata}\bdj_studio_audio_analyzer');
    if DirExists(DataDir) then DelTree(DataDir, True, True, True);

    // 2. Borrar datos de Local
    DataDir := ExpandConstant('{localappdata}\BDJ Studio\BDJ Studio Audio Analyzer');
    if DirExists(DataDir) then DelTree(DataDir, True, True, True);

    DataDir := ExpandConstant('{localappdata}\BDJ Studio\bdj_studio_audio_analyzer');
    if DirExists(DataDir) then DelTree(DataDir, True, True, True);

    DataDir := ExpandConstant('{localappdata}\bdj_studio_audio_analyzer');
    if DirExists(DataDir) then DelTree(DataDir, True, True, True);

    // 3. Limpieza de carpetas de marca padre:
    // RemoveDir SOLO tiene éxito si la carpeta está completamente vacía (no quedan otras apps).
    RemoveDir(ExpandConstant('{userappdata}\BDJ Studio'));
    RemoveDir(ExpandConstant('{localappdata}\BDJ Studio'));
  end;
end;
