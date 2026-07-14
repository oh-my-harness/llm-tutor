$ErrorActionPreference = "Stop"

$scriptStartedAt = Get-Date
$root = Split-Path -Parent $PSScriptRoot
$targetRoot = Join-Path $root "target\debug"
$webUiRoot = Join-Path $root "web-ui"

function Test-PathPrefix([string]$Path, [string]$Prefix) {
    return $Path -and $Path.StartsWith($Prefix, [System.StringComparison]::OrdinalIgnoreCase)
}

# A terminated `cargo tauri dev` can leave its desktop, backend, or Vite child
# alive on Windows. Only clean processes whose paths belong to this workspace.
$workspaceProcesses = Get-CimInstance Win32_Process | Where-Object {
    $_.CreationDate -lt $scriptStartedAt -and
    ($_.Name -in @("llm-tutor-desktop.exe", "tutor-web.exe")) -and
    (Test-PathPrefix $_.ExecutablePath $targetRoot)
}

foreach ($item in ($workspaceProcesses | Where-Object Name -eq "llm-tutor-desktop.exe")) {
    $process = Get-Process -Id $item.ProcessId -ErrorAction SilentlyContinue
    if (-not $process) { continue }
    $closed = $process.CloseMainWindow()
    if (-not $closed -or -not $process.WaitForExit(3000)) {
        Stop-Process -Id $item.ProcessId -Force -ErrorAction SilentlyContinue
    }
}

foreach ($item in ($workspaceProcesses | Where-Object Name -eq "tutor-web.exe")) {
    Stop-Process -Id $item.ProcessId -Force -ErrorAction SilentlyContinue
}

$workspaceVite = Get-CimInstance Win32_Process | Where-Object {
    $_.CreationDate -lt $scriptStartedAt -and
    $_.Name -eq "node.exe" -and
    $_.CommandLine -and
    $_.CommandLine.Contains($webUiRoot) -and
    $_.CommandLine.Contains("vite")
}
foreach ($item in $workspaceVite) {
    Stop-Process -Id $item.ProcessId -Force -ErrorAction SilentlyContinue
}

Start-Sleep -Milliseconds 250
$portOwner = Get-NetTCPConnection -LocalPort 5173 -State Listen -ErrorAction SilentlyContinue |
    Select-Object -First 1
if ($portOwner) {
    throw "Port 5173 is occupied by process $($portOwner.OwningProcess), which does not belong to this llm-tutor workspace."
}

npm run dev --prefix "$root\web-ui" -- --host 127.0.0.1 --port 5173 --strictPort
if ($LASTEXITCODE -ne 0) {
    throw "Failed to start the web UI development server."
}
