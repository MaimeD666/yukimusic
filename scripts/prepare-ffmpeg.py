"""Build the small bundled audio decoder from pinned, unmodified FFmpeg sources.
Uses portable tools in .tools only. Requires Python 3.11+ on Windows x64.
The exact source archive is copied to release/ for redistribution beside the installer.
"""
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parent.parent
TOOLS = ROOT / '.tools'
DOWNLOADS = TOOLS / 'downloads'
DOWNLOADS.mkdir(parents=True, exist_ok=True)
ASSETS = [
    ('ffmpeg-8.0.1.tar.xz', 'https://ffmpeg.org/releases/ffmpeg-8.0.1.tar.xz', '05ee0b03119b45c0bdb4df654b96802e909e0a752f72e4fe3794f487229e5a41'),
    ('w64devkit-x64-2.4.0.7z.exe', 'https://github.com/skeeto/w64devkit/releases/download/v2.4.0/w64devkit-x64-2.4.0.7z.exe', '80678c29a56982d7d3b342f12a0060157df5654c38d82d4adc40704070d6aafb'),
    ('nasm-2.16.03-win64.zip', 'https://www.nasm.us/pub/nasm/releasebuilds/2.16.03/win64/nasm-2.16.03-win64.zip', '3ee4782247bcb874378d02f7eab4e294a84d3d15f3f6ee2de2f47a46aa7226e6'),
]
for name, url, expected in ASSETS:
    path = DOWNLOADS / name
    if not path.exists():
        print(f'Downloading {name}', flush=True)
        urllib.request.urlretrieve(url, path)
    if hashlib.sha256(path.read_bytes()).hexdigest() != expected:
        raise RuntimeError(f'SHA256 mismatch: {name}. Remove this download and retry.')

def run(command, **kwargs):
    subprocess.run(command, check=True, creationflags=subprocess.CREATE_NO_WINDOW, **kwargs)

if not (TOOLS / 'w64devkit/bin/gcc.exe').exists():
    run([str(DOWNLOADS / ASSETS[1][0]), '-y', '-o' + str(TOOLS)])
if not (TOOLS / 'nasm-2.16.03/nasm.exe').exists():
    with zipfile.ZipFile(DOWNLOADS / ASSETS[2][0]) as archive:
        archive.extractall(TOOLS)
source = TOOLS / 'ffmpeg-8.0.1'
if not source.exists():
    with tarfile.open(DOWNLOADS / ASSETS[0][0]) as archive:
        archive.extractall(TOOLS, filter='data')
env = os.environ.copy()
env['PATH'] = os.pathsep.join([str(TOOLS / 'w64devkit/bin'), str(TOOLS / 'nasm-2.16.03'), env['PATH']])
temporary = TOOLS / 'ffmpeg-tmp'
temporary.mkdir(exist_ok=True)
env['TMPDIR'] = temporary.as_posix()
run([str(TOOLS / 'w64devkit/bin/sh.exe'), str(ROOT / 'scripts/ffmpeg-configure.sh').replace('\\', '/')], cwd=source, env=env)
resources = ROOT / 'src-tauri/resources'
shutil.copy2(source / 'ffmpeg.exe', resources / 'ffmpeg.exe')
shutil.copy2(source / 'COPYING.LGPLv2.1', resources / 'FFmpeg-LGPLv2.1.txt')
release = ROOT / 'release'
release.mkdir(exist_ok=True)
shutil.copy2(DOWNLOADS / ASSETS[0][0], release / ASSETS[0][0])
print('Built FFmpeg; corresponding source archive is in release/.', flush=True)
