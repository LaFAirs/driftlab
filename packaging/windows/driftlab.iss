; Inno Setup script for DriftLab. Build: iscc /DAppVersion=0.1.0 /DSourceDir=<dir with driftlab.exe + driftlab-cli.exe> driftlab.iss
#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif
#ifndef SourceDir
  #define SourceDir "..\..\dist\windows"
#endif

[Setup]
; Fixed AppId => installing a newer version upgrades in place (no double installation).
AppId={{6F1D2E9B-3C4A-4B57-9E0A-D21F7A4C8B13}
AppName=DriftLab
AppVersion={#AppVersion}
AppVerName=DriftLab {#AppVersion}
AppPublisher=DriftLab contributors
AppPublisherURL=https://github.com/LaFAirs/driftlab
AppSupportURL=https://github.com/LaFAirs/driftlab/issues
AppUpdatesURL=https://github.com/LaFAirs/driftlab/releases
DefaultDirName={autopf}\DriftLab
DefaultGroupName=DriftLab
DisableProgramGroupPage=yes
OutputDir=..\..\dist
OutputBaseFilename=DriftLab-{#AppVersion}-windows-x64-setup
SetupIconFile=..\..\assets\icons\driftlab.ico
UninstallDisplayIcon={app}\driftlab.exe
WizardStyle=modern
Compression=lzma2/max
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
LicenseFile=..\..\LICENSE
CloseApplications=yes
VersionInfoVersion={#AppVersion}
VersionInfoProductName=DriftLab

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "german"; MessagesFile: "compiler:Languages\German.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#SourceDir}\driftlab.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\driftlab-cli.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\README.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\DriftLab"; Filename: "{app}\driftlab.exe"
Name: "{group}\{cm:UninstallProgram,DriftLab}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\DriftLab"; Filename: "{app}\driftlab.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\driftlab.exe"; Description: "{cm:LaunchProgram,DriftLab}"; Flags: nowait postinstall skipifsilent
