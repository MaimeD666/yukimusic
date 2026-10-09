. "$PSScriptRoot/env.ps1"
Set-Location (Split-Path $PSScriptRoot -Parent)
npm.cmd run desktop
exit $LASTEXITCODE
