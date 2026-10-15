; Instalador do GameRec (Inno Setup 6.7+). Gera dist\GameRec-Setup.exe.
;   ISCC /DAppVersion=2.1.0 installer\GameRec.iss
; Espera target\release\GameRec.exe e build\ffmpeg\ffmpeg.exe (o workflow de release prepara os dois).

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef AppExe
  #define AppExe "..\target\release\GameRec.exe"
#endif
#ifndef FFmpegDir
  #define FFmpegDir "..\build\ffmpeg"
#endif

[Setup]
AppId=GameRec
AppName=GameRec
AppVersion={#AppVersion}
AppVerName=GameRec {#AppVersion}
AppPublisher=eebagabe
AppPublisherURL=https://github.com/eebagabe/windows-rec
AppSupportURL=https://github.com/eebagabe/windows-rec/issues
AppUpdatesURL=https://github.com/eebagabe/windows-rec/releases
VersionInfoVersion={#AppVersion}
; sem administrador: instala em %LOCALAPPDATA%\Programs\GameRec, como a versao anterior
PrivilegesRequired=lowest
DefaultDirName={autopf}\GameRec
DisableWelcomePage=no
DisableProgramGroupPage=yes
UninstallDisplayName=GameRec
UninstallDisplayIcon={app}\GameRec.exe
SetupIconFile=..\assets\icon.ico
WizardStyle=modern dark
WizardImageFile=wizard-100.bmp,wizard-150.bmp,wizard-200.bmp
WizardSmallImageFile=small-100.bmp,small-150.bmp,small-200.bmp
WizardImageAlphaFormat=defined
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
Compression=lzma2/ultra64
SolidCompression=yes
CloseApplications=force
RestartApplications=no
OutputDir=..\dist
OutputBaseFilename=GameRec-Setup

[Languages]
Name: "pt"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"

[InstallDelete]
; restos da versao em Python (instalada pelos scripts antigos na mesma pasta)
Type: filesandordirs; Name: "{app}\app"
Type: filesandordirs; Name: "{app}\runtime"
Type: filesandordirs; Name: "{app}\_internal"
Type: files; Name: "{app}\uninstall.ps1"
Type: files; Name: "{app}\icon.ico"

[Files]
Source: "{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#FFmpegDir}\ffmpeg.exe"; DestDir: "{app}\bin"; Flags: ignoreversion
#ifexist AddBackslash(FFmpegDir) + "LICENSE.txt"
Source: "{#FFmpegDir}\LICENSE.txt"; DestDir: "{app}\bin"; DestName: "ffmpeg-LICENSE.txt"; Flags: ignoreversion
#endif

[Icons]
Name: "{autoprograms}\GameRec"; Filename: "{app}\GameRec.exe"; Comment: "Gravador de tela para gameplays"
Name: "{autodesktop}\GameRec"; Filename: "{app}\GameRec.exe"; Comment: "Gravador de tela para gameplays"; Tasks: desktopicon

[Registry]
; entrada de desinstalacao da versao antiga, substituida pela deste instalador
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Uninstall\GameRec"; ValueType: none; Flags: deletekey

[Run]
Filename: "{app}\GameRec.exe"; Description: "{cm:LaunchProgram,GameRec}"; Flags: nowait postinstall skipifsilent

[Code]
// Fecha o GameRec antes de instalar/desinstalar. Primeiro pede para fechar
// (se estiver gravando, ele salva o video); depois de 15 s, forca.
procedure CloseGameRec();
var
  Code, I: Integer;
begin
  // codigo 0 = havia um GameRec aberto
  if not Exec(ExpandConstant('{sys}\taskkill.exe'), '/IM GameRec.exe', '', SW_HIDE, ewWaitUntilTerminated, Code)
     or (Code <> 0) then
    Exit;
  for I := 1 to 30 do
  begin
    Sleep(500);
    // find devolve 1 quando o processo ja nao aparece
    if Exec(ExpandConstant('{cmd}'), '/C tasklist /FI "IMAGENAME eq GameRec.exe" /NH | find /I "GameRec.exe" >nul',
            '', SW_HIDE, ewWaitUntilTerminated, Code) and (Code <> 0) then
      Exit;
  end;
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /IM GameRec.exe', '', SW_HIDE, ewWaitUntilTerminated, Code);
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  CloseGameRec();
  Result := '';
end;

function InitializeUninstall(): Boolean;
begin
  CloseGameRec();
  Result := True;
end;
