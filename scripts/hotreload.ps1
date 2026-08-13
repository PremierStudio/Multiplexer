# Rebuild and restart multiplexer-desktop when apps/ or crates/ change.
# GPUI has no in-process hot swap; this is the Windows-first reload loop.
#
#   pwsh -File scripts/hotreload.ps1
#   scripts\hotreload.cmd

[CmdletBinding()]
param(
    [int]$DebounceMs = 900
)

$ErrorActionPreference = "Stop"
$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Set-Location $Root

$ExeName = "multiplexer-desktop"
$Exe = Join-Path $Root "target\debug\$ExeName.exe"
$Queue = [System.Collections.Concurrent.ConcurrentQueue[string]]::new()

function Test-ReloadPath {
    param([string]$Path)
    if ([string]::IsNullOrWhiteSpace($Path)) { return $false }
    $norm = $Path.Replace("/", "\")
    if ($norm -match '\\target\\|\\third_party\\|\\mutants\.out|\\\.git\\|\\spike\\') {
        return $false
    }
    return $norm -match '\.(rs|toml|svg|ttf)$'
}

function Stop-Desktop {
    Get-Process -Name $ExeName -ErrorAction SilentlyContinue | ForEach-Object {
        Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue
    }
}

function Wait-CargoIdle {
    $deadline = (Get-Date).AddSeconds(90)
    while (Get-Process -Name "cargo" -ErrorAction SilentlyContinue) {
        if ((Get-Date) -gt $deadline) { break }
        Start-Sleep -Milliseconds 400
    }
}

function Start-Desktop {
    Wait-CargoIdle
    Stop-Desktop
    Write-Host ""
    Write-Host "$(Get-Date -Format 'HH:mm:ss')  building $ExeName"
    cargo build -p multiplexer-desktop
    if ($LASTEXITCODE -ne 0) {
        Write-Host "$(Get-Date -Format 'HH:mm:ss')  build failed; waiting for the next save"
        return
    }
    if (-not (Test-Path $Exe)) {
        Write-Host "$(Get-Date -Format 'HH:mm:ss')  missing $Exe"
        return
    }
    $proc = Start-Process -FilePath $Exe -WorkingDirectory $Root -PassThru
    Write-Host "$(Get-Date -Format 'HH:mm:ss')  running pid $($proc.Id)"
}

function Register-TreeWatch {
    param([string]$Rel)
    $full = Join-Path $Root $Rel
    if (-not (Test-Path $full)) { return $null }
    $w = New-Object System.IO.FileSystemWatcher
    $w.Path = $full
    $w.IncludeSubdirectories = $true
    $w.NotifyFilter = [IO.NotifyFilters]::FileName -bor [IO.NotifyFilters]::LastWrite -bor [IO.NotifyFilters]::Size
    $w.EnableRaisingEvents = $true
    $handler = {
        $item = $Event.SourceEventArgs.FullPath
        [void]$Event.MessageData.Enqueue($item)
    }
    $subs = @(
        (Register-ObjectEvent $w Changed -Action $handler -MessageData $Queue),
        (Register-ObjectEvent $w Created -Action $handler -MessageData $Queue),
        (Register-ObjectEvent $w Renamed -Action $handler -MessageData $Queue)
    )
    [pscustomobject]@{ Watcher = $w; Subs = $subs }
}

Write-Host "Multiplexer hotreload  (rebuild + restart, not in-process GPUI swap)"
Write-Host "watching  apps\\multiplexer-desktop  crates  Cargo.toml"
Write-Host "Ctrl+C stops the watcher (the app keeps running)"

$watches = @(
    (Register-TreeWatch "apps\multiplexer-desktop"),
    (Register-TreeWatch "crates")
)
$rootToml = New-Object System.IO.FileSystemWatcher
$rootToml.Path = $Root
$rootToml.Filter = "Cargo.toml"
$rootToml.IncludeSubdirectories = $false
$rootToml.EnableRaisingEvents = $true
$rootHandler = {
    [void]$Event.MessageData.Enqueue($Event.SourceEventArgs.FullPath)
}
$rootSub = Register-ObjectEvent $rootToml Changed -Action $rootHandler -MessageData $Queue

try {
    Start-Desktop
    while ($true) {
        Start-Sleep -Milliseconds 200
        $reload = $false
        $item = $null
        while ($Queue.TryDequeue([ref]$item)) {
            if (Test-ReloadPath $item) {
                $reload = $true
            }
        }
        if (-not $reload) { continue }
        Start-Sleep -Milliseconds $DebounceMs
        while ($Queue.TryDequeue([ref]$item)) { }
        Start-Desktop
    }
}
finally {
    Get-EventSubscriber | Where-Object { $_.SourceObject -is [System.IO.FileSystemWatcher] } | Unregister-Event -Force -ErrorAction SilentlyContinue
    foreach ($w in $watches) {
        if ($null -eq $w) { continue }
        $w.Watcher.EnableRaisingEvents = $false
        $w.Watcher.Dispose()
    }
    $rootToml.EnableRaisingEvents = $false
    $rootToml.Dispose()
    Unregister-Event -SourceIdentifier $rootSub.Name -ErrorAction SilentlyContinue
}
