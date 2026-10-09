. "$PSScriptRoot/env.ps1"
Set-Location (Split-Path $PSScriptRoot -Parent)
npm.cmd run check
if ($LASTEXITCODE) { exit $LASTEXITCODE }
npm.cmd run tauri -- build
exit $LASTEXITCODE
