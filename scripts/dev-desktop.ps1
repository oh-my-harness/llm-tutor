$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot

Push-Location $root
try {
    cargo build -p tutor-web
}
finally {
    Pop-Location
}

npm run dev --prefix "$root\web-ui" -- --host 127.0.0.1
