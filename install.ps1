# install.ps1 — Install microcodes CLI on Windows
# Run with: powershell -ExecutionPolicy Bypass -File install.ps1
param(
    [string]$InstallDir = "$env:APPDATA\microcodes"
)

$ErrorActionPreference = "Stop"
$BinaryName = "microcodes"

Write-Host "==> Building microcodes (release)..." -ForegroundColor Cyan

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Error "cargo not found. Install Rust from https://rustup.rs and try again."
    exit 1
}

cargo build --release
if ($LASTEXITCODE -ne 0) {
    Write-Error "Build failed."
    exit 1
}

$SourceBinary = Join-Path (Get-Location) "target\release\${BinaryName}.exe"
if (-not (Test-Path $SourceBinary)) {
    Write-Error "Build succeeded but binary not found at: $SourceBinary"
    exit 1
}

# Create install directory
Write-Host "==> Installing to $InstallDir ..." -ForegroundColor Cyan
if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir | Out-Null
}

Copy-Item -Force $SourceBinary (Join-Path $InstallDir "${BinaryName}.exe")

# Create mcodes.bat and microcodes.bat launchers
$BatContent = "@echo off`r`n`"%~dp0${BinaryName}.exe`" %*`r`n"
[System.IO.File]::WriteAllText((Join-Path $InstallDir "mcodes.bat"), $BatContent)
[System.IO.File]::WriteAllText((Join-Path $InstallDir "${BinaryName}.bat"), $BatContent)

Write-Host "==> Adding $InstallDir to user PATH (registry)..." -ForegroundColor Cyan
$RegPath = "HKCU:\Environment"
$CurrentPath = (Get-ItemProperty -Path $RegPath -Name "Path" -ErrorAction SilentlyContinue).Path

if ($CurrentPath -notlike "*$InstallDir*") {
    $NewPath = if ($CurrentPath) { "$CurrentPath;$InstallDir" } else { $InstallDir }
    Set-ItemProperty -Path $RegPath -Name "Path" -Value $NewPath -Type ExpandString

    # Broadcast environment change so open terminals pick it up
    $signature = @"
[DllImport("user32.dll", SetLastError=true, CharSet=CharSet.Auto)]
public static extern IntPtr SendMessageTimeout(
    IntPtr hWnd, uint Msg, UIntPtr wParam, string lParam,
    uint fuFlags, uint uTimeout, out UIntPtr lpdwResult);
"@
    $type = Add-Type -MemberDefinition $signature -Name WinAPI -Namespace SendMessage -PassThru
    $result = [UIntPtr]::Zero
    $type::SendMessageTimeout(
        [IntPtr]0xffff, 0x001A, [UIntPtr]::Zero, "Environment",
        0x0002, 5000, [ref]$result
    ) | Out-Null
}

Write-Host ""
Write-Host "v microcodes installed successfully!" -ForegroundColor Green
Write-Host ""
Write-Host "Installed files:"
Write-Host "  $InstallDir\${BinaryName}.exe"
Write-Host "  $InstallDir\mcodes.bat"
Write-Host "  $InstallDir\${BinaryName}.bat"
Write-Host ""
Write-Host "----------------------------------------------------------------" -ForegroundColor Yellow
Write-Host "  Next step: set your API token" -ForegroundColor Yellow
Write-Host ""
Write-Host "  For the current session:" -ForegroundColor Yellow
Write-Host "    `$env:MICROCODES_API_TOKEN = 'your_key_here'"
Write-Host ""
Write-Host "  Permanently (user environment variable):" -ForegroundColor Yellow
Write-Host "    [System.Environment]::SetEnvironmentVariable("
Write-Host "      'MICROCODES_API_TOKEN', 'your_key_here', 'User')"
Write-Host "----------------------------------------------------------------" -ForegroundColor Yellow
Write-Host ""
Write-Host "  Open a new terminal and run:  mcodes --help"
Write-Host ""
Write-Host "  Note: you may need to restart your terminal for PATH changes to take effect."
