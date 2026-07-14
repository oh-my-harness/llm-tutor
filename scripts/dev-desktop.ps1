$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot

npm run dev --prefix "$root\web-ui" -- --host 127.0.0.1 --port 5173 --strictPort
if ($LASTEXITCODE -ne 0) {
    throw "Failed to start the web UI development server."
}
