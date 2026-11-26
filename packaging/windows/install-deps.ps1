param(
    [switch]$InstallViGEmBus,
    [switch]$InstallHidHide,
    [switch]$InstallUsbip
)

$ErrorActionPreference = "Stop"
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

function Test-ViGEmBusInstalled {
    $sysPath = Join-Path $env:SystemRoot "System32\drivers\ViGEmBus.sys"
    if (Test-Path $sysPath) { return $true }
    $reg = Get-ItemProperty -Path "HKLM:\SYSTEM\CurrentControlSet\Services\ViGEmBus" -ErrorAction SilentlyContinue
    return ($null -ne $reg)
}

function Test-HidHideInstalled {
    $sysPath = Join-Path $env:SystemRoot "System32\drivers\HidHide.sys"
    if (Test-Path $sysPath) { return $true }
    $cli = Join-Path $env:ProgramFiles "Nefarius Software Solutions\HidHide\x64\HidHideCLI.exe"
    return (Test-Path $cli)
}

function Test-UsbipInstalled {
    return ($null -ne (Get-Command "usbip" -ErrorAction SilentlyContinue))
}

function Get-LatestAssetUrl {
    param(
        [string]$Repo,
        [string]$Pattern,
        [string]$FallbackUrl
    )
    try {
        $headers = @{ "User-Agent" = "DualSense-Studio-Installer" }
        $release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" -Headers $headers -TimeoutSec 15
        $asset = $release.assets | Where-Object { $_.name -match $Pattern } | Select-Object -First 1
        if ($asset -and $asset.browser_download_url) {
            return $asset.browser_download_url
        }
    } catch {
        Write-Warning "Could not query GitHub latest release for ${Repo}: $_"
    }
    return $FallbackUrl
}

function Invoke-ElevatedInstaller {
    param(
        [string]$FilePath,
        [string[]]$ArgumentList
    )
    $proc = Start-Process -FilePath $FilePath -ArgumentList $ArgumentList -Verb RunAs -Wait -PassThru
    if ($proc.ExitCode -ne 0 -and $proc.ExitCode -ne 3010) {
        throw "Installer $FilePath exited with code $($proc.ExitCode)"
    }
}

$tmpDir = Join-Path $env:TEMP "dualsense-studio-deps"
New-Item -ItemType Directory -Force -Path $tmpDir | Out-Null

if ($InstallViGEmBus) {
    if (Test-ViGEmBusInstalled) {
        Write-Host "[OK] ViGEmBus driver is already installed."
    } else {
        Write-Host "Downloading ViGEmBus driver (Xbox 360 / DualShock 4 emulation)..."
        $url = Get-LatestAssetUrl -Repo "nefarius/ViGEmBus" -Pattern "ViGEmBus_.*_x64_x86_arm64\.exe$" -FallbackUrl "https://github.com/nefarius/ViGEmBus/releases/download/v1.22.0/ViGEmBus_1.22.0_x64_x86_arm64.exe"
        $installer = Join-Path $tmpDir "ViGEmBusSetup.exe"
        Invoke-WebRequest -Uri $url -OutFile $installer -UseBasicParsing
        Write-Host "Running ViGEmBus installer (UAC prompt may appear)..."
        Invoke-ElevatedInstaller -FilePath $installer -ArgumentList @("/quiet", "/norestart")
        Write-Host "[OK] ViGEmBus installed."
    }
}

if ($InstallHidHide) {
    if (Test-HidHideInstalled) {
        Write-Host "[OK] HidHide driver is already installed."
    } else {
        Write-Host "Downloading HidHide driver (prevents double-input)..."
        $url = Get-LatestAssetUrl -Repo "nefarius/HidHide" -Pattern "HidHide_.*_x64\.exe$" -FallbackUrl "https://github.com/nefarius/HidHide/releases/latest/download/HidHide_1.5.230_x64.exe"
        $installer = Join-Path $tmpDir "HidHideSetup.exe"
        Invoke-WebRequest -Uri $url -OutFile $installer -UseBasicParsing
        Write-Host "Running HidHide installer (UAC prompt may appear)..."
        Invoke-ElevatedInstaller -FilePath $installer -ArgumentList @("/quiet", "/norestart")
        Write-Host "[OK] HidHide installed."
    }
}

if ($InstallUsbip) {
    if (Test-UsbipInstalled) {
        Write-Host "[OK] usbip-win2 is already installed."
    } else {
        Write-Host "Downloading usbip-win2 (Virtual DualSense USB/IP driver)..."
        $url = Get-LatestAssetUrl -Repo "vadimgrn/usbip-win2" -Pattern "usbip-win2.*\.msi$" -FallbackUrl "https://github.com/vadimgrn/usbip-win2/releases/latest"
        if ($url -match "\.msi$") {
            $msi = Join-Path $tmpDir "usbip-win2.msi"
            Invoke-WebRequest -Uri $url -OutFile $msi -UseBasicParsing
            Write-Host "Running usbip-win2 installer (UAC prompt may appear)..."
            Invoke-ElevatedInstaller -FilePath "msiexec.exe" -ArgumentList @("/i", "`"$msi`"", "/passive", "/norestart")
            Write-Host "[OK] usbip-win2 installed."
        } else {
            Write-Warning "Could not locate usbip-win2 .msi asset automatically. Opening releases page..."
            Start-Process $url
        }
    }
}
