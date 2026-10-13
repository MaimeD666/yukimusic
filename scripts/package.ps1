. "$PSScriptRoot/env.ps1"
Set-Location (Split-Path $PSScriptRoot -Parent)
$releaseRoot = Join-Path (Get-Location) 'release'
$portableRoot = Join-Path $releaseRoot 'portable'
$version = (Get-Content -LiteralPath 'src-tauri/tauri.conf.json' -Raw | ConvertFrom-Json).version
$installerName = "yukicloud_${version}_x64-setup.exe"
$portableName = "yukicloud_${version}_x64-portable.zip"
New-Item -ItemType Directory -Force -Path "$portableRoot/resources" | Out-Null
Copy-Item -LiteralPath 'src-tauri/target/release/yukicloud.exe' -Destination "$portableRoot/yukicloud.exe"
foreach ($name in @('ffmpeg.exe','THIRD-PARTY.txt','FFmpeg-LGPLv2.1.txt')) {
    Copy-Item -LiteralPath "src-tauri/resources/$name" -Destination "$portableRoot/resources/$name"
}
foreach ($name in @('manrope-OFL.txt','unifrakturcook-OFL.txt')) {
    Copy-Item -LiteralPath "public/fonts/$name" -Destination "$portableRoot/resources/$name"
}
# Remove the obsolete license left by early local development builds only.
$oldLicense = Join-Path $portableRoot 'resources/FFmpeg-GPLv3.txt'
if (Test-Path -LiteralPath $oldLicense) { Remove-Item -LiteralPath $oldLicense }
Copy-Item -LiteralPath "src-tauri/target/release/bundle/nsis/$installerName" -Destination $releaseRoot
Compress-Archive -Path "$portableRoot/*" -DestinationPath "$releaseRoot/$portableName" -Force
Copy-Item -LiteralPath "$releaseRoot/$portableName" -Destination "$releaseRoot/yukicloud-portable.zip"
Copy-Item -LiteralPath 'README.md' -Destination "$releaseRoot/README.md"
$files = @($installerName,$portableName,'ffmpeg-8.0.1.tar.xz')
$lines = foreach ($name in $files) { ((Get-FileHash -LiteralPath "$releaseRoot/$name" -Algorithm SHA256).Hash.ToLower() + '  ' + $name) }
[IO.File]::WriteAllLines("$releaseRoot/SHA256SUMS.txt", $lines, [Text.UTF8Encoding]::new($false))
