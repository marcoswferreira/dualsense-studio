#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif
#ifndef SourceDir
  #define SourceDir "..\..\dist\dualsense-studio-windows-x86_64"
#endif
#ifndef OutputDir
  #define OutputDir "..\..\dist"
#endif
#ifndef OutputBaseFilename
  #define OutputBaseFilename "dualsense-studio-" + AppVersion + "-windows-x86_64-setup"
#endif

[Setup]
AppId={{8D6B5F2A-4D7E-4D62-9A2E-1B9C7E345A10}
AppName=DualSense Studio
AppVersion={#AppVersion}
AppPublisher=marcoswferreira
AppPublisherURL=https://github.com/marcoswferreira/dualsense-studio
AppSupportURL=https://github.com/marcoswferreira/dualsense-studio/issues
DefaultDirName={localappdata}\Programs\DualSense Studio
DefaultGroupName=DualSense Studio
AllowNoIcons=yes
LicenseFile=..\..\LICENSE
OutputDir={#OutputDir}
OutputBaseFilename={#OutputBaseFilename}
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
CloseApplications=yes
RestartApplications=no
UninstallDisplayIcon={app}\dualsense-studio.exe

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "portuguese"; MessagesFile: "compiler:Languages\Portuguese.isl"
Name: "brazilianportuguese"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"

[Types]
Name: "full"; Description: "Full installation (App + Xbox/DualShock Emulation Drivers)"
Name: "compact"; Description: "Compact installation (DualSense Studio only)"
Name: "custom"; Description: "Custom installation"; Flags: iscustom

[Components]
Name: "core"; Description: "DualSense Studio (Application, Web UI & System Tray)"; Types: full compact custom; Flags: fixed
Name: "deps"; Description: "Controller Emulation Drivers (Optional - requires Administrator prompt)"; Types: full custom
Name: "deps\vigembus"; Description: "ViGEmBus Driver (Xbox 360 / XInput & DualShock 4 Emulation)"; Types: full custom
Name: "deps\hidhide"; Description: "HidHide Driver (Hide physical DualSense to prevent double-input)"; Types: full custom
Name: "deps\usbip"; Description: "usbip-win2 Driver (Native Virtual DualSense USB/IP Emulation)"; Types: custom

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"
Name: "autostart"; Description: "Start DualSense Studio minimized in System Tray on Windows login"; GroupDescription: "Startup:"

[Files]
Source: "{#SourceDir}\dualsense-studio.exe"; DestDir: "{app}"; Flags: ignoreversion; Components: core
Source: "{#SourceDir}\ui\*"; DestDir: "{app}\ui"; Flags: ignoreversion recursesubdirs createallsubdirs; Components: core
Source: "{#SourceDir}\README.md"; DestDir: "{app}"; Flags: ignoreversion; Components: core
Source: "{#SourceDir}\LICENSE"; DestDir: "{app}"; Flags: ignoreversion; Components: core
Source: "install-deps.ps1"; DestDir: "{app}\scripts"; Flags: ignoreversion; Components: core

[Icons]
Name: "{group}\DualSense Studio"; Filename: "{app}\dualsense-studio.exe"; Parameters: "--tray --open-ui"; WorkingDir: "{app}"
Name: "{group}\DualSense Studio (System Tray)"; Filename: "{app}\dualsense-studio.exe"; Parameters: "--tray"; WorkingDir: "{app}"
Name: "{group}\DualSense Studio (Console)"; Filename: "{app}\dualsense-studio.exe"; WorkingDir: "{app}"
Name: "{group}\{cm:UninstallProgram,DualSense Studio}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\DualSense Studio"; Filename: "{app}\dualsense-studio.exe"; Parameters: "--tray --open-ui"; WorkingDir: "{app}"; Tasks: desktopicon

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "DualSenseStudio"; ValueData: """{app}\dualsense-studio.exe"" --tray"; Flags: uninsdeletevalue; Tasks: autostart

[Run]
Filename: "powershell.exe"; Parameters: "-NoProfile -ExecutionPolicy Bypass -File ""{app}\scripts\install-deps.ps1"" -InstallViGEmBus"; StatusMsg: "Installing ViGEmBus (Xbox 360 & DualShock 4 emulation driver)..."; Flags: runhidden waituntilterminated; Components: deps\vigembus
Filename: "powershell.exe"; Parameters: "-NoProfile -ExecutionPolicy Bypass -File ""{app}\scripts\install-deps.ps1"" -InstallHidHide"; StatusMsg: "Installing HidHide (controller hiding filter driver)..."; Flags: runhidden waituntilterminated; Components: deps\hidhide
Filename: "powershell.exe"; Parameters: "-NoProfile -ExecutionPolicy Bypass -File ""{app}\scripts\install-deps.ps1"" -InstallUsbip"; StatusMsg: "Installing usbip-win2 (Virtual DualSense USB/IP driver)..."; Flags: runhidden waituntilterminated; Components: deps\usbip
Filename: "{app}\dualsense-studio.exe"; Parameters: "--tray --open-ui"; Description: "{cm:LaunchProgram,DualSense Studio}"; Flags: nowait postinstall skipifsilent
