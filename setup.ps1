# NANO // OXO: REALMS -- Windows setup, optional flash, and launcher.
# Run from extracted ZIP folder (no administrator rights required for the game):
#   powershell -NoProfile -ExecutionPolicy Bypass -File .\setup.ps1 -Port COM3
# Optional:
#   .\setup.ps1 -Port COM3 -Flash             # upload updated Arduino firmware
#   .\setup.ps1 -Port COM3 -Flash -OldBootloader
#   .\setup.ps1 -Port COM3 -Probe             # serial protocol diagnostic, makes a test move
#   .\setup.ps1 -NoRun                       # compile Rust only
[CmdletBinding()]
param(
    [string]$Port = '',
    [switch]$Flash,
    [switch]$OldBootloader,
    [switch]$Probe,
    [switch]$NoRun
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Say([string]$Message) {
    Write-Host ("[OXO] " + $Message) -ForegroundColor Cyan
}
function Tip([string]$Message) {
    Write-Host ("[OXO] " + $Message) -ForegroundColor Yellow
}
function Ask-Yes([string]$Question) {
    $answer = Read-Host "$Question [y/N]"
    return $answer -match '^(y|yes|д|да)$'
}
function Check-Exit([string]$What) {
    if ($LASTEXITCODE -ne 0) {
        throw "$What failed (exit code $LASTEXITCODE)."
    }
}
function Find-Command([string]$Name) {
    $cmd = Get-Command $Name -ErrorAction SilentlyContinue
    if ($null -ne $cmd) { return $cmd.Source }
    return $null
}
function Refresh-Path {
    $machine = [Environment]::GetEnvironmentVariable('Path', 'Machine')
    $user = [Environment]::GetEnvironmentVariable('Path', 'User')
    # Keep current process PATH as well (may contain the freshly added Rust bin dir).
    $env:Path = "$env:Path;$machine;$user"
}

try {
    if (-not [Environment]::OSVersion.Platform.Equals([PlatformID]::Win32NT)) {
        throw 'setup.ps1 targets Windows. For Linux/macOS see README.md.'
    }
    $root = $PSScriptRoot
    if ([string]::IsNullOrWhiteSpace($root)) { $root = Split-Path -Parent $MyInvocation.MyCommand.Path }
    $cargoDir = Join-Path $root 'tui'
    $firmwareDir = Join-Path $root 'firmware\NanoOXO'
    $manifest = Join-Path $cargoDir 'Cargo.toml'
    $sketch = Join-Path $firmwareDir 'NanoOXO.ino'
    $game = Join-Path $cargoDir 'target\release\nano-oxo.exe'
    if (-not (Test-Path -LiteralPath $manifest) -or -not (Test-Path -LiteralPath $sketch)) {
        throw 'The archive is incomplete. Extract the WHOLE ZIP, not only setup.ps1.'
    }
    Say 'NANO // OXO: REALMS | Windows setup'
    Say "Project: $root"

    # Detect Cargo. Never download or execute anything silently.
    $cargo = Find-Command 'cargo.exe'
    if (-not $cargo) {
        $rustBin = Join-Path $env:USERPROFILE '.cargo\bin'
        if (Test-Path -LiteralPath (Join-Path $rustBin 'cargo.exe')) {
            $env:Path = "$rustBin;$env:Path"
            $cargo = Find-Command 'cargo.exe'
        }
    }
    if (-not $cargo) {
        Tip 'Rust/Cargo is not installed or is not in PATH.'
        if ((Find-Command 'winget.exe') -and (Ask-Yes 'Install official Rustup through winget now?')) {
            & winget.exe install --id Rustlang.Rustup --exact --source winget
            Check-Exit 'Rustup installation'
            Refresh-Path
            $rustBin = Join-Path $env:USERPROFILE '.cargo\bin'
            $env:Path = "$rustBin;$env:Path"
            $cargo = Find-Command 'cargo.exe'
        }
        if (-not $cargo) {
            throw 'Install Rust from https://rustup.rs/ (MSVC toolchain), reopen PowerShell, then rerun setup.ps1.'
        }
    }

    # Build the viewport. Cargo automatically fetches packages the first time.
    Say 'Building optimized Rust client...'
    Push-Location -LiteralPath $cargoDir
    try {
        & $cargo build --release
        Check-Exit 'cargo build --release'
    }
    finally { Pop-Location }
    if (-not (Test-Path -LiteralPath $game)) { throw "Executable not found: $game" }
    Say 'Rust client built successfully.'
    if ($NoRun -and -not $Flash -and -not $Probe) {
        Say "Build-only mode. Executable: $game"
        return
    }

    # --list is provided by our Rust client. Resolve one port or ask the user.
    if ([string]::IsNullOrWhiteSpace($Port)) {
        $rawPorts = @(& $game --list)
        Check-Exit 'serial port listing'
        $ports = @($rawPorts | ForEach-Object {
            if ($_ -match '^\s*(COM\d+)\s*$') { $matches[1] }
        })
        if ($ports.Count -eq 0) {
            throw 'No COM ports found. Connect Arduino with a data USB cable, check Device Manager, and try again.'
        }
        if ($ports.Count -eq 1) {
            $Port = $ports[0]
            Say "Auto-selected serial port: $Port"
        }
        else {
            Tip ("Detected COM ports: " + ($ports -join ', '))
            $Port = Read-Host 'Enter the COM port of your Arduino (e.g. COM3)'
        }
    }
    if ($Port -notmatch '^COM\d{1,4}$') {
        throw "Invalid Windows port '$Port'. Use COM3, COM5, etc."
    }
    $Port = $Port.ToUpperInvariant()

    if ($Flash) {
        Say 'Firmware upload requested.'
        $arduinoCli = Find-Command 'arduino-cli.exe'
        if (-not $arduinoCli) {
            Tip 'arduino-cli is not available in PATH.'
            if ((Find-Command 'winget.exe') -and (Ask-Yes 'Install official Arduino CLI through winget now?')) {
                & winget.exe install --id ArduinoSA.CLI --exact --source winget
                Check-Exit 'Arduino CLI installation'
                Refresh-Path
                $arduinoCli = Find-Command 'arduino-cli.exe'
            }
        }
        if (-not $arduinoCli) {
            throw "Install Arduino CLI from https://docs.arduino.cc/arduino-cli/installation/ OR upload $sketch with Arduino IDE, then run without -Flash."
        }
        $coreList = (& $arduinoCli core list) | Out-String
        Check-Exit 'arduino-cli core list'
        if ($coreList -notmatch '(?m)^\s*arduino:avr\s+') {
            Say 'Installing official Arduino AVR platform (internet needed)...'
            & $arduinoCli core update-index
            Check-Exit 'arduino-cli core update-index'
            & $arduinoCli core install 'arduino:avr'
            Check-Exit 'arduino-cli core install arduino:avr'
        }
        $fqbn = 'arduino:avr:nano:cpu=atmega328'
        if ($OldBootloader) { $fqbn = 'arduino:avr:nano:cpu=atmega328old' }
        Say "Compiling Arduino Nano firmware: $fqbn"
        & $arduinoCli compile --fqbn $fqbn $firmwareDir
        Check-Exit 'Arduino firmware compilation'
        Tip "Close Arduino IDE Serial Monitor/Plotter and any other app using $Port before uploading."
        Say "Uploading firmware to $Port..."
        & $arduinoCli upload --port $Port --fqbn $fqbn $firmwareDir
        if ($LASTEXITCODE -ne 0) {
            if (-not $OldBootloader) {
                Tip 'Upload failed with the new bootloader setting.'
                if (Ask-Yes 'Retry upload with the Nano Old Bootloader setting?') {
                    $fqbn = 'arduino:avr:nano:cpu=atmega328old'
                    & $arduinoCli compile --fqbn $fqbn $firmwareDir
                    Check-Exit 'Old Bootloader compilation'
                    & $arduinoCli upload --port $Port --fqbn $fqbn $firmwareDir
                    Check-Exit 'Old Bootloader upload'
                }
                else { throw 'Firmware upload failed. Try -OldBootloader or upload in Arduino IDE.' }
            }
            else { throw 'Firmware upload failed. Check USB cable, COM port and board type.' }
        }
        Say 'Arduino firmware uploaded.'
        # Classic Nano reboots when the serial port is opened by the TUI.
        Start-Sleep -Milliseconds 800
    }
    else {
        Tip 'Using firmware already uploaded to Arduino. To upload the bundled firmware via CLI, rerun with -Flash.'
    }

    if ($Probe) {
        Tip 'Probe makes a TEST MOVE on the Arduino and prints received frames.'
        & $game --probe $Port
        Check-Exit 'serial probe'
        return
    }
    if ($NoRun) {
        Say 'Setup complete (-NoRun).'
        return
    }
    Tip "Close Arduino Serial Monitor/Plotter before starting; only one program can use $Port."
    Say "Launching OXO on $Port... Press Q to quit."
    & $game --port $Port
    Check-Exit 'game client'
}
catch {
    Write-Host ("`n[OXO ERROR] " + $_.Exception.Message) -ForegroundColor Red
    Write-Host 'See README.md or БЫСТРЫЙ_СТАРТ.md for troubleshooting.' -ForegroundColor Yellow
    exit 1
}
