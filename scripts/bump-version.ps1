param(
    [Parameter(Mandatory = $true, Position = 0)]
    [string]$Version
)

$ErrorActionPreference = "Stop"

if ($Version -notmatch '^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$') {
    throw "Version must be a valid SemVer value, for example 0.1.0 or 0.1.0-alpha.1."
}

$root = Split-Path -Parent $PSScriptRoot

function Update-TextFile {
    param(
        [string]$Path,
        [scriptblock]$Updater
    )

    $text = Get-Content -Raw -LiteralPath $Path
    $next = & $Updater $text
    if ($null -eq $next -or $next -eq $text) {
        throw "No version change was made in $Path."
    }
    Set-Content -LiteralPath $Path -Value $next -NoNewline
    Write-Host "Updated $Path"
}

function Replace-First {
    param(
        [string]$Text,
        [string]$Pattern,
        [string]$Replacement
    )

    $regex = [regex]::new($Pattern)
    return $regex.Replace($Text, $Replacement, 1)
}

function Update-JsonVersion {
    param([string]$Path)

    Update-TextFile -Path $Path -Updater {
        param($text)
        Replace-First $text '(?m)^(\s*"version"\s*:\s*)"[^"]+"' "`$1`"$Version`""
    }
}

function Update-PackageLockVersion {
    param([string]$Path)

    Update-TextFile -Path $Path -Updater {
        param($text)
        $next = Replace-First $text '(?m)^(  "version"\s*:\s*)"[^"]+"' "`$1`"$Version`""
        $next = Replace-First $next '(?m)^(      "version"\s*:\s*)"[^"]+"' "`$1`"$Version`""
        $next
    }
}

Update-TextFile -Path (Join-Path $root "Cargo.toml") -Updater {
    param($text)
    $text -replace '(?m)^version\s*=\s*"[^"]+"', "version = `"$Version`""
}

Update-JsonVersion -Path (Join-Path $root "src-tauri\tauri.conf.json")
Update-JsonVersion -Path (Join-Path $root "web-ui\package.json")
Update-PackageLockVersion -Path (Join-Path $root "web-ui\package-lock.json")

Write-Host ""
Write-Host "Version updated to $Version." -ForegroundColor Green
Write-Host "Recommended next checks:"
Write-Host "  cargo metadata --no-deps --format-version 1 > `$null"
Write-Host "  npm run build --prefix web-ui"
