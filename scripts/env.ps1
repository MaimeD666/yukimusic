$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$portableRoot = Join-Path $projectRoot '.tools'
$portableNode = Join-Path $portableRoot 'node'
if (Test-Path "$portableNode/node.exe") { $env:PATH = "$portableNode;$env:PATH" }
if (Test-Path "$portableRoot/cargo/bin/cargo.exe") {
    $env:CARGO_HOME = "$portableRoot/cargo"
    $env:RUSTUP_HOME = "$portableRoot/rustup"
    $env:PATH = "$portableRoot/cargo/bin;$env:PATH"
}
$msvcRoot = Join-Path $portableRoot 'msvc'
if (Test-Path "$msvcRoot/setup_x64.bat") {
    # Read the portable toolchain environment; this command only sets variables.
    $envLines = & cmd.exe /d /c "`"$msvcRoot\setup_x64.bat`" && set"
    foreach ($line in $envLines) {
        if ($line -match '^([^=]+)=(.*)$') { [Environment]::SetEnvironmentVariable($matches[1], $matches[2], 'Process') }
    }
}
