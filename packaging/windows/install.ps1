param(
    [string]$InstallDir = "$env:LOCALAPPDATA\Programs\DualSense Studio",
    [string]$SourceDir = "",
    [switch]$InstallViGEmBus,
    [switch]$InstallHidHide,
    [switch]$InstallUsbip,
    [switch]$AllDeps,
    [switch]$Autostart,
    [switch]$NoDesktopShortcut,
    [switch]$NonInteractive
)

$ErrorActionPreference = "Stop"

Write-Host "=== DualSense Studio Windows Installer ===" -ForegroundColor Cyan

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
if ([string]::IsNullOrWhiteSpace($SourceDir)) {
    $repoRoot = Resolve-Path (Join-Path $scriptDir "..\..")
    $releaseExe = Join-Path $repoRoot "target\release\dualsense-studio.exe"
    $uiDir = Join-Path $repoRoot "crates\dualsense-app\ui"
    if (-not (Test-Path $releaseExe)) {
        throw "Could not find $releaseExe. Build first with 'cargo build --workspace --release' or pass -SourceDir."
    }
    $exeSource = $releaseExe
    $uiSource = $uiDir
} else {
    $exeSource = Join-Path $SourceDir "dualsense-studio.exe"
    $uiSource = Join-Path $SourceDir "ui"
}

if (-not (Test-Path $exeSource)) {
    throw "Missing binary at $exeSource"
}

if ($AllDeps) {
    $InstallViGEmBus = $true
    $InstallHidHide = $true
    $InstallUsbip = $true
}

if (-not $NonInteractive) {
    if (-not $InstallViGEmBus) {
        $ans = Read-Host "Install ViGEmBus driver for Xbox 360 & DualShock 4 emulation? [Y/n]"
        if ([string]::IsNullOrWhiteSpace($ans) -or $ans -match "^[yYsS]") { $InstallViGEmBus = $true }
    }
    if (-not $InstallHidHide) {
        $ans = Read-Host "Install HidHide driver (hides physical DualSense to prevent double-input)? [y/N]"
        if ($ans -match "^[yYsS]") { $InstallHidHide = $true }
    }
    if (-not $InstallUsbip) {
        $ans = Read-Host "Install usbip-win2 driver for Virtual DualSense USB/IP emulation? [y/N]"
        if ($ans -match "^[yYsS]") { $InstallUsbip = $true }
    }
    if (-not $Autostart) {
        $ans = Read-Host "Start DualSense Studio in System Tray on Windows login? [y/N]"
        if ($ans -match "^[yYsS]") { $Autostart = $true }
    }
}

Write-Host "Installing DualSense Studio to $InstallDir..."
New-Item -ItemType Directory -Force -Path "$InstallDir\ui", "$InstallDir\scripts" | Out-Null
Copy-Item $exeSource "$InstallDir\dualsense-studio.exe" -Force
if (Test-Path $uiSource) {
    Copy-Item "$uiSource\*" "$InstallDir\ui\" -Recurse -Force
}
Copy-Item (Join-Path $scriptDir "install-deps.ps1") "$InstallDir\scripts\install-deps.ps1" -Force

# Start Menu & Desktop Shortcuts
$wsh = New-Object -ComObject WScript.Shell
$startMenu = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\DualSense Studio"
New-Item -ItemType Directory -Force -Path $startMenu | Out-Null

$scMain = $wsh.CreateShortcut((Join-Path $startMenu "DualSense Studio.lnk"))
$scMain.TargetPath = "$InstallDir\dualsense-studio.exe"
$scMain.Arguments = "--tray --open-ui"
$scMain.WorkingDirectory = $InstallDir
$scMain.Save()

$scTray = $wsh.CreateShortcut((Join-Path $startMenu "DualSense Studio (System Tray).lnk"))
$scTray.TargetPath = "$InstallDir\dualsense-studio.exe"
$scTray.Arguments = "--tray"
$scTray.WorkingDirectory = $InstallDir
$scTray.Save()

if (-not $NoDesktopShortcut) {
    $desktop = [Environment]::GetFolderPath("Desktop")
    $scDesk = $wsh.CreateShortcut((Join-Path $desktop "DualSense Studio.lnk"))
    $scDesk.TargetPath = "$InstallDir\dualsense-studio.exe"
    $scDesk.Arguments = "--tray --open-ui"
    $scDesk.WorkingDirectory = $InstallDir
    $scDesk.Save()
}

if ($Autostart) {
    $runKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
    Set-ItemProperty -Path $runKey -Name "DualSenseStudio" -Value "`"$InstallDir\dualsense-studio.exe`" --tray"
    Write-Host "[OK] Registered System Tray autostart on Windows login."
}

if ($InstallViGEmBus -or $InstallHidHide -or $InstallUsbip) {
    & "$InstallDir\scripts\install-deps.ps1" `
        -InstallViGEmBus:$InstallViGEmBus `
        -InstallHidHide:$InstallHidHide `
        -InstallUsbip:$InstallUsbip
}

Write-Host "`nDualSense Studio installed successfully!" -ForegroundColor Green
Write-Host "Launch from Start Menu or run: `"$InstallDir\dualsense-studio.exe`" --tray --open-ui"
