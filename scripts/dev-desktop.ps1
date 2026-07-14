$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot

Push-Location $root
try {
    cargo build -p tutor-web --bin tutor-web
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to build tutor-web. Close any running Tutor Agent window or tutor-web process, then retry cargo tauri dev."
    }
}
finally {
    Pop-Location
}

npm run dev --prefix "$root\web-ui" -- --host 127.0.0.1
if ($LASTEXITCODE -ne 0) {
    throw "Failed to start the web UI development server."
}
