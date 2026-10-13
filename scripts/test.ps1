. "$PSScriptRoot/env.ps1"
Set-Location (Split-Path $PSScriptRoot -Parent)
npm.cmd run check
if ($LASTEXITCODE) { exit $LASTEXITCODE }
node --experimental-strip-types scripts/palette.test.mjs
if ($LASTEXITCODE) { exit $LASTEXITCODE }
node --experimental-strip-types scripts/bass.test.mjs
if ($LASTEXITCODE) { exit $LASTEXITCODE }
node --experimental-strip-types scripts/catalog.test.mjs
if ($LASTEXITCODE) { exit $LASTEXITCODE }
cargo test --manifest-path src-tauri/Cargo.toml
exit $LASTEXITCODE
