#!/bin/sh
# FFmpeg 8.0.1, unchanged upstream source, LGPL 2.1 or later.
# Only audio decoding/resampling and the HTTP/HLS inputs used by the player.
set -eu
./configure \
  --target-os=mingw32 --arch=x86_64 --cc=gcc --cxx=g++ \
  --disable-autodetect --disable-everything --disable-doc --disable-debug \
  --disable-ffplay --disable-ffprobe --disable-avdevice --disable-swscale \
  --disable-iconv --disable-bzlib --disable-lzma --disable-zlib \
  --enable-static --disable-shared --enable-small --enable-ffmpeg --enable-network \
  --enable-schannel --enable-w32threads \
  --enable-decoder=aac,aac_fixed,mp3,mp3float,opus,vorbis,flac,pcm_s16le,pcm_f32le \
  --enable-parser=aac,aac_latm,mpegaudio,opus,vorbis,flac \
  --enable-demuxer=aac,mp3,mpegts,hls,mov,ogg,flac,wav \
  --enable-muxer=pcm_f32le --enable-encoder=pcm_f32le \
  --enable-protocol=file,pipe,http,https,tcp,tls,crypto,data \
  --enable-filter=aresample,aformat,anull
make -j4 ffmpeg.exe
strip ffmpeg.exe
