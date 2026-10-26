; Instalador do WinRec (Inno Setup 6.7+). Gera dist\WinRec-Setup.exe.
;   ISCC /DAppVersion=3.0.0 installer\WinRec.iss
; Espera publish\WinRec.exe, build\ffmpeg\ffmpeg.exe e as imagens do assistente
; (o build.ps1 e o workflow de release preparam os três).

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef AppExe
  #define AppExe "..\publish\WinRec.exe"
#endif
#ifndef FFmpegDir
  #define FFmpegDir "..\build\ffmpeg"
#endif

[Setup]
AppId={{C880CFA9-28F0-44E8-AD8D-3CE750B1FB35}
AppName=WinRec
AppVersion={#AppVersion}
AppVerName=WinRec {#AppVersion}
AppPublisher=eebagabe
AppPublisherURL=https://github.com/eebagabe/windows-rec
AppSupportURL=https://github.com/eebagabe/windows-rec/issues
AppUpdatesURL=https://github.com/eebagabe/windows-rec/releases
VersionInfoVersion={#AppVersion}
; sem administrador: instala em %LOCALAPPDATA%\Programs\WinRec
PrivilegesRequired=lowest
DefaultDirName={autopf}\WinRec
DisableWelcomePage=no
DisableProgramGroupPage=yes
UninstallDisplayName=WinRec
UninstallDisplayIcon={app}\WinRec.exe
SetupIconFile=..\assets\icon.ico
WizardStyle=modern dark
WizardImageFile=wizard-100.bmp,wizard-150.bmp,wizard-200.bmp
WizardSmallImageFile=small-100.bmp,small-150.bmp,small-200.bmp
WizardImageAlphaFormat=defined
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.17763
Compression=lzma2/ultra64
SolidCompression=yes
CloseApplications=force
RestartApplications=no
OutputDir=..\dist
OutputBaseFilename=WinRec-Setup

[Languages]
Name: "pt"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"

[Files]
Source: "{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#FFmpegDir}\ffmpeg.exe"; DestDir: "{app}\bin"; Flags: ignoreversion
#ifexist AddBackslash(FFmpegDir) + "LICENSE.txt"
Source: "{#FFmpegDir}\LICENSE.txt"; DestDir: "{app}\bin"; DestName: "ffmpeg-LICENSE.txt"; Flags: ignoreversion
#endif

[Icons]
Name: "{autoprograms}\WinRec"; Filename: "{app}\WinRec.exe"; Comment: "Gravador de tela, webcam e áudio"
Name: "{autodesktop}\WinRec"; Filename: "{app}\WinRec.exe"; Comment: "Gravador de tela, webcam e áudio"; Tasks: desktopicon

[Run]
Filename: "{app}\WinRec.exe"; Description: "{cm:LaunchProgram,WinRec}"; Flags: nowait postinstall skipifsilent

[Code]
// Fecha o WinRec antes de instalar/desinstalar. Primeiro pede para fechar
// (se estiver gravando, ele salva o vídeo); depois de 15 s, força.
procedure CloseWinRec();
var
  Code, I: Integer;
begin
  // código 0 = havia um WinRec aberto
  if not Exec(ExpandConstant('{sys}\taskkill.exe'), '/IM WinRec.exe', '', SW_HIDE, ewWaitUntilTerminated, Code)
     or (Code <> 0) then
    Exit;
  for I := 1 to 30 do
  begin
    Sleep(500);
    // find devolve 1 quando o processo já não aparece
    if Exec(ExpandConstant('{cmd}'), '/C tasklist /FI "IMAGENAME eq WinRec.exe" /NH | find /I "WinRec.exe" >nul',
            '', SW_HIDE, ewWaitUntilTerminated, Code) and (Code <> 0) then
      Exit;
  end;
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /IM WinRec.exe', '', SW_HIDE, ewWaitUntilTerminated, Code);
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  CloseWinRec();
  Result := '';
end;

function InitializeUninstall(): Boolean;
begin
  CloseWinRec();
  Result := True;
end;
