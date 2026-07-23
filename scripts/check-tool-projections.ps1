$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$inventoryPath = Join-Path $root "docs/runtime-tool-projections.json"
$inventory = Get-Content -Raw -Encoding UTF8 $inventoryPath | ConvertFrom-Json
$inventoryByName = @{}

foreach ($entry in $inventory) {
    if ($entry.projection -notin @("projected", "ephemeral")) {
        throw "Tool '$($entry.tool)' has unsupported projection '$($entry.projection)'."
    }
    if ($inventoryByName.ContainsKey($entry.tool)) {
        throw "Duplicate Tool projection inventory entry: $($entry.tool)"
    }
    $inventoryByName[$entry.tool] = $entry
}

$toolNamePattern = 'fn\s+name\s*\([^)]*\)\s*->\s*&str\s*\{\s*"([^"]+)"'
$actualTools = @{}
$toolFiles = Get-ChildItem -Path (Join-Path $root "crates") -Recurse -Filter "*.rs" |
    Where-Object { $_.FullName -match '[\\/]src[\\/]' }

foreach ($file in $toolFiles) {
    $text = Get-Content -Raw -Encoding UTF8 $file.FullName
    if ($text -notmatch 'impl\s+Tool\s+for') {
        continue
    }
    if ($text -match 'ToolResult::full\s*\(' -or $text -match 'Ok\s*\(\s*ToolResult\s*\{') {
        throw "Production Tool file '$($file.FullName)' uses an unreviewed Full or struct-literal projection."
    }
    if ($text -notmatch 'ToolResult::(?:projected|ephemeral)\s*\(') {
        throw "Production Tool file '$($file.FullName)' has no explicit Projected or Ephemeral result."
    }

    foreach ($match in [regex]::Matches($text, $toolNamePattern, "Singleline")) {
        $name = $match.Groups[1].Value
        if ($actualTools.ContainsKey($name)) {
            throw "Duplicate production Tool name '$name'."
        }
        $actualTools[$name] = $file.FullName
    }
}

$missingInventory = @($actualTools.Keys | Where-Object { -not $inventoryByName.ContainsKey($_) } | Sort-Object)
$staleInventory = @($inventoryByName.Keys | Where-Object { -not $actualTools.ContainsKey($_) } | Sort-Object)
if ($missingInventory.Count -gt 0 -or $staleInventory.Count -gt 0) {
    $parts = @()
    if ($missingInventory.Count -gt 0) {
        $parts += "Missing inventory entries: $($missingInventory -join ', ')"
    }
    if ($staleInventory.Count -gt 0) {
        $parts += "Stale inventory entries: $($staleInventory -join ', ')"
    }
    throw $parts -join [Environment]::NewLine
}

Write-Host "Tool projection audit passed for $($actualTools.Count) production tools."
