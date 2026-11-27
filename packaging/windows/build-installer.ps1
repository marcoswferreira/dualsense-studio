param(
    [string]$Version = "0.2.0",
    [switch]$SkipFrontendBuild
)

$ErrorActionPreference = "Stop"
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Resolve-Path (Join-Path $scriptDir "..\..")

Push-Location $repoRoot
try {
    if (-not $SkipFrontendBuild) {
        Write-Host "Building frontend production bundle..." -ForegroundColor Cyan
        Push-Location "frontend"
        npm.cmd ci
        npm.cmd run build
        Pop-Location
    }

    Write-Host "Building release binary..." -ForegroundColor Cyan
    cargo build --workspace --release --locked

    $stage = Join-Path $repoRoot "dist\dualsense-studio-windows-x86_64"
    New-Item -ItemType Directory -Force -Path "$stage\ui" | Out-Null
    Copy-Item "target\release\dualsense-studio.exe" "$stage\" -Force
    Copy-Item "crates\dualsense-app\ui\*" "$stage\ui\" -Recurse -Force
    Copy-Item "README.md", "LICENSE" "$stage\" -Force

    $iscc = Get-Command "ISCC.exe" -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source
    if (-not $iscc -and (Test-Path "C:\Program Files (x86)\Inno Setup 6\ISCC.exe")) {
        $iscc = "C:\Program Files (x86)\Inno Setup 6\ISCC.exe"
    }

    if ($iscc) {
        Write-Host "Compiling Inno Setup installer..." -ForegroundColor Cyan
        & $iscc "/DAppVersion=$Version" "/DSourceDir=$stage" "/DOutputDir=$repoRoot\dist" (Join-Path $scriptDir "dualsense-studio.iss")
        Write-Host "Installer created in $repoRoot\dist" -ForegroundColor Green
    } else {
        Write-Warning "Inno Setup (ISCC.exe) not found. Staged portable files in $stage; you can still run packaging\windows\install.ps1."
    }
} finally {
    Pop-Location
}
